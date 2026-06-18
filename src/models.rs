use std::process::Command;

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub name: String,
    pub tier: ModelTier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelTier {
    Flash,
    Pro,
    Other,
}

#[must_use]
pub fn list_models() -> Vec<ModelInfo> {
    let output = Command::new("agy").arg("models").output();
    match output {
        Ok(out) if out.status.success() => parse_models(&String::from_utf8_lossy(&out.stdout)),
        _ => Vec::new(),
    }
}

fn parse_models(stdout: &str) -> Vec<ModelInfo> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|name| ModelInfo {
            name: name.to_string(),
            tier: classify(name),
        })
        .collect()
}

fn classify(name: &str) -> ModelTier {
    let lower = name.to_ascii_lowercase();
    if lower.contains("flash") {
        ModelTier::Flash
    } else if lower.contains("pro") {
        ModelTier::Pro
    } else {
        ModelTier::Other
    }
}

#[must_use]
pub fn guidance_text(models: &[ModelInfo]) -> String {
    let mut lines = Vec::new();

    if models.is_empty() {
        lines.push(
            "WARNING: `agy models` could not be parsed (is agy installed and \
             authenticated?). Model validation is disabled; any model string \
             will be passed through to agy."
                .to_string(),
        );
        return lines.join("\n");
    }

    lines.push("Available models (you MUST choose one explicitly):".to_string());
    for m in models {
        lines.push(format!("  • {}{}", m.name, tier_hint(m.tier)));
    }
    lines.push(
        "\nGuidance: prefer Flash for routine description tasks. Switch to Pro \
         only when the task requires real-world knowledge or high-precision \
         transcription. Do NOT use Other-tier models for this MCP."
            .to_string(),
    );
    lines.join("\n")
}

fn tier_hint(tier: ModelTier) -> &'static str {
    match tier {
        ModelTier::Flash => " — Flash: cheaper & faster, good default for most tasks",
        ModelTier::Pro => {
            " — Pro: use for world knowledge (landmarks, cities, brand names, public figures) or complex tasks like precise lyric transcription"
        }
        ModelTier::Other => " — non-Gemini model; multimodal support through agy is unverified",
    }
}

pub fn validate(models: &[ModelInfo], requested: &str) -> Result<(), String> {
    if models.is_empty() {
        return Ok(());
    }
    if models.iter().any(|m| m.name == requested) {
        return Ok(());
    }
    let valid = models
        .iter()
        .map(|m| format!("  • {}", m.name))
        .collect::<Vec<_>>()
        .join("\n");
    Err(format!(
        "Unknown model: \"{requested}\".\n\
         Valid models:\n{valid}\n\n{}",
        guidance_text(models)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_classifies_models() {
        let stdout = "Gemini 3.5 Flash (Medium)\n\
                      Gemini 3.5 Flash (High)\n\
                      Gemini 3.5 Flash (Low)\n\
                      Gemini 3.1 Pro (Low)\n\
                      Gemini 3.1 Pro (High)\n\
                      Claude Sonnet 4.6 (Thinking)\n\
                      Claude Opus 4.6 (Thinking)\n\
                      GPT-OSS 120B (Medium)\n";
        let models = parse_models(stdout);
        assert_eq!(models.len(), 8);
        assert_eq!(models[0].tier, ModelTier::Flash);
        assert_eq!(models[3].tier, ModelTier::Pro);
        assert_eq!(models[5].tier, ModelTier::Other);
        assert_eq!(models[6].tier, ModelTier::Other);
    }

    #[test]
    fn validate_rejects_unknown_and_accepts_known() {
        let models = parse_models("Gemini 3.5 Flash (High)\nGemini 3.1 Pro (Low)\n");
        assert!(validate(&models, "Gemini 3.5 Flash (High)").is_ok());
        assert!(validate(&models, "Gemini 3.1 Pro (Low)").is_ok());
        let err = validate(&models, "gpt-4").unwrap_err();
        assert!(err.contains("Unknown model"));
        assert!(err.contains("Gemini 3.5 Flash (High)"));
    }

    #[test]
    fn validate_passes_through_when_list_empty() {
        assert!(validate(&[], "anything").is_ok());
    }

    #[test]
    fn guidance_text_lists_all_models_with_hints() {
        let models = parse_models("Gemini 3.5 Flash (High)\nGemini 3.1 Pro (Low)\n");
        let g = guidance_text(&models);
        assert!(g.contains("Gemini 3.5 Flash (High)"));
        assert!(g.contains("cheaper"));
        assert!(g.contains("world knowledge"));
    }

    #[test]
    fn guidance_text_warns_when_empty() {
        let g = guidance_text(&[]);
        assert!(g.contains("WARNING"));
        assert!(g.contains("validation is disabled"));
    }
}
