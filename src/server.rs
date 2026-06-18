use std::sync::OnceLock;
use std::time::Duration;

use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ServerCapabilities, ServerInfo},
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    tool, tool_handler, tool_router,
};

use crate::agy;
use crate::error::AppError;
use crate::input::{self, ResolvedInput};
use crate::models::{self, ModelInfo};
use crate::probe::{self, MediaKind, MediaProbe};
use crate::prompts::{self, Modality};

const AUDIO_LONG_THRESHOLD: Duration = Duration::from_secs(1800);
const VIDEO_LONG_THRESHOLD: Duration = Duration::from_secs(300);

static MODELS: OnceLock<Vec<ModelInfo>> = OnceLock::new();

fn cached_models() -> Option<&'static [ModelInfo]> {
    if let Some(models) = MODELS.get() {
        return Some(models);
    }

    let loaded = models::list_models();
    if loaded.is_empty() {
        return None;
    }

    let _ = MODELS.set(loaded);
    MODELS.get().map(Vec::as_slice)
}

fn current_models() -> &'static [ModelInfo] {
    cached_models().unwrap_or(&[])
}

#[derive(Debug)]
pub struct VisionServer {
    tool_router: ToolRouter<Self>,
    default_model: Option<String>,
}

impl VisionServer {
    #[must_use]
    pub fn new(default_model: Option<String>) -> Self {
        Self {
            tool_router: Self::tool_router(),
            default_model,
        }
    }

    fn missing_model_error(&self) -> String {
        let guidance = models::guidance_text(current_models());
        format!(
            "Missing required parameter: `model`.\n\
             No default model was set on server startup.\n\n\
             {guidance}"
        )
    }

    fn resolve_model(&self, requested: Option<&str>) -> Result<String, String> {
        match requested.or(self.default_model.as_deref()) {
            Some(m) => Ok(m.to_string()),
            None => Err(self.missing_model_error()),
        }
    }
}

impl Default for VisionServer {
    fn default() -> Self {
        Self::new(None)
    }
}

#[tool_router]
impl VisionServer {
    #[tool(
        name = "describe_image",
        title = "Describe image",
        description = "Have Gemini describe a single image in exhaustive structured detail \
                       (scene, objects, layout, lighting, palette, text, style, fine details). \
                       Input may be a local path or a \
                       `data:<mime>;base64,...` data URI. The `model` parameter is optional \
                       if a default model was configured on server startup; otherwise it is required. \
                       Optionally pass `question` to ask one specific question instead of \
                       the full structured description, and `focus` to steer the structured \
                       description toward an aspect. Input is required to look like an image \
                       unless `force_input_type` is true."
    )]
    async fn describe_image(
        &self,
        params: Parameters<DescribeImageParams>,
    ) -> Result<String, String> {
        let p = params.0;
        let model = match self.resolve_model(p.model.as_deref()) {
            Ok(m) => m,
            Err(e) => return Err(e),
        };
        run_tool(ToolCall::image(&p, &model)).await
    }

    #[tool(
        name = "describe_video",
        title = "Describe video",
        description = "Have Gemini describe a single video in structured detail: a temporal \
                       timeline (relative markers only, no exact timestamps), people & objects, \
                       on-screen text, audio summary, lighting, mood, and fine details. \
                       Input may be a local path or a \
                       `data:<mime>;base64,...` data URI. The `model` parameter is optional \
                       if a default model was configured on server startup; otherwise it is required. \
                       `question` overrides the structured description with a direct Q&A; \
                       `focus` steers it. Videos longer than 5 minutes require \
                       `confirm_long_media: true`. Input is required to look like video \
                       unless `force_input_type` is true."
    )]
    async fn describe_video(
        &self,
        params: Parameters<DescribeVideoParams>,
    ) -> Result<String, String> {
        let p = params.0;
        let model = match self.resolve_model(p.model.as_deref()) {
            Ok(m) => m,
            Err(e) => return Err(e),
        };
        run_tool(ToolCall::video(&p, &model)).await
    }

    #[tool(
        name = "describe_audio",
        title = "Describe audio",
        description = "Have Gemini describe a single audio recording in structured detail: \
                       speakers, language(s), tone, speech content summary, music genre feel, \
                       sound effects, ambience, and audio quality. Input may be a local path \
                       or a `data:<mime>;base64,...` data URI. The `model` parameter is optional \
                       if a default model was configured on server startup; otherwise it is required. \
                       `question` overrides the structured description \
                       with a direct Q&A; `focus` steers it. Audio longer than 30 minutes \
                       requires `confirm_long_media: true`. Input is required to look like \
                       audio unless `force_input_type: true`."
    )]
    async fn describe_audio(
        &self,
        params: Parameters<DescribeAudioParams>,
    ) -> Result<String, String> {
        let p = params.0;
        let model = match self.resolve_model(p.model.as_deref()) {
            Ok(m) => m,
            Err(e) => return Err(e),
        };
        run_tool(ToolCall::audio(&p, &model)).await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for VisionServer {
    fn get_info(&self) -> ServerInfo {
        let capabilities = ServerCapabilities::builder().enable_tools().build();
        let guidance = models::guidance_text(current_models());
        let instructions = if let Some(ref default_model) = self.default_model {
            format!(
                "Gives text-only LLMs Gemini's vision, video, and audio understanding \
                 via the `agy` CLI (Google Antigravity). Three tools: \
                 `describe_image`, `describe_video`, `describe_audio`. Each accepts a \
                 local path or a `data:<mime>;base64,...` data URI. Remote URLs are \
                 not supported — download the file to a local path first. \
                 By default each returns an exhaustive structured markdown description; \
                 pass `question` to ask one specific question instead, or `focus` to \
                 steer the structured description toward an aspect. Each tool validates \
                 that the input matches its modality; pass `force_input_type: true` only \
                 when you intentionally want to override that guard.\n\n\
                 The `model` parameter is optional (default: `{default_model}`). No API key \
                 needed — `agy` must be installed and signed in.\n\n\
                 {guidance}"
            )
        } else {
            format!(
                "Gives text-only LLMs Gemini's vision, video, and audio understanding \
                 via the `agy` CLI (Google Antigravity). Three tools: \
                 `describe_image`, `describe_video`, `describe_audio`. Each accepts a \
                 local path or a `data:<mime>;base64,...` data URI. Remote URLs are \
                 not supported — download the file to a local path first. \
                 By default each returns an exhaustive structured markdown description; \
                 pass `question` to ask one specific question instead, or `focus` to \
                 steer the structured description toward an aspect. Each tool validates \
                 that the input matches its modality; pass `force_input_type: true` only \
                 when you intentionally want to override that guard.\n\n\
                 The `model` parameter is REQUIRED on every call unless a default model was \
                 configured on server startup via `--model`. No API key needed — `agy` must \
                 be installed and signed in.\n\n\
                 {guidance}"
            )
        };
        ServerInfo::new(capabilities)
            .with_server_info(Implementation::new(
                "gemini-multimodal-mcp",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(instructions)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DescribeImageParams {
    /// Local image path or `data:<mime>;base64,...` URI.
    pub image: String,
    /// Required agy model name unless a default model is configured on startup.
    pub model: Option<String>,
    /// Aspect to emphasize in the structured prompt.
    pub focus: Option<String>,
    /// Direct question that replaces the structured prompt.
    pub question: Option<String>,
    /// Allow non-image or unknown input type.
    #[serde(default)]
    pub force_input_type: bool,
    /// Allow long or unknown-duration forced audio/video input.
    #[serde(default)]
    pub confirm_long_media: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DescribeVideoParams {
    /// Local video path or `data:<mime>;base64,...` URI.
    pub video: String,
    /// Required agy model name unless a default model is configured on startup.
    pub model: Option<String>,
    /// Aspect to emphasize in the structured prompt.
    pub focus: Option<String>,
    /// Direct question that replaces the structured prompt.
    pub question: Option<String>,
    /// Allow videos longer than 5 minutes or unknown duration.
    #[serde(default)]
    pub confirm_long_media: bool,
    /// Allow non-video or unknown input type.
    #[serde(default)]
    pub force_input_type: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DescribeAudioParams {
    /// Local audio path or `data:<mime>;base64,...` URI.
    pub audio: String,
    /// Required agy model name unless a default model is configured on startup.
    pub model: Option<String>,
    /// Aspect to emphasize in the structured prompt.
    pub focus: Option<String>,
    /// Direct question that replaces the structured prompt.
    pub question: Option<String>,
    /// Allow audio longer than 30 minutes or unknown duration.
    #[serde(default)]
    pub confirm_long_media: bool,
    /// Allow non-audio or unknown input type.
    #[serde(default)]
    pub force_input_type: bool,
}

struct ToolCall<'a> {
    modality: Modality,
    input: &'a str,
    focus: Option<&'a str>,
    question: Option<&'a str>,
    model: &'a str,
    deadline: Duration,
    confirm_long_media: bool,
    force_input_type: bool,
}

async fn run_tool(call: ToolCall<'_>) -> Result<String, String> {
    call.run().await.map_err(|e| e.to_string())
}

impl<'a> ToolCall<'a> {
    fn image(params: &'a DescribeImageParams, model: &'a str) -> Self {
        Self {
            modality: Modality::Image,
            input: &params.image,
            focus: params.focus.as_deref(),
            question: params.question.as_deref(),
            model,
            deadline: agy::DEFAULT_TIMEOUT,
            confirm_long_media: params.confirm_long_media,
            force_input_type: params.force_input_type,
        }
    }

    fn video(params: &'a DescribeVideoParams, model: &'a str) -> Self {
        Self {
            modality: Modality::Video,
            input: &params.video,
            focus: params.focus.as_deref(),
            question: params.question.as_deref(),
            model,
            deadline: agy::VIDEO_TIMEOUT,
            confirm_long_media: params.confirm_long_media,
            force_input_type: params.force_input_type,
        }
    }

    fn audio(params: &'a DescribeAudioParams, model: &'a str) -> Self {
        Self {
            modality: Modality::Audio,
            input: &params.audio,
            focus: params.focus.as_deref(),
            question: params.question.as_deref(),
            model,
            deadline: agy::DEFAULT_TIMEOUT,
            confirm_long_media: params.confirm_long_media,
            force_input_type: params.force_input_type,
        }
    }

    async fn run(self) -> Result<String, AppError> {
        let resolved: ResolvedInput = input::resolve(self.input).await?;

        let media_probe = probe::probe_media(resolved.local_path());
        self.validate_input_type(&media_probe)?;
        self.enforce_long_media_policy(&media_probe)?;

        models::validate(current_models(), self.model).map_err(AppError::Input)?;

        let file_ref = resolved.file_ref();
        let prompt = prompts::render(self.modality, self.focus, self.question, &file_ref);
        agy::run(&prompt, self.model, self.deadline).await
    }

    fn expected_kind(&self) -> MediaKind {
        match self.modality {
            Modality::Image => MediaKind::Image,
            Modality::Video => MediaKind::Video,
            Modality::Audio => MediaKind::Audio,
        }
    }

    fn validate_input_type(&self, probe: &MediaProbe) -> Result<(), AppError> {
        let expected = self.expected_kind();
        match probe.kind {
            Some(actual) if actual == expected || self.force_input_type => Ok(()),
            Some(actual) => Err(AppError::InputTypeMismatch {
                expected: expected.as_str(),
                actual: actual.as_str(),
            }),
            None if self.force_input_type => Ok(()),
            None => Err(AppError::UnknownInputType {
                expected: expected.as_str(),
            }),
        }
    }

    fn enforce_long_media_policy(&self, probe: &MediaProbe) -> Result<(), AppError> {
        let policy_kind = probe.kind.unwrap_or_else(|| self.expected_kind());
        let Some(threshold) = long_threshold(policy_kind) else {
            return Ok(());
        };

        match probe.duration {
            Some(dur) if dur > threshold && !self.confirm_long_media => Err(AppError::LongMedia {
                secs: dur.as_secs(),
                threshold: threshold.as_secs(),
            }),
            Some(_) => Ok(()),
            None if self.confirm_long_media => Ok(()),
            None => Err(AppError::UnknownDuration),
        }
    }
}

fn long_threshold(kind: MediaKind) -> Option<Duration> {
    match kind {
        MediaKind::Audio => Some(AUDIO_LONG_THRESHOLD),
        MediaKind::Video => Some(VIDEO_LONG_THRESHOLD),
        MediaKind::Image => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(
        modality: Modality,
        force_input_type: bool,
        confirm_long_media: bool,
    ) -> ToolCall<'static> {
        ToolCall {
            modality,
            input: "",
            focus: None,
            question: None,
            model: "unused",
            deadline: Duration::from_secs(1),
            confirm_long_media,
            force_input_type,
        }
    }

    #[test]
    fn rejects_mismatched_input_type_without_force() {
        let probe = MediaProbe {
            kind: Some(MediaKind::Video),
            duration: Some(Duration::from_secs(1)),
        };
        let err = call(Modality::Image, false, false)
            .validate_input_type(&probe)
            .unwrap_err();
        assert!(matches!(err, AppError::InputTypeMismatch { .. }));
    }

    #[test]
    fn allows_mismatched_input_type_with_force() {
        let probe = MediaProbe {
            kind: Some(MediaKind::Audio),
            duration: Some(Duration::from_secs(1)),
        };
        assert!(
            call(Modality::Video, true, false)
                .validate_input_type(&probe)
                .is_ok()
        );
    }

    #[test]
    fn requires_confirm_when_duration_is_unknown() {
        let probe = MediaProbe {
            kind: Some(MediaKind::Video),
            duration: None,
        };
        let err = call(Modality::Video, false, false)
            .enforce_long_media_policy(&probe)
            .unwrap_err();
        assert!(matches!(err, AppError::UnknownDuration));

        assert!(
            call(Modality::Video, false, true)
                .enforce_long_media_policy(&probe)
                .is_ok()
        );
    }

    #[test]
    fn forced_mismatch_still_enforces_actual_video_duration() {
        let probe = MediaProbe {
            kind: Some(MediaKind::Video),
            duration: Some(VIDEO_LONG_THRESHOLD + Duration::from_secs(1)),
        };
        let err = call(Modality::Image, true, false)
            .enforce_long_media_policy(&probe)
            .unwrap_err();
        assert!(matches!(err, AppError::LongMedia { .. }));
    }

    #[test]
    fn resolves_model_successfully_when_provided() {
        let server = VisionServer::new(Some("default-model".to_string()));
        assert_eq!(server.resolve_model(Some("requested-model")).unwrap(), "requested-model");

        let server_no_default = VisionServer::new(None);
        assert_eq!(server_no_default.resolve_model(Some("requested-model")).unwrap(), "requested-model");
    }

    #[test]
    fn resolves_model_successfully_from_default() {
        let server = VisionServer::new(Some("default-model".to_string()));
        assert_eq!(server.resolve_model(None).unwrap(), "default-model");
    }

    #[test]
    fn fails_resolving_model_when_missing_and_no_default() {
        let server = VisionServer::new(None);
        let err = server.resolve_model(None).unwrap_err();
        assert!(err.contains("Missing required parameter: `model`"));
    }
}
