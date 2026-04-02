use shakti_game_domain::PassageGapLlmOutput;

/// Removes optional ```json ... ``` fences from model output.
pub fn strip_code_fences(s: &str) -> String {
    let t = s.trim();
    if let Some(rest) = t.strip_prefix("```") {
        let rest = rest.trim_start();
        let rest = rest.strip_prefix("json").unwrap_or(rest).trim_start();
        if let Some(idx) = rest.rfind("```") {
            return rest[..idx].trim().to_string();
        }
        return rest.trim().to_string();
    }
    t.to_string()
}

/// Parses and returns [`PassageGapLlmOutput`] (caller runs [`PassageGapLlmOutput::validate`]).
pub fn parse_passage_gap_response(raw: &str) -> Result<PassageGapLlmOutput, String> {
    let cleaned = strip_code_fences(raw);
    serde_json::from_str::<PassageGapLlmOutput>(&cleaned)
        .map_err(|e| format!("invalid LLM JSON: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakti_game_domain::PASSAGE_LLM_SCHEMA_VERSION;

    #[test]
    fn strip_fences() {
        let s = "```json\n{\"schema_version\":1}\n```";
        assert!(strip_code_fences(s).contains("schema_version"));
    }

    #[test]
    fn parse_passage() {
        let raw = r#"{"schema_version":1,"full_text":"ab","hard_words":[{"id":0,"start_char":0,"end_char":1,"surface":"a"}],"fake_words":["z"]}"#;
        let p = parse_passage_gap_response(raw).unwrap();
        assert_eq!(p.schema_version, PASSAGE_LLM_SCHEMA_VERSION);
        p.validate().unwrap();
    }
}
