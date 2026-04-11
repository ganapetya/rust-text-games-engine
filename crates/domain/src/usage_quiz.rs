//! Engine for “choose correct usage” (one step per hard word).

use crate::answer::{EvaluationMode, ExpectedAnswer, StepEvaluation, UserAnswer};
use crate::content::{ContentProvenance, LearningItem, PreparedContent, PreparedItem};
use crate::engine::GameEngine;
use crate::errors::DomainError;
use crate::game_session::GameSession;
use crate::game_step::{GameStep, StepState, UserFacingStepPrompt};
use crate::ids::GameStepId;
use crate::policies::{GameDefinition, GameKind, ScoringPolicy};
use crate::result::GameResult;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use time::OffsetDateTime;
use uuid::Uuid;

pub struct CorrectUsageEngine;

impl CorrectUsageEngine {
    pub fn new() -> Self {
        Self
    }

    fn normalize_answer(s: &str) -> String {
        s.trim().to_lowercase()
    }

    /// Deterministic shuffle of three strings; returns (shuffled, index of original `correct` in shuffled).
    fn shuffle_three(sentences: [String; 3], correct_index: u8, seed: u64) -> (Vec<String>, usize) {
        let correct_idx = correct_index as usize;
        let mut order: Vec<usize> = vec![0, 1, 2];
        order.sort_by_key(|i| {
            let mut h = DefaultHasher::new();
            h.write_u64(seed);
            h.write_usize(*i);
            h.finish()
        });
        let shuffled: Vec<String> = order.iter().map(|i| sentences[*i].clone()).collect();
        let new_correct_pos = order
            .iter()
            .position(|&i| i == correct_idx)
            .unwrap_or(0);
        (shuffled, new_correct_pos)
    }
}

impl Default for CorrectUsageEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl GameEngine for CorrectUsageEngine {
    fn kind(&self) -> GameKind {
        GameKind::CorrectUsage
    }

    fn prepare_content(
        &self,
        input: &[LearningItem],
        definition: &GameDefinition,
    ) -> Result<PreparedContent, DomainError> {
        let _ = definition.correct_usage_config()?;
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
        let _ = definition.correct_usage_config()?;
        let batch = content
            .correct_usage_batch
            .as_ref()
            .ok_or(DomainError::MissingCorrectUsageBatch)?;

        if batch.puzzles.is_empty() {
            return Err(DomainError::NoSteps);
        }

        let mut steps = Vec::with_capacity(batch.puzzles.len());
        for (ord, puzzle) in batch.puzzles.iter().enumerate() {
            let s0 = puzzle.sentences[0].trim().to_string();
            let s1 = puzzle.sentences[1].trim().to_string();
            let s2 = puzzle.sentences[2].trim().to_string();
            let arr = [s0, s1, s2];
            let mut h = DefaultHasher::new();
            (ord as u64).hash(&mut h);
            puzzle.word.hash(&mut h);
            let seed = h.finish();
            let (options, correct_pos) =
                Self::shuffle_three(arr, puzzle.correct_index, seed);
            let correct_sentence = options[correct_pos].clone();

            steps.push(GameStep {
                id: GameStepId(Uuid::new_v4()),
                ordinal: ord,
                user_facing_step_prompt: UserFacingStepPrompt::CorrectUsageChoice {
                    word: puzzle.word.clone(),
                    options,
                },
                expected_answer: ExpectedAnswer::ExactText {
                    value: correct_sentence,
                },
                user_answer: None,
                evaluation: None,
                deadline_at: None,
                state: StepState::Pending,
            });
        }

        Ok(steps)
    }

    fn evaluate_answer(
        &self,
        step: &GameStep,
        answer: &UserAnswer,
        _now: OffsetDateTime,
        definition: &GameDefinition,
    ) -> Result<StepEvaluation, DomainError> {
        let _ = definition.correct_usage_config()?;
        let ExpectedAnswer::ExactText { value: expected } = &step.expected_answer else {
            return Err(DomainError::InvalidTransition(
                "expected exact_text".into(),
            ));
        };
        let UserAnswer::Text { value } = answer else {
            return Err(DomainError::InvalidTransition(
                "answer must be text (chosen sentence)".into(),
            ));
        };
        let ok = Self::normalize_answer(value) == Self::normalize_answer(expected);
        let per = match &definition.scoring_policy {
            ScoringPolicy::FixedPerCorrect { points } => *points,
        };
        let awarded = if ok { per } else { 0 };
        Ok(StepEvaluation {
            is_correct: ok,
            awarded_points: awarded,
            expected: Some(expected.clone()),
            actual: Some(value.clone()),
            explanation: None,
            evaluation_mode: EvaluationMode::Normalized,
            gap_stats: None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shuffle_is_deterministic() {
        let s = ["a".into(), "b".into(), "c".into()];
        let x = CorrectUsageEngine::shuffle_three(s.clone(), 1, 42);
        let y = CorrectUsageEngine::shuffle_three(s, 1, 42);
        assert_eq!(x.0, y.0);
        assert_eq!(x.1, y.1);
    }
}
