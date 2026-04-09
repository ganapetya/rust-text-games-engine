use serde_json::json;

pub fn translation_system_prompt() -> &'static str {
    r#"You translate text for a language-learning game.

Respond with ONLY valid JSON (no markdown fences, no commentary) matching exactly:
{"translated_text":"..."}

Rules:
- translated_text must be a faithful, natural translation into the target language.
- Preserve tone and register (informal/formal) appropriate to the source.
- Do not add explanations, notes, or the original text — only the translation inside translated_text.
- Escape JSON string characters properly (e.g. newlines as \n)."#
}

pub fn translation_user_message_json(
    source_lang: &str,
    target_lang: &str,
    text: &str,
) -> serde_json::Value {
    json!({
        "source_language": source_lang,
        "target_language": target_lang,
        "text": text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_message_contains_languages_and_text() {
        let v = translation_user_message_json("no", "en", "Hei.");
        assert_eq!(v["source_language"], "no");
        assert_eq!(v["target_language"], "en");
        assert_eq!(v["text"], "Hei.");
    }
}
