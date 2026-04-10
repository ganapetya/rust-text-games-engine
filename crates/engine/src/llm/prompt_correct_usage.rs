//! LLM prompts for the “choose correct usage” game (one A/B/C puzzle per hard word).

use serde_json::json;
use shakti_game_domain::{CorrectUsageConfig, LearningItem};

/// System prompt: JSON with `puzzles[]` aligned to `registered_hard_words` order.
pub fn correct_usage_system_prompt(cfg: &CorrectUsageConfig, target_language: &str) -> String {
    let max_w = cfg.max_sentence_words;
    format!(
        r#"You write data for a language-learning game: for each vocabulary word, three short sentences in which that **exact listed form** appears — exactly one sentence uses it in a **fully grammatical, natural** way; the other two are **incorrect but plausible** because of **context** (wrong word order, odd placement, agreement or syntax errors, unnatural collocation, wrong register)—not because you swapped in a different word shape.

Respond with ONLY valid JSON (no markdown fences, no commentary) matching exactly:
{{"schema_version":1,"puzzles":[{{"word":"...","sentences":["...","...","..."],"correct_index":0}}]}}

Rules:
- **Order:** `puzzles` must have the **same length** and **same order** as `registered_hard_words` in the user message — exactly **one puzzle object per word**, same `word` string as listed (after trim).
- **Per puzzle:** `sentences` must have **exactly 3** strings. `correct_index` is **0, 1, or 2** — the index of the **only** grammatically correct sentence in `sentences`.
- **Language:** Every sentence must be entirely in "{target_language}" (the learner session language). Do not translate to another language.
- **Same surface in every sentence:** Each of the three sentences must include the puzzle `word` with the **exact same spelling and Unicode characters** as in `registered_hard_words` (same letters/diacritics; punctuation beside it is fine, e.g. `happy,` or `(happy)`). **All three options use that same token**—correct and incorrect alike.
- **Wrong answers = bad context, not a different word:** Do **not** build distractors by replacing the target with a related form whose spelling differs (e.g. for `happy` do not use `happily`, `happiness`, `unhappy`, `happier`, …). The learner is practicing **usage of the listed surface**; wrong sentences must still contain that surface but use it in a **questionable or ungrammatical** way. (If the language normally requires inflection in a slot, still keep the **listed** characters in the sentence and make the sentence wrong via structure or collocation—do not “fix” the wrong options by morphing the word.)
- **Distinct:** The three sentences must be **pairwise different** (no duplicates).
- **Length:** Each sentence should be at most about {max_w} words (short, clear).
- **Originality:** Use `learning_items` in the user message only as **vague thematic inspiration** (tone, topic). Do **not** paste, quote, lightly edit, or concatenate `source_text` / `context_text` from learning_items.
- **Cohesion:** Across the whole batch, prefer sentences that **loosely share a theme** suggested by the learning context (still original wording)."#
    )
}

/// User JSON: same learning context shape as gap-fill for consistency.
pub fn correct_usage_user_message_json(
    items: &[LearningItem],
    registered_hard_words: &[String],
    language: &str,
    cfg: &CorrectUsageConfig,
) -> serde_json::Value {
    let max_w = cfg.max_sentence_words;
    json!({
        "task": "correct_usage_quiz",
        "language": language,
        "learning_items": items,
        "registered_hard_words": registered_hard_words,
        "max_sentence_words": max_w,
        "must": [
            "Emit puzzles in the same order as registered_hard_words; one entry per word.",
            "For each puzzle: exactly 3 sentences, correct_index in 0..3, all sentences contain the word as substring.",
            "Exactly one sentence per puzzle is fully correct; the other two are wrong but believable.",
            "Incorrect sentences must still use the listed surface form exactly; make them wrong via grammar, order, collocation, or context—not via a different derivative (adverb, noun, different inflection) of the target.",
            format!("Keep each sentence within roughly {max_w} words."),
        ],
        "must_not": [
            "Reorder, drop, or add words relative to registered_hard_words.",
            "Copy or stitch learning_items source_text or context_text into sentences.",
            "Switch away from the target language.",
            "Use duplicate sentences within a puzzle.",
            "Use morphological substitutes as wrong options (e.g. happily/happiness for happy): every sentence must contain the same spelled token as registered_hard_words.",
        ],
    })
}
