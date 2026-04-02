use serde_json::json;
use shakti_game_domain::{GapFillConfig, LearningItem};

pub fn gap_fill_system_prompt() -> &'static str {
    r#"You are a content preparation assistant for a language-learning gap-fill game.
You receive JSON describing learning items (source text, hard fragment to hide, language, etc.).

Respond with ONLY valid JSON (no markdown fences, no commentary) matching this shape:
{"learning_items":[<LearningItem>,...]}

Each LearningItem must include: id, user_id, source_text, context_text (nullable), hard_fragment, lemma (nullable), language, metadata (object).
Preserve id and user_id from the input. You may enrich context_text, lemma, or metadata for pedagogy.
The caller will use at most `steps_count` items in order; ensure the first `steps_count` items are the best choices if you reorder."#
}

/// User message payload: items + config hints for the model.
pub fn gap_fill_user_message_json(
    items: &[LearningItem],
    config: &GapFillConfig,
) -> serde_json::Value {
    json!({
        "learning_items": items,
        "steps_count": config.steps_count,
        "distractors_per_step": config.distractors_per_step,
        "allow_skip": config.allow_skip,
    })
}
