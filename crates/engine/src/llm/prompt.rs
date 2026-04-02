use serde_json::json;
use shakti_game_domain::{GameConfig, GameDefinition, LearningItem};

/// System prompt: single JSON object with passage + hard word character spans + fake words.
pub fn passage_gap_system_prompt(max_passage_words: u32) -> String {
    format!(
        r#"You write coherent prose for a language-learning gap-fill game.

Respond with ONLY valid JSON (no markdown fences, no commentary) matching exactly:
{{"schema_version":1,"full_text":"...","hard_words":[{{"id":0,"start_char":0,"end_char":0,"surface":"..."}}],"fake_words":["..."]}}

Rules:
- full_text must be at most {max_passage_words} words.
- hard_words: each entry is a hard vocabulary word that appears in full_text as the substring full_text[start_char..end_char] (Unicode scalar indices); surface must match that slice exactly.
- ids are distinct small integers matching the word identity.
- fake_words: plausible distractors for gaps; should not duplicate any hard word surface.
- The topic should relate to summaries of the provided learning items (history snippets)."#
    )
}

/// User payload: items, registered words, target language, definition hints.
pub fn passage_gap_user_message_json(
    items: &[LearningItem],
    registered_hard_words: &[String],
    language: &str,
    definition: &GameDefinition,
) -> serde_json::Value {
    let GameConfig::GapFill(gap) = &definition.config;
    json!({
        "language": language,
        "learning_items": items,
        "registered_hard_words": registered_hard_words,
        "distractors_per_gap": gap.distractors_per_gap,
        "scoring_mode": gap.scoring_mode,
    })
}
