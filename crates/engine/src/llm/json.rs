use serde::Deserialize;
use shakti_game_domain::LearningItem;

#[derive(Debug, Deserialize)]
pub struct GapFillLlmJsonEnvelope {
    pub learning_items: Vec<LearningItem>,
}

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

pub fn parse_gap_fill_response(raw: &str) -> Result<Vec<LearningItem>, String> {
    let cleaned = strip_code_fences(raw);
    let env: GapFillLlmJsonEnvelope =
        serde_json::from_str(&cleaned).map_err(|e| format!("invalid LLM JSON: {e}"))?;
    Ok(env.learning_items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn strip_fences() {
        let s = "```json\n{\"learning_items\":[]}\n```";
        assert_eq!(strip_code_fences(s), r#"{"learning_items":[]}"#);
    }

    #[test]
    fn parse_envelope() {
        let id = Uuid::new_v4();
        let uid = Uuid::new_v4();
        let raw = format!(
            r#"{{"learning_items":[{{"id":"{id}","user_id":"{uid}","source_text":"a","context_text":null,"hard_fragment":"x","lemma":null,"language":"no","metadata":{{}}}}]}}"#
        );
        let items = parse_gap_fill_response(&raw).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].hard_fragment, "x");
    }
}
