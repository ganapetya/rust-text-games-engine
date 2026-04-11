//! Single-step crossword: LLM grid + hints; player fills cells; score per word.

use crate::answer::{
    CrosswordExpectedWord, EvaluationMode, ExpectedAnswer, StepEvaluation, UserAnswer,
};
use crate::content::{ContentProvenance, LearningItem, PreparedContent, PreparedItem};
use crate::crossword::{CrosswordDirection, CrosswordLlmOutput, CROSSWORD_BLOCK};
use crate::engine::GameEngine;
use crate::errors::DomainError;
use crate::game_session::GameSession;
use crate::game_step::{CrosswordWordPublic, GameStep, StepState, UserFacingStepPrompt};
use crate::ids::GameStepId;
use crate::policies::{GameDefinition, GameKind, ScoringPolicy};
use crate::result::GameResult;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use time::OffsetDateTime;
use uuid::Uuid;

pub struct CrosswordEngine;

impl CrosswordEngine {
    pub fn new() -> Self {
        Self
    }

    fn normalize(s: &str) -> String {
        s.trim().to_lowercase()
    }

    fn is_rtl_language(language: &str) -> bool {
        let l = language.trim().to_ascii_lowercase();
        l.starts_with("ar")
            || l.starts_with("he")
            || l.starts_with("iw")
            || l.starts_with("fa")
            || l.starts_with("ur")
            || l.starts_with("yi")
    }

    fn difficulty_prefill_count(word_count: usize, difficulty: u8) -> usize {
        match difficulty.min(3).max(1) {
            1 => (word_count + 1) / 2,
            2 => (word_count + 3) / 4,
            _ => 0,
        }
    }

    fn pick_prefilled_word_ids(ids: &[u32], k: usize, seed: u64) -> HashSet<u32> {
        if k == 0 || ids.is_empty() {
            return HashSet::new();
        }
        let mut scored: Vec<(u64, u32)> = ids
            .iter()
            .map(|id| {
                let mut h = DefaultHasher::new();
                seed.hash(&mut h);
                id.hash(&mut h);
                (h.finish(), *id)
            })
            .collect();
        scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        scored.into_iter().map(|(_, id)| id).take(k).collect()
    }

    fn solution_cells(llm: &CrosswordLlmOutput) -> Vec<Vec<char>> {
        llm.grid
            .iter()
            .map(|row| row.chars().collect::<Vec<char>>())
            .collect()
    }

    fn word_cell_coords(w: &crate::crossword::CrosswordWordEntry) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let n = w.answer.chars().count();
        for i in 0..n {
            let (r, c) = match w.direction {
                CrosswordDirection::Across => (w.start_row, w.start_col + i),
                CrosswordDirection::Down => (w.start_row + i, w.start_col),
            };
            out.push((r, c));
        }
        out
    }

    fn read_word_from_cells(
        cells: &[Vec<String>],
        w: &CrosswordExpectedWord,
    ) -> Result<String, DomainError> {
        let mut s = String::new();
        let n = w.answer.chars().count();
        for i in 0..n {
            let (r, c) = match w.direction {
                CrosswordDirection::Across => (w.start_row, w.start_col + i),
                CrosswordDirection::Down => (w.start_row + i, w.start_col),
            };
            let row = cells.get(r).ok_or_else(|| {
                DomainError::InvalidTransition(format!("crossword cells missing row {r}"))
            })?;
            let cell = row.get(c).ok_or_else(|| {
                DomainError::InvalidTransition(format!("crossword cells missing col {c}"))
            })?;
            s.push_str(cell);
        }
        Ok(s)
    }
}

impl Default for CrosswordEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl GameEngine for CrosswordEngine {
    fn kind(&self) -> GameKind {
        GameKind::Crossword
    }

    fn prepare_content(
        &self,
        input: &[LearningItem],
        definition: &GameDefinition,
    ) -> Result<PreparedContent, DomainError> {
        let _ = definition.crossword_config()?;
        let items: Vec<PreparedItem> = input
            .iter()
            .map(|li| PreparedItem {
                learning_item_id: li.id,
                payload: serde_json::to_value(li).unwrap_or(serde_json::json!({})),
            })
            .collect();
        Ok(PreparedContent {
            items,
            provenance: ContentProvenance {
                source: "learning_items".into(),
            },
            passage: None,
            correct_usage_batch: None,
            crossword: None,
            session_seed: None,
            crossword_ui_language: None,
            crossword_difficulty: None,
        })
    }

    fn generate_steps(
        &self,
        content: &PreparedContent,
        definition: &GameDefinition,
    ) -> Result<Vec<GameStep>, DomainError> {
        let cfg = definition.crossword_config()?;
        let llm = content
            .crossword
            .as_ref()
            .ok_or(DomainError::MissingCrossword)?;

        let (rows, cols) = llm.grid_dims()?;
        let sol = Self::solution_cells(llm);

        let difficulty = content
            .crossword_difficulty
            .unwrap_or(cfg.default_difficulty)
            .min(3)
            .max(1);

        let ids: Vec<u32> = llm.words.iter().map(|w| w.id).collect();
        let k = Self::difficulty_prefill_count(ids.len(), difficulty);
        let seed = content.session_seed.unwrap_or(0);
        let prefilled = Self::pick_prefilled_word_ids(&ids, k, seed);

        let mut locked = vec![vec![false; cols]; rows];
        let mut cells: Vec<Vec<String>> = vec![vec![String::new(); cols]; rows];

        for r in 0..rows {
            for c in 0..cols {
                let ch = sol[r][c];
                if ch == CROSSWORD_BLOCK {
                    cells[r][c] = "#".into();
                    locked[r][c] = true;
                }
            }
        }

        for w in &llm.words {
            let pre = prefilled.contains(&w.id);
            for (r, c) in Self::word_cell_coords(w) {
                let ch = sol[r][c];
                if ch == CROSSWORD_BLOCK {
                    continue;
                }
                if pre {
                    cells[r][c] = ch.to_string();
                    locked[r][c] = true;
                }
            }
        }

        let words_ui: Vec<CrosswordWordPublic> = llm
            .words
            .iter()
            .map(|w| CrosswordWordPublic {
                id: w.id,
                hint: w.hint.clone(),
                start_row: w.start_row,
                start_col: w.start_col,
                direction: w.direction,
                is_prefilled_word: prefilled.contains(&w.id),
            })
            .collect();

        let lang = content
            .crossword_ui_language
            .as_deref()
            .unwrap_or("")
            .to_string();
        let text_direction = if Self::is_rtl_language(&lang) {
            "rtl".to_string()
        } else {
            "ltr".to_string()
        };

        let expected_words: Vec<CrosswordExpectedWord> = llm
            .words
            .iter()
            .map(|w| CrosswordExpectedWord {
                id: w.id,
                start_row: w.start_row,
                start_col: w.start_col,
                direction: w.direction,
                answer: w.answer.clone(),
            })
            .collect();

        let step = GameStep {
            id: GameStepId(Uuid::new_v4()),
            ordinal: 0,
            user_facing_step_prompt: UserFacingStepPrompt::CrosswordGrid {
                story: llm.story.clone(),
                rows,
                cols,
                cells,
                locked_cells: locked,
                words: words_ui,
                text_direction,
            },
            expected_answer: ExpectedAnswer::Crossword {
                rows,
                cols,
                words: expected_words,
            },
            user_answer: None,
            evaluation: None,
            deadline_at: None,
            state: StepState::Pending,
        };
        Ok(vec![step])
    }

    fn evaluate_answer(
        &self,
        step: &GameStep,
        answer: &UserAnswer,
        _now: OffsetDateTime,
        definition: &GameDefinition,
    ) -> Result<StepEvaluation, DomainError> {
        let _ = definition.crossword_config()?;
        let ExpectedAnswer::Crossword {
            rows,
            cols,
            words,
        } = &step.expected_answer
        else {
            return Err(DomainError::InvalidTransition(
                "expected crossword".into(),
            ));
        };
        let UserAnswer::CrosswordCells { cells } = answer else {
            return Err(DomainError::InvalidTransition(
                "answer must be crossword_cells".into(),
            ));
        };
        if cells.len() != *rows {
            return Err(DomainError::InvalidTransition(format!(
                "crossword rows mismatch want {} got {}",
                rows,
                cells.len()
            )));
        }
        for (r, row) in cells.iter().enumerate() {
            if row.len() != *cols {
                return Err(DomainError::InvalidTransition(format!(
                    "crossword cols mismatch row {r}"
                )));
            }
        }

        let mut correct_n = 0i32;
        for w in words {
            let got = Self::read_word_from_cells(cells, w)?;
            if Self::normalize(&got) == Self::normalize(&w.answer) {
                correct_n += 1;
            }
        }
        let total = words.len() as i32;
        let all_ok = correct_n == total;
        let per = match &definition.scoring_policy {
            ScoringPolicy::FixedPerCorrect { points } => *points,
        };
        let awarded = correct_n * per;

        Ok(StepEvaluation {
            is_correct: all_ok,
            awarded_points: awarded,
            expected: None,
            actual: None,
            explanation: Some(format!("{correct_n}/{total} words match")),
            evaluation_mode: EvaluationMode::Normalized,
            gap_stats: Some((correct_n, total)),
        })
    }

    fn finalize(
        &self,
        session: &GameSession,
        _definition: &GameDefinition,
    ) -> Result<GameResult, DomainError> {
        let acc = session.score.accuracy();
        Ok(GameResult {
            score: session.score.clone(),
            summary: format!(
                "Finished with {} / {} points ({:.2}% accuracy)",
                session.score.earned_points,
                session.score.total_points,
                acc * 100.0
            ),
        })
    }
}
