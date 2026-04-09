use serde_json::json;
use shakti_game_domain::{GameConfig, GameDefinition, GapFillPassageConfig, LearningItem};

/// System prompt: single JSON object with passage + hard word character spans + fake words.
/// `target_language` is the BCP-47 / locale code from the session (e.g. `no`); the model must write only in that language.
pub fn passage_gap_system_prompt(gap: &GapFillPassageConfig, target_language: &str) -> String {
    let max_passage_words = gap.max_passage_words;
    let max_s = gap.max_llm_sentences;
    let max_g = gap.max_llm_gap_slots;
    format!(
        r#"You write coherent prose for a language-learning gap-fill game.

Respond with ONLY valid JSON (no markdown fences, no commentary) matching exactly:
{{"schema_version":1,"full_text":"...","hard_words":[{{"id":0,"start_char":0,"end_char":0,"surface":"..."}}],"fake_words":["..."]}}

Rules:
- full_text must be at most {max_passage_words} words.
- **Story cohesion:** The passage must read as **one unified mini-story** (or one clear narrative thread). Sentences must **connect logically**—use pronouns, time flow, cause/effect, or natural transitions so each sentence follows from the previous. Avoid a random list of unrelated sentences; the learner should feel a single beginning-to-end arc.
- **Sentences:** full_text must contain **at most {max_s} sentences** (complete thoughts ending with `.`, `?`, or `!` appropriate to `{target_language}`; do not split one idea into extra sentences to bypass this).
- **Gaps — count:** `hard_words` must contain **at most {max_g} entries** (at most **{max_g} words** appear as gaps). If `registered_hard_words` lists more than {max_g} items, create gaps for **at most {max_g}** of them — pick a subset that fits naturally; you may leave other listed words in `full_text` as normal (non-gap) text, or omit them if they cannot fit without exceeding limits.
- **Language:** Write full_text entirely in language "{target_language}" (the same language as the learner session). Do not switch languages or translate the passage to another language.
- **Originality:** full_text must be **new original prose** (one continuous, **cohesive** story or short article). Use `learning_items` in the user message only as **inspiration** (themes, facts, tone). Do **not** paste, quote, lightly edit, or concatenate `source_text` / `context_text` from learning_items to form full_text.
- **Training vocabulary (primary):** `registered_hard_words` are the **words to practise**. Prefer each **verbatim** in `full_text`; up to **{max_g}** of them may be **gaps** (`hard_words`). If there are **{max_g} or fewer** listed words, gap **all** of them (one entry each unless duplicate surfaces — see below). If there are **more than {max_g}**, gap **at most {max_g}** chosen for natural fit; others may appear as plain text or be omitted if limits forbid.
- **Extra gaps (rare):** Only if you **cannot** include a listed word after a reasonable rewrite, you may replace **that single slot** with a minimal alternative gap word still in `{target_language}` — still respect **at most {max_g}** `hard_words` total. Keep such exceptions **few**.
- **Duplicates:** If the same surface appears **more than once** in `full_text` and each occurrence should be a gap, include **one hard_words entry per occurrence** (same `surface`, distinct id / span) **only if** the total number of `hard_words` entries stays **≤ {max_g}**.
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
    let max_s = gap.max_llm_sentences;
    let max_g = gap.max_llm_gap_slots;
    let note = format!(
        "At most {max_s} sentences in full_text; at most {max_g} hard_words entries. If registered_hard_words is longer than {max_g}, gap only up to {max_g} of them."
    );
    let exceptions = format!(
        "When limits conflict with the full vocabulary list, respect the {max_s}-sentence and {max_g}-gap caps first; only when a listed word truly cannot fit within those caps, omit that gap or use a rare substitute per system rules."
    );
    json!({
        "language": language,
        "learning_items": items,
        "registered_hard_words": registered_hard_words,
        "distractors_per_gap": gap.distractors_per_gap,
        "scoring_mode": gap.scoring_mode,
        "passage_authoring": {
            "target_language": language,
            "goal": "Generate one original, cohesive mini-story (connected sentences forming a single narrative) so the learner trains on the supplied hard words as gaps.",
            "length_limits": {
                "max_sentences": max_s,
                "max_gap_words": max_g,
                "note": note
            },
            "must_not": [
                "Use learning_items source_text or context_text as the passage body or as consecutive stitched paragraphs.",
                "Change language away from target_language.",
                "Routinely omit, replace with synonyms, or skip registered_hard_words when the passage could reasonably include them.",
                format!("Exceed {max_s} sentences in full_text or more than {max_g} gap words in hard_words."),
                "Produce disconnected or random sentences that do not read as one flowing story."
            ],
            "must": [
                format!("Keep full_text to at most {max_s} sentences and hard_words to at most {max_g} entries (see system message)."),
                "Make sentences connect into a single coherent story: clear progression, sensible links between sentences, no abrupt jumps unless intentional (e.g. twist).",
                "For each gap in hard_words: exact spelling in full_text and correct Unicode scalar start_char/end_char.",
                format!("If registered_hard_words has {max_g} or fewer items, prefer gapping all of them; if more than {max_g}, gap at most {max_g} and choose the best-fitting subset."),
                "Include only substrings of full_text in hard_words; keep full_text within the max word count from the system message."
            ],
            "exceptions": exceptions
        }
    })
}
