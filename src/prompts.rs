use std::fmt::Write as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modality {
    Image,
    Video,
    Audio,
}

const HARD_CONSTRAINTS: &str = "\
You are operating in PERCEPTION-ONLY mode. Hard constraints:
- Your ONLY allowed action is to use a file-viewing tool to read the
  single file given below, so its content enters your multimodal
  context. Call that tool exactly once on the given path, then stop
  using tools.
- Do NOT write, edit, create, or execute any code or files.
- Do NOT search the web. Do NOT list or explore the workspace or any
  directory. Do NOT run shell commands.
- Do NOT reference any file other than the single one given below.
- Do NOT attempt to compute quantitative values (no code, no exact
  hex codes, no exact Hz, no exact BPM, no exact pixel counts,
  no exact timestamps or durations in seconds).
- After viewing the file, answer ONLY from what you now perceive in
  it. If something cannot be determined by direct perception, say
  \"uncertain\" rather than guessing or computing.";

const IMAGE_BODY: &str = "\
Describe this image in exhaustive, structured detail. Cover ALL
categories below; for any that do not apply, write \"N/A\" rather than
inventing content.

1. Overall scene & setting
   - What is happening, where, and (only if cues exist) when.
2. Every visible object & person
   - For each: position (left/center/right, foreground/midground/
     background), rough size relative to the frame, color (named,
     not hex), texture, and—for people—pose, gaze direction, facial
     expression, clothing, accessories.
3. Spatial layout & composition
   - Foreground / midground / background layers; leading lines,
     framing, symmetry, depth of field.
4. Lighting
   - Direction, hardness (hard/soft), warmth (warm/neutral/cool),
     shadow quality, and any time-of-day or artificial-light cues.
5. Color palette
   - Name the 3-5 dominant tones in plain language (e.g. \"muted
     teal\", \"warm cream\"). No hex codes.
6. Visible text, symbols, numbers
   - Transcribe VERBATIM and give each item's location. If none,
     write \"None\".
7. Style, mood & atmosphere
   - Photographic / illustrative / diagrammatic / screenshot;
     emotional tone; likely intended audience if obvious.
8. Notable details a casual viewer would miss

MODALITY-SPECIFIC ADAPTATION:
- Chart / diagram / schematic / UI screenshot: prioritize structure,
  axes, labels, values, and layout hierarchy over photographic
  qualities. Transcribe all readable data points.
- Document / screenshot of text: transcribe the text faithfully first,
  then describe layout.
- Meme or image with overlaid text: transcribe the overlay and explain
  the juxtaposition with the underlying image.

Format: markdown with an H3 (###) per category. Be concrete; avoid
vague adjectives like \"nice\" or \"interesting\". When an interpretation
is inferred rather than directly visible, mark it \"(inferred)\".";

const VIDEO_BODY: &str = "\
Describe this video in exhaustive, structured detail. Cover ALL
categories below; for any that do not apply, write \"N/A\".

1. Overall summary
   - One paragraph: what happens, setting, duration feel (short clip /
     long segment — do NOT guess exact seconds).
2. Visual timeline
   - Walk through the video in temporal order. Use relative markers
     (\"at the start\", \"about a quarter in\", \"around halfway\",
     \"near three-quarters\", \"at the end\"). For each segment: what is
     on screen, camera movement (static / pan / zoom / handheld),
     and any notable change.
3. People & objects
   - Every recurring or significant person/object: appearance, color
     (named, not hex), position, and how they move or change over time.
4. On-screen text & graphics
   - Transcribe VERBATIM with the moment they appear (relative time).
     Titles, subtitles, captions, UI overlays, watermarks. If none,
     write \"None\".
5. Audio description
   - Speech: who seems to be speaking, language(s), approximate tone
     and pace (do NOT transcribe unless it is short and clearly
     legible — instead summarize content).
   - Non-speech: music (genre feel, mood), sound effects, ambient
     environment. Do NOT guess BPM or exact instruments if unclear.
6. Lighting & color
   - Lighting quality and color palette in plain language. Note any
     shifts between scenes.
7. Mood, style & likely genre
   - Documentary / vlog / animation / tutorial / security cam / etc.
8. Notable details a casual viewer would miss

Format: markdown with an H3 (###) per category. Use relative temporal
markers everywhere; never invent exact timestamps. Mark inferences
with \"(inferred)\".";

const AUDIO_BODY: &str = "\
Describe this audio recording in exhaustive, structured detail. Cover
ALL categories below; for any that do not apply, write \"N/A\".

1. Overall summary
   - One paragraph: what kind of recording this is (speech / music /
     ambient / mixed), general character, rough length feel (short /
     medium / long — do NOT guess exact seconds).
2. Speech (if present)
   - Number of distinct speakers and roughly when each is active
     (relative markers: \"at the start\", \"midway\", etc.).
   - Language(s) heard. If multiple, note switches.
   - For each speaker: apparent gender/age feel, tone, pace, emotion.
   - Content: summarize what is being said. Do NOT attempt a full
     verbatim transcript unless the recording is very short and the
     speech is clearly legible; instead give a faithful summary with
     key phrases quoted when they matter.
   - If no speech: write \"No speech\".
3. Music (if present)
   - Genre feel, mood, instrumentation in plain language (e.g.
     \"soft piano\", \"synth pad\"). Do NOT guess exact BPM or key.
   - Sections / changes over time (intro, verse, build, etc., in
     relative terms).
   - If no music: write \"No music\".
4. Sound effects & events
   - Discrete events (door slam, dog bark, notification chime, etc.)
     with relative timing and a plain-language description.
5. Ambient / background
   - Environment cues (outdoor traffic, cafe murmur, room tone,
     wind, etc.) and how steady or changing they are.
6. Audio quality
   - Clarity, presence of noise/static/clipping/reverb. 
   Do NOT guess sample rate or bit depth.
7. Notable details a casual listener would miss

Format: markdown with an H3 (###) per category. Use relative temporal
markers throughout; never invent exact timestamps, durations, Hz, or
BPM. Mark inferences with \"(inferred)\".";

#[allow(clippy::needless_pass_by_value)]
pub fn render(
    modality: Modality,
    focus: Option<&str>,
    question: Option<&str>,
    file_ref: &str,
) -> String {
    let mut out = String::with_capacity(4096);

    let _ = writeln!(out, "{HARD_CONSTRAINTS}");
    out.push('\n');

    if let Some(q) = question {
        write_question_prompt(&mut out, q, file_ref);
        return out;
    }

    write_structured_prompt(&mut out, modality, focus, file_ref);
    out
}

fn write_question_prompt(out: &mut String, question: &str, file_ref: &str) {
    let _ = writeln!(out, "{question}");
    out.push('\n');
    write_file_ref(out, file_ref);
}

fn write_structured_prompt(
    out: &mut String,
    modality: Modality,
    focus: Option<&str>,
    file_ref: &str,
) {
    out.push_str(body_for(modality));
    out.push('\n');

    if let Some(f) = focus {
        out.push('\n');
        let _ = writeln!(
            out,
            "The caller is specifically interested in: {f}. Still complete all\n\
             categories, but give extra attention to this aspect within the\n\
             relevant categories."
        );
    }

    out.push('\n');
    write_file_ref(out, file_ref);
}

fn body_for(modality: Modality) -> &'static str {
    match modality {
        Modality::Image => IMAGE_BODY,
        Modality::Video => VIDEO_BODY,
        Modality::Audio => AUDIO_BODY,
    }
}

fn write_file_ref(out: &mut String, file_ref: &str) {
    let path = file_ref.strip_prefix('@').unwrap_or(file_ref);
    let _ = writeln!(out, "The file to view (call your file-viewing tool on this exact path): {path}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_image_prompt_has_all_eight_categories_and_file_ref() {
        let p = render(Modality::Image, None, None, "@/tmp/x.png");
        for n in 1..=8 {
            let header = format!("{n}.");
            assert!(p.contains(&header), "missing category {n}");
        }
        assert!(p.contains("/tmp/x.png"));
        assert!(p.contains("PERCEPTION-ONLY"));
    }

    #[test]
    fn question_override_skips_default_body() {
        let p = render(
            Modality::Image,
            None,
            Some("Is there a red hat?"),
            "@/tmp/x.png",
        );
        assert!(p.contains("Is there a red hat?"));
        assert!(p.contains("PERCEPTION-ONLY"));
        assert!(!p.contains("Every visible object & person"));
    }

    #[test]
    fn focus_block_is_only_inserted_when_focus_given() {
        let with_focus = render(Modality::Video, Some("the dog"), None, "@/tmp/x.mp4");
        assert!(with_focus.contains("specifically interested in: the dog"));

        let without = render(Modality::Video, None, None, "@/tmp/x.mp4");
        assert!(!without.contains("specifically interested in"));
    }
}
