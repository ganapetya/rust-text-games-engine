//! LLM prompts for the crossword **hints-and-bridges** strategy.
//!
//! The LLM is asked to:
//!   1. Write a short story using the hard words.
//!   2. Supply a one-sentence clue for each hard word.
//!   3. Generate bridge (helper) words with clues.
//!
//! The grid is built by the deterministic placer in `crossword_placer` — the LLM
//! never sees or produces a letter grid.

use serde_json::json;
use shakti_game_domain::{CrosswordConfig, LearningItem};

/// System prompt for crossword hints generation.
pub fn crossword_hints_system_prompt(cfg: &CrosswordConfig, target_language: &str) -> String {
    let max_w = cfg.max_words;
    let max_hint = cfg.max_hint_chars;
    let bridge_count = (max_w as usize * 2).max(16);
    format!(
        r##"You are a language-learning assistant for "{target_language}".

Your task has THREE parts:

1. **Story** – write a short cohesive passage (3-6 sentences) that naturally uses the provided hard words. The story must fit the vocabulary level and language of "{target_language}".

2. **Hard-word hints** – for each hard word the user provides, write exactly one short clue sentence (max {max_hint} characters) that describes the word without naming it.

3. **Bridge words** – generate exactly {bridge_count} additional vocabulary words that:
   - Are real {target_language} words
   - Are 3–12 characters long and use only letters (no hyphens, spaces, or punctuation)
   - Fit the story theme
   - Have diverse letters to maximize crossing opportunities
   - Each has a short clue sentence (max {max_hint} characters)

Respond with **ONLY** valid JSON (no markdown, no extra text):
{{"schema_version":1,"story":"...","hard_word_hints":[{{"word":"WORD","hint":"clue..."}}],"bridge_words":[{{"word":"BRIDGE","hint":"clue..."}}]}}

Rules:
- All `word` values must be **UPPERCASE**.
- `hint` is a clue sentence that describes the meaning without repeating the word.
- Bridge words must NOT duplicate any hard word.
- Bridge words must contain only letters (A-Z or language-specific alphabet letters).
- Do NOT produce a grid, coordinates, or any layout. The placement is done separately."##
    )
}

/// User JSON message for crossword hints.
pub fn crossword_hints_user_message(
    items: &[LearningItem],
    hard_words: &[String],
    language: &str,
    cfg: &CrosswordConfig,
) -> serde_json::Value {
    let bridge_count = (cfg.max_words as usize * 2).max(16);
    let context_snippets: Vec<_> = items
        .iter()
        .map(|li| li.context_text.as_deref().unwrap_or(li.source_text.as_str()).trim())
        .filter(|s| !s.is_empty())
        .take(6)
        .collect();

    json!({
        "task": "crossword_hints_and_bridges",
        "language": language,
        "hard_words": hard_words,
        "context_snippets": context_snippets,
        "requested_bridge_count": bridge_count,
        "max_hint_chars": cfg.max_hint_chars,
        "must": [
            "Emit schema_version 1.",
            "Provide exactly one hard_word_hint per hard word.",
            "Generate exactly the requested number of bridge words.",
            "All words UPPERCASE, letters only.",
            "NO grid or coordinates in output.",
        ],
        "must_not": [
            "Copy context_snippets into story verbatim.",
            "Include hard words in bridge_words.",
            "Use markdown fences or commentary outside JSON.",
        ],
    })
}
