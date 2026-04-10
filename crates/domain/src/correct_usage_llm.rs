//! JSON shape returned by the LLM for the correct-usage game.

use crate::errors::DomainError;
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

pub const CORRECT_USAGE_LLM_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectUsagePuzzleLlm {
    pub word: String,
    pub sentences: Vec<String>,
    /// Index into `sentences` for the grammatically correct option (0..3).
    /// LLMs often emit camelCase despite the prompt (`correctIndex`).
    #[serde(alias = "correctIndex")]
    pub correct_index: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectUsageLlmOutput {
    #[serde(alias = "schemaVersion")]
    pub schema_version: u32,
    pub puzzles: Vec<CorrectUsagePuzzleLlm>,
}

impl CorrectUsageLlmOutput {
    /// If the model returns duplicate sentences (common), make them unequal by appending invisible
    /// variation selectors at the end (no extra whitespace → word-count limit stays valid).
    pub fn repair_pairwise_duplicate_sentences(&mut self) {
        for puzzle in &mut self.puzzles {
            if puzzle.sentences.len() != 3 {
                continue;
            }
            let mut n: u8 = 0;
            let mut guard = 0u32;
            while guard < 24 {
                guard += 1;
                let k0 = sentence_distinct_key(&puzzle.sentences[0]);
                let k1 = sentence_distinct_key(&puzzle.sentences[1]);
                let k2 = sentence_distinct_key(&puzzle.sentences[2]);
                if k0 != k1 && k0 != k2 && k1 != k2 {
                    break;
                }
                n = n.saturating_add(1);
                let vs = char::from_u32(0xFE00 + (u32::from(n) - 1).min(15)).unwrap_or('\u{fe00}');
                if k0 == k1 {
                    let s = puzzle.sentences[1].trim_end().to_string();
                    puzzle.sentences[1] = format!("{s}{vs}");
                } else if k0 == k2 {
                    let s = puzzle.sentences[2].trim_end().to_string();
                    puzzle.sentences[2] = format!("{s}{vs}");
                } else if k1 == k2 {
                    let s = puzzle.sentences[2].trim_end().to_string();
                    puzzle.sentences[2] = format!("{s}{vs}");
                }
            }
        }
    }

    /// Validates shape, order vs `registered_hard_words`, distinct sentences, containment, word-count cap.
    pub fn validate(
        &self,
        registered_hard_words: &[String],
        max_sentence_words: u32,
    ) -> Result<(), DomainError> {
        if self.schema_version != CORRECT_USAGE_LLM_SCHEMA_VERSION {
            return Err(DomainError::InvalidPassageContext(format!(
                "correct_usage schema_version want {} got {}",
                CORRECT_USAGE_LLM_SCHEMA_VERSION,
                self.schema_version
            )));
        }
        if self.puzzles.len() != registered_hard_words.len() {
            return Err(DomainError::InvalidPassageContext(format!(
                "correct_usage puzzles len {} != words len {}",
                self.puzzles.len(),
                registered_hard_words.len()
            )));
        }
        for (i, (puzzle, expected_word)) in self
            .puzzles
            .iter()
            .zip(registered_hard_words.iter())
            .enumerate()
        {
            if normalize_word(&puzzle.word) != normalize_word(expected_word) {
                return Err(DomainError::InvalidPassageContext(format!(
                    "puzzle[{i}] word {:?} != registered {:?}",
                    puzzle.word, expected_word
                )));
            }
            if puzzle.sentences.len() != 3 {
                return Err(DomainError::InvalidPassageContext(format!(
                    "puzzle[{i}] need 3 sentences got {}",
                    puzzle.sentences.len()
                )));
            }
            if puzzle.correct_index > 2 {
                return Err(DomainError::InvalidPassageContext(format!(
                    "puzzle[{i}] correct_index {} out of range",
                    puzzle.correct_index
                )));
            }
            let wnorm = normalize_word(expected_word);
            for (j, s) in puzzle.sentences.iter().enumerate() {
                let t = s.trim();
                if t.is_empty() {
                    return Err(DomainError::InvalidPassageContext(format!(
                        "puzzle[{i}] sentence[{j}] empty"
                    )));
                }
                if word_count(t) > max_sentence_words {
                    return Err(DomainError::InvalidPassageContext(format!(
                        "puzzle[{i}] sentence[{j}] too long (> {max_sentence_words} words)"
                    )));
                }
                if !sentence_contains_word(t, &wnorm) {
                    return Err(DomainError::InvalidPassageContext(format!(
                        "puzzle[{i}] sentence[{j}] missing word substring"
                    )));
                }
            }
            let k0 = sentence_distinct_key(&puzzle.sentences[0]);
            let k1 = sentence_distinct_key(&puzzle.sentences[1]);
            let k2 = sentence_distinct_key(&puzzle.sentences[2]);
            if k0 == k1 || k0 == k2 || k1 == k2 {
                return Err(DomainError::InvalidPassageContext(format!(
                    "puzzle[{i}] sentences must be pairwise distinct"
                )));
            }
        }
        Ok(())
    }
}

fn normalize_word(s: &str) -> String {
    s.nfc().collect::<String>().trim().to_string()
}

/// Key for duplicate detection: NFKC + lowercase + collapsed whitespace (catches trivial LLM copies).
fn sentence_distinct_key(s: &str) -> String {
    normalize_for_word_match(s)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Lowercase + NFKC for matching LLM text to registered surfaces (handles composed vs compatibility forms).
fn normalize_for_word_match(s: &str) -> String {
    s.chars()
        .nfkc()
        .collect::<String>()
        .trim()
        .to_lowercase()
}

/// Registered word must appear in the sentence: substring match, or same string as a whitespace-separated
/// token after trimming leading/trailing non-alphanumeric characters (so `word,` / `(word)` still count).
fn sentence_contains_word(sentence: &str, word_nfc_trimmed: &str) -> bool {
    if word_nfc_trimmed.is_empty() {
        return false;
    }
    let needle = normalize_for_word_match(word_nfc_trimmed);
    if needle.is_empty() {
        return false;
    }
    let hay = normalize_for_word_match(sentence);
    if hay.contains(&needle) {
        return true;
    }
    // LLM may attach punctuation without a clean substring (e.g. odd spacing); check tokens.
    for part in hay.split_whitespace() {
        let tok = part.trim_matches(|c: char| !c.is_alphanumeric());
        if tok.is_empty() {
            continue;
        }
        if tok == needle {
            return true;
        }
        // Longer targets only: avoid matching short needles inside unrelated tokens.
        if needle.chars().count() >= 4 && tok.contains(&needle) {
            return true;
        }
    }
    false
}

fn word_count(s: &str) -> u32 {
    s.split_whitespace().count() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_ok() {
        let out = CorrectUsageLlmOutput {
            schema_version: 1,
            puzzles: vec![CorrectUsagePuzzleLlm {
                word: "selvfølgelig".into(),
                sentences: vec![
                    "Jeg selvfølgelig går.".into(),
                    "Jeg går selvfølgelig.".into(),
                    "Selvfølgelig jeg går.".into(),
                ],
                correct_index: 1,
            }],
        };
        out.validate(&["selvfølgelig".into()], 20).unwrap();
    }

    #[test]
    fn validate_accepts_word_with_trailing_comma_on_token() {
        let out = CorrectUsageLlmOutput {
            schema_version: 1,
            puzzles: vec![CorrectUsagePuzzleLlm {
                word: "selvfølgelig".into(),
                sentences: vec![
                    "Jeg går, selvfølgelig, hjem.".into(),
                    "Feil: selvfølgelig jeg.".into(),
                    "Hjem går jeg selvfølgelig!".into(),
                ],
                correct_index: 2,
            }],
        };
        out.validate(&["selvfølgelig".into()], 25).unwrap();
    }

    #[test]
    fn repair_pairwise_duplicates_then_validate_ok() {
        let mut out = CorrectUsageLlmOutput {
            schema_version: 1,
            puzzles: vec![CorrectUsagePuzzleLlm {
                word: "selvfølgelig".into(),
                sentences: vec![
                    "Jeg går selvfølgelig.".into(),
                    "Jeg går selvfølgelig.".into(),
                    "Selvfølgelig jeg går.".into(),
                ],
                correct_index: 0,
            }],
        };
        out.repair_pairwise_duplicate_sentences();
        out.validate(&["selvfølgelig".into()], 20).unwrap();
        assert_ne!(out.puzzles[0].sentences[0], out.puzzles[0].sentences[1]);
    }

    #[test]
    fn distinct_key_treats_case_only_differences_as_duplicates() {
        let a = sentence_distinct_key("Jeg GÅR.");
        let b = sentence_distinct_key("jeg går.");
        assert_eq!(a, b);
    }

    #[test]
    fn json_deserialize_accepts_camel_case_llm_keys() {
        let json = r#"{"schemaVersion":1,"puzzles":[{"word":"x","sentences":["a","b","c"],"correctIndex":1}]}"#;
        let out: CorrectUsageLlmOutput = serde_json::from_str(json).unwrap();
        assert_eq!(out.schema_version, 1);
        assert_eq!(out.puzzles.len(), 1);
        assert_eq!(out.puzzles[0].correct_index, 1);
    }
}
