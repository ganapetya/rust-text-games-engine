//! LLM contract for passage-based gap-fill: validated once after model output.

use crate::errors::DomainError;
use crate::policies::GapFillPassageConfig;
use serde::{Deserialize, Serialize};

pub const PASSAGE_LLM_SCHEMA_VERSION: u32 = 1;

/// Root JSON shape returned by the LLM (stored in `game_sessions.base_context` after validation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassageGapLlmOutput {
    pub schema_version: u32,
    pub full_text: String,
    pub hard_words: Vec<PassageHardWordOccurrence>,
    pub fake_words: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassageHardWordOccurrence {
    pub id: u32,
    /// Unicode scalar index in `full_text` (character boundary), start inclusive.
    pub start_char: usize,
    /// Exclusive end index (character boundary).
    pub end_char: usize,
    pub surface: String,
}

impl PassageGapLlmOutput {
    /// Checks spans and word counts; rejects invalid model output before persisting.
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.schema_version != PASSAGE_LLM_SCHEMA_VERSION {
            return Err(DomainError::InvalidPassageContext(format!(
                "schema_version want {PASSAGE_LLM_SCHEMA_VERSION} got {}",
                self.schema_version
            )));
        }
        if self.full_text.is_empty() {
            return Err(DomainError::InvalidPassageContext(
                "full_text empty".into(),
            ));
        }
        if self.hard_words.is_empty() {
            return Err(DomainError::InvalidPassageContext(
                "no hard_words".into(),
            ));
        }
        let chars: Vec<char> = self.full_text.chars().collect();
        let len = chars.len();
        for hw in &self.hard_words {
            if hw.start_char > len || hw.end_char > len || hw.start_char >= hw.end_char {
                return Err(DomainError::InvalidPassageContext(format!(
                    "bad span for id {}: {}..{} (len {})",
                    hw.id, hw.start_char, hw.end_char, len
                )));
            }
            let slice: String = chars[hw.start_char..hw.end_char].iter().collect();
            if slice != hw.surface {
                return Err(DomainError::InvalidPassageContext(format!(
                    "surface mismatch id {}: expected {:?} got {:?}",
                    hw.id, hw.surface, slice
                )));
            }
        }
        Ok(())
    }

    /// Like [`validate`], plus gap-fill definition limits (gap count, passage word count).
    pub fn validate_against_gap_fill_config(&self, cfg: &GapFillPassageConfig) -> Result<(), DomainError> {
        self.validate()?;
        let max_gaps = cfg.max_llm_gap_slots as usize;
        if self.hard_words.len() > max_gaps {
            return Err(DomainError::InvalidPassageContext(format!(
                "hard_words count {} exceeds max_llm_gap_slots ({})",
                self.hard_words.len(),
                cfg.max_llm_gap_slots
            )));
        }
        let word_count = self.full_text.split_whitespace().count() as u32;
        if word_count > cfg.max_passage_words {
            return Err(DomainError::InvalidPassageContext(format!(
                "full_text word count {word_count} exceeds max_passage_words ({})",
                cfg.max_passage_words
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policies::{GapFillPassageConfig, GapFillScoringMode};

    fn sample_output() -> PassageGapLlmOutput {
        PassageGapLlmOutput {
            schema_version: PASSAGE_LLM_SCHEMA_VERSION,
            full_text: "one two three".into(),
            hard_words: vec![PassageHardWordOccurrence {
                id: 0,
                start_char: 0,
                end_char: 3,
                surface: "one".into(),
            }],
            fake_words: vec!["x".into()],
        }
    }

    fn tight_gap_cfg() -> GapFillPassageConfig {
        GapFillPassageConfig {
            max_passage_words: 2,
            distractors_per_gap: 1,
            allow_skip: false,
            scoring_mode: GapFillScoringMode::PerGap,
            max_llm_gap_slots: 1,
            max_llm_sentences: 5,
            max_learning_items_for_llm: 100,
        }
    }

    #[test]
    fn validate_against_rejects_too_many_gaps() {
        let mut p = sample_output();
        p.hard_words.push(PassageHardWordOccurrence {
            id: 1,
            start_char: 4,
            end_char: 7,
            surface: "two".into(),
        });
        let cfg = tight_gap_cfg();
        let err = p.validate_against_gap_fill_config(&cfg).unwrap_err();
        let msg = format!("{err:?}");
        assert!(msg.contains("max_llm_gap_slots"), "{msg}");
    }

    #[test]
    fn validate_against_rejects_word_count_over_max_passage_words() {
        let p = sample_output();
        let cfg = tight_gap_cfg();
        let err = p.validate_against_gap_fill_config(&cfg).unwrap_err();
        let msg = format!("{err:?}");
        assert!(msg.contains("max_passage_words"), "{msg}");
    }
}
