use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use tempfile::NamedTempFile;
use tokio::fs;
use url::Url;

use crate::error::AppError;

#[derive(Debug)]
pub enum ResolvedInput {
    Path(PathBuf),
    Temp(NamedTempFile),
}

impl ResolvedInput {
    #[must_use]
    pub fn file_ref(&self) -> String {
        match self {
            Self::Path(p) => format!("@{}", p.display()),
            Self::Temp(f) => format!("@{}", f.path().display()),
        }
    }

    #[must_use]
    pub fn local_path(&self) -> &Path {
        match self {
            Self::Path(p) => p,
            Self::Temp(f) => f.path(),
        }
    }
}

pub async fn resolve(input: &str) -> Result<ResolvedInput, AppError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(AppError::Input("empty input".to_string()));
    }

    if starts_with_ignore_ascii_case(trimmed, "data:") {
        return resolve_data_uri(&trimmed[5..]).await;
    }

    if let Ok(url) = Url::parse(trimmed) {
        if matches!(url.scheme(), "http" | "https") {
            return Err(AppError::Input(format!(
                "remote URLs are not supported — download the file to a local \
                 path first, then pass that path. Got: {trimmed}"
            )));
        }
        if trimmed.contains("://") {
            return Err(AppError::Input(format!(
                "unsupported URL scheme: {} — only local paths and data URIs are accepted",
                url.scheme()
            )));
        }
    }

    resolve_path(trimmed).await
}

async fn resolve_path(p: &str) -> Result<ResolvedInput, AppError> {
    let path = Path::new(p);
    let meta = fs::metadata(path)
        .await
        .map_err(|e| AppError::Input(format!("cannot read input path {p}: {e}")))?;
    if meta.is_dir() {
        return Err(AppError::Input(format!(
            "input path is a directory, not a file: {p}"
        )));
    }
    Ok(ResolvedInput::Path(path.to_path_buf()))
}

async fn resolve_data_uri(raw: &str) -> Result<ResolvedInput, AppError> {
    let comma = raw
        .find(',')
        .ok_or_else(|| AppError::Input("malformed data URI: missing comma".to_string()))?;

    let meta = &raw[..comma];
    let payload = &raw[comma + 1..];
    let (mime_part, is_base64) = parse_data_uri_meta(meta);

    if !is_base64 {
        return Err(AppError::Input(
            "only base64 data URIs are supported (use `;base64,`)".to_string(),
        ));
    }

    let ext = extension_for_mime(mime_part);
    let suffix = format!(".{ext}");
    let mut tmp = tempfile::Builder::new()
        .suffix(&suffix)
        .tempfile()
        .map_err(AppError::from)?;
    let mut decoder = base64::read::DecoderReader::new(
        payload.as_bytes(),
        &base64::engine::general_purpose::STANDARD,
    );
    io::copy(&mut decoder, tmp.as_file_mut()).map_err(|e| {
        if e.kind() == io::ErrorKind::InvalidData {
            AppError::Input(format!("failed to decode base64 payload: {e}"))
        } else {
            AppError::Io(e)
        }
    })?;
    tmp.as_file_mut().flush().map_err(AppError::Io)?;
    Ok(ResolvedInput::Temp(tmp))
}

fn starts_with_ignore_ascii_case(s: &str, prefix: &str) -> bool {
    s.get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}

fn parse_data_uri_meta(meta: &str) -> (&str, bool) {
    let mut is_base64 = false;
    let mut mime = "";
    for part in meta.split(';') {
        if part.eq_ignore_ascii_case("base64") {
            is_base64 = true;
        } else if !part.is_empty() && mime.is_empty() {
            mime = part;
        }
    }
    (mime, is_base64)
}

fn extension_for_mime(mime: &str) -> &'static str {
    match mime.to_ascii_lowercase().as_str() {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/bmp" => "bmp",
        "image/tiff" => "tiff",
        "image/svg+xml" => "svg",
        "audio/wav" | "audio/x-wav" | "audio/wave" => "wav",
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/ogg" => "ogg",
        "audio/mp4" | "audio/m4a" => "m4a",
        "audio/aac" => "aac",
        "audio/flac" => "flac",
        "audio/webm" => "weba",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "video/quicktime" => "mov",
        "video/x-matroska" => "mkv",
        "video/x-msvideo" => "avi",
        _ => "bin",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_data_uri_meta_with_base64_and_mime() {
        assert_eq!(parse_data_uri_meta("image/png;base64"), ("image/png", true));
        assert_eq!(
            parse_data_uri_meta("base64;image/jpeg"),
            ("image/jpeg", true)
        );
        assert_eq!(
            parse_data_uri_meta("image/png;charset=utf-8;base64"),
            ("image/png", true)
        );
        assert_eq!(parse_data_uri_meta("audio/wav"), ("audio/wav", false));
        assert_eq!(parse_data_uri_meta(";base64"), ("", true));
    }

    #[test]
    fn extension_mapping_covers_common_types() {
        assert_eq!(extension_for_mime("image/png"), "png");
        assert_eq!(extension_for_mime("image/jpeg"), "jpg");
        assert_eq!(extension_for_mime("audio/wav"), "wav");
        assert_eq!(extension_for_mime("video/mp4"), "mp4");
        assert_eq!(extension_for_mime("application/octet-stream"), "bin");
        assert_eq!(extension_for_mime("IMAGE/PNG"), "png");
    }

    #[tokio::test]
    async fn rejects_empty_input() {
        let err = resolve("").await.unwrap_err();
        assert!(matches!(err, AppError::Input(_)));
    }

    #[tokio::test]
    async fn rejects_https_url_with_download_hint() {
        let err = resolve("https://example.com/x.png").await.unwrap_err();
        match err {
            AppError::Input(msg) => {
                assert!(
                    msg.contains("download"),
                    "error should mention downloading: {msg}"
                );
                assert!(msg.contains("https://example.com/x.png"));
            }
            other => panic!("expected Input error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn rejects_http_url_with_download_hint() {
        let err = resolve("http://example.com/x.png").await.unwrap_err();
        assert!(matches!(err, AppError::Input(_)));
    }

    #[tokio::test]
    async fn rejects_unsupported_scheme() {
        let err = resolve("ftp://example.com/x.png").await.unwrap_err();
        assert!(matches!(err, AppError::Input(_)));
    }

    #[tokio::test]
    async fn treats_colon_without_url_slashes_as_local_path() {
        let err = resolve("foo:bar.png").await.unwrap_err();
        match err {
            AppError::Input(msg) => assert!(
                msg.contains("cannot read input path"),
                "colon path should be treated as local path: {msg}"
            ),
            other => panic!("expected Input error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn rejects_non_base64_data_uri() {
        let err = resolve("data:image/png,rawbytesthatarenotbase64")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Input(_)));
    }

    #[tokio::test]
    async fn resolves_base64_data_uri_to_tempfile() {
        const ONE_PIXEL_RED_PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4\
             nGP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==";

        let r = resolve(ONE_PIXEL_RED_PNG).await.unwrap();
        match &r {
            ResolvedInput::Temp(f) => {
                let path = f.path();
                assert!(path.to_string_lossy().ends_with(".png"));
                let bytes = std::fs::read(path).unwrap();
                assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
            }
            other => panic!("expected Temp, got {other:?}"),
        }
        assert!(r.file_ref().starts_with("@"));
        drop(r);
    }
}
