use std::fs::File;
use std::io::Read as _;
use std::path::Path;
use std::sync::Once;
use std::time::Duration;

use ffmpeg::{format::stream::Disposition, media};
use ffmpeg_next as ffmpeg;

static FFMPEG_INIT: Once = Once::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Audio,
    Video,
}

impl MediaKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Audio => "audio",
            Self::Video => "video",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaProbe {
    pub kind: Option<MediaKind>,
    pub duration: Option<Duration>,
}

#[must_use]
pub fn probe_media(path: &Path) -> MediaProbe {
    if looks_like_image(path) {
        return MediaProbe {
            kind: Some(MediaKind::Image),
            duration: None,
        };
    }

    init_ffmpeg();
    let ictx = match ffmpeg::format::input(path) {
        Ok(ictx) => ictx,
        Err(_) => {
            return MediaProbe {
                kind: None,
                duration: None,
            };
        }
    };

    let mut has_audio = false;
    let mut has_video = false;
    let mut has_non_attached_video = false;

    for stream in ictx.streams() {
        match stream.parameters().medium() {
            media::Type::Audio => has_audio = true,
            media::Type::Video => {
                has_video = true;
                if !stream.disposition().contains(Disposition::ATTACHED_PIC) {
                    has_non_attached_video = true;
                }
            }
            _ => {}
        }
    }

    let kind = if has_non_attached_video || (has_video && !has_audio) {
        Some(MediaKind::Video)
    } else if has_audio {
        Some(MediaKind::Audio)
    } else {
        None
    };

    MediaProbe {
        kind,
        duration: duration_from_micros(ictx.duration()),
    }
}

fn init_ffmpeg() {
    FFMPEG_INIT.call_once(|| {
        let _ = ffmpeg::init();
    });
}

fn duration_from_micros(micros: i64) -> Option<Duration> {
    if micros > 0 {
        Some(Duration::from_micros(u64::try_from(micros).ok()?))
    } else {
        None
    }
}

fn looks_like_image(path: &Path) -> bool {
    let mut buf = [0_u8; 512];
    let len = File::open(path)
        .and_then(|mut f| f.read(&mut buf))
        .unwrap_or(0);
    let bytes = &buf[..len];

    has_image_signature(bytes) || looks_like_svg(bytes)
}

fn has_image_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(b"\xff\xd8\xff")
        || bytes.starts_with(b"GIF87a")
        || bytes.starts_with(b"GIF89a")
        || bytes.starts_with(b"BM")
        || bytes.starts_with(b"II*\0")
        || bytes.starts_with(b"MM\0*")
        || bytes.starts_with(b"\0\0\x01\0")
        || is_webp(bytes)
        || is_isobmff_image(bytes)
}

fn is_webp(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP"
}

fn is_isobmff_image(bytes: &[u8]) -> bool {
    if bytes.len() < 12 || &bytes[4..8] != b"ftyp" {
        return false;
    }
    let brand = &bytes[8..12];
    matches!(
        brand,
        b"avif" | b"avis" | b"heic" | b"heix" | b"hevc" | b"hevx" | b"mif1" | b"msf1"
    )
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text.trim_start_matches('\u{feff}').trim_start(),
        Err(_) => return false,
    };
    text.starts_with("<svg") || text.starts_with("<?xml") && text.contains("<svg")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_png_as_image() {
        let mut f = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
        std::io::Write::write_all(&mut f, b"\x89PNG\r\n\x1a\nrest").unwrap();

        let probe = probe_media(f.path());
        assert_eq!(probe.kind, Some(MediaKind::Image));
        assert_eq!(probe.duration, None);
    }
}
