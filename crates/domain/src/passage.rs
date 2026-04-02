//! LLM contract for passage-based gap-fill: validated once after model output.

use crate::errors::DomainError;
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
}
