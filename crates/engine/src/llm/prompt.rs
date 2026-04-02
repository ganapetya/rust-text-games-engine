use serde_json::json;
use shakti_game_domain::{GameConfig, GameDefinition, LearningItem};

/// System prompt: single JSON object with passage + hard word character spans + fake words.
/// `target_language` is the BCP-47 / locale code from the session (e.g. `no`); the model must write only in that language.
pub fn passage_gap_system_prompt(max_passage_words: u32, target_language: &str) -> String {
    format!(
        r#"You write coherent prose for a language-learning gap-fill game.

Respond with ONLY valid JSON (no markdown fences, no commentary) matching exactly:
{{"schema_version":1,"full_text":"...","hard_words":[{{"id":0,"start_char":0,"end_char":0,"surface":"..."}}],"fake_words":["..."]}}

Rules:
- full_text must be at most {max_passage_words} words.
- **Language:** Write full_text entirely in language "{target_language}" (the same language as the learner session). Do not switch languages or translate the passage to another language.
- **Originality:** full_text must be **new original prose** (one continuous story or article). Use `learning_items` in the user message only as **inspiration** (themes, facts, tone). Do **not** paste, quote, lightly edit, or concatenate `source_text` / `context_text` from learning_items to form full_text.
- **Gaps (authoritative):** `hard_words` lists the **actual** gaps for the game. Each entry's `surface` must be a **verbatim** substring of `full_text`. You may **omit** some entries from `registered_hard_words`, use **synonyms**, or add other gap words — the learner plays whatever you put in `hard_words`. If the same word form appears multiple times in full_text and should be gapped multiple times, include **one hard_words entry per occurrence** (same `surface`, distinct id / span).
- **Suggested vocabulary:** `registered_hard_words` are **hints only**; weave as many as fit naturally, but the puzzle is fully defined by your `hard_words` + `full_text`.
- hard_words: ids are distinct small integers for each gap (reading order does not need to match id order; spans must be correct).
- **Character indices:** start_char and end_char count **Unicode scalar values** (not UTF-8 bytes). Each `surface` must equal the slice `full_text[start_char..end_char]` under that counting (important for Norwegian å, ø, æ).
- fake_words: plausible distractors for gaps; should not duplicate any hard word surface."#
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
        "passage_authoring": {
            "target_language": language,
            "goal": "Generate one original passage in target_language and choose realistic gap words for the game.",
            "must_not": [
                "Use learning_items source_text or context_text as the passage body or as consecutive stitched paragraphs.",
                "Change language away from target_language."
            ],
            "must": [
                "Include only substrings of full_text in hard_words (each surface must appear exactly at the given span in full_text).",
                "Keep full_text within the max word count from the system message."
            ],
            "optional_hints": "registered_hard_words are suggested vocabulary; you may skip some, substitute similar words, or add gaps the learner did not list — the final puzzle is your hard_words array."
        }
    })
}
