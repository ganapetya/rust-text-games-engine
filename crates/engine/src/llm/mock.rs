use async_trait::async_trait;
use shakti_game_domain::{
    GameDefinition, LearningItem, PassageGapLlmOutput, PassageHardWordOccurrence,
    PASSAGE_LLM_SCHEMA_VERSION, UserId,
};

use crate::errors::AppError;
use crate::ports::LlmContentPreparer;

/// Deterministic passage builder for tests and offline runs (no HTTP).
#[derive(Debug, Default, Clone)]
pub struct MockLlmContentPreparer;

fn first_occurrence_chars(haystack: &str, needle: &str) -> Option<(usize, usize)> {
    let hay: Vec<char> = haystack.chars().collect();
    let need: Vec<char> = needle.chars().collect();
    if need.is_empty() {
        return None;
    }
    'outer: for start in 0..hay.len() {
        if start + need.len() > hay.len() {
            break;
        }
        for i in 0..need.len() {
            if hay[start + i] != need[i] {
                continue 'outer;
            }
        }
        return Some((start, start + need.len()));
    }
    None
}

#[async_trait]
impl LlmContentPreparer for MockLlmContentPreparer {
    async fn build_passage_gap_context(
        &self,
        user_id: UserId,
        trace_id: Option<&str>,
        learning_items: &[LearningItem],
        registered_hard_words: &[String],
        language: &str,
        definition: &GameDefinition,
    ) -> Result<PassageGapLlmOutput, AppError> {
        let gap = definition.gap_fill_config().map_err(AppError::from)?;
        let max_gaps = gap.max_llm_gap_slots as usize;

        tracing::info!(
            user_id = %user_id.0,
            trace_id = trace_id.unwrap_or(""),
            mode = "mock",
            items_in = learning_items.len(),
            lang = language,
            "llm passage gap build (mock)"
        );

        let mut full_text = learning_items
            .iter()
            .map(|i| {
                i.context_text
                    .as_deref()
                    .unwrap_or(i.source_text.as_str())
                    .trim()
            })
            .collect::<Vec<_>>()
            .join(" ");
        if full_text.is_empty() {
            full_text = "(tomtekst)".into();
        }

        let mut hard_words = Vec::new();
        let mut next_id = 0u32;
        for w in registered_hard_words {
            if hard_words.len() >= max_gaps {
                break;
            }
            let w = w.trim();
            if w.is_empty() {
                continue;
            }
            if let Some((a, b)) = first_occurrence_chars(&full_text, w) {
                hard_words.push(PassageHardWordOccurrence {
                    id: next_id,
                    start_char: a,
                    end_char: b,
                    surface: w.to_string(),
                });
                next_id += 1;
            } else {
                let before_chars = full_text.chars().count();
                full_text.push(' ');
                full_text.push_str(w);
                let start = before_chars + 1;
                let end = full_text.chars().count();
                hard_words.push(PassageHardWordOccurrence {
                    id: next_id,
                    start_char: start,
                    end_char: end,
                    surface: w.to_string(),
                });
                next_id += 1;
            }
        }

        if hard_words.is_empty() {
            for li in learning_items {
                if hard_words.len() >= max_gaps {
                    break;
                }
                let w = li.hard_fragment.trim();
                if w.is_empty() {
                    continue;
                }
                if let Some((a, b)) = first_occurrence_chars(&full_text, w) {
                    hard_words.push(PassageHardWordOccurrence {
                        id: next_id,
                        start_char: a,
                        end_char: b,
                        surface: w.to_string(),
                    });
                    next_id += 1;
                }
            }
        }

        let out = PassageGapLlmOutput {
            schema_version: PASSAGE_LLM_SCHEMA_VERSION,
            full_text,
            hard_words,
            fake_words: vec![
                "mock_distractor_a".into(),
                "mock_distractor_b".into(),
                "mock_distractor_c".into(),
            ],
        };
        out.validate_against_gap_fill_config(gap)
            .map_err(|e| AppError::LlmPreparation(e.to_string()))?;
        Ok(out)
    }
}
