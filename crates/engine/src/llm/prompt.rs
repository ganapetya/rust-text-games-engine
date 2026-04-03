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
- **Training vocabulary (primary):** `registered_hard_words` from the user message are the **words the learner must practise**. Your job is to make **every** one of them appear **verbatim** in `full_text` and give **each** a matching entry in `hard_words` (surface equals that exact substring; correct `start_char`/`end_char`). Rewrite the passage as needed so they fit naturally.
- **Extra gaps (rare):** Only if you **cannot** include a listed word after a reasonable rewrite, you may replace **that single slot** with a minimal alternative gap word still in `{target_language}` — keep such exceptions **few** and prefer **not** to skip listed words. Do **not** routinely swap them for synonyms or drop them.
- **Duplicates:** If the same surface appears **more than once** in `full_text` and each occurrence should be a gap, include **one hard_words entry per occurrence** (same `surface`, distinct id / span).
- hard_words: ids are distinct small integers per gap; spans must be correct even if ids are not in left-to-right order.
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
            "goal": "Generate one original passage so the learner trains on the supplied hard words as gaps.",
            "must_not": [
                "Use learning_items source_text or context_text as the passage body or as consecutive stitched paragraphs.",
                "Change language away from target_language.",
                "Routinely omit, replace with synonyms, or skip registered_hard_words when the passage could reasonably include them."
            ],
            "must": [
                "Work every registered_hard_words item into full_text with exact spelling and gap each with a hard_words entry (verbatim surface / correct Unicode scalar spans).",
                "Include only substrings of full_text in hard_words; keep full_text within the max word count from the system message."
            ],
            "exceptions": "Only when a listed word truly cannot fit, allow a sparing substitute gap or omit that one gap — default is always to include all listed words."
        }
    })
}
