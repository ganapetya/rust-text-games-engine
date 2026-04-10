use crate::answer::{EvaluationMode, ExpectedAnswer, StepEvaluation, UserAnswer};
use crate::content::{ContentProvenance, LearningItem, PreparedContent, PreparedItem};
use crate::engine::GameEngine;
use crate::errors::DomainError;
use crate::game_session::GameSession;
use crate::game_step::{GameStep, GapFillSlotPublic, StepState, UserFacingStepPrompt};
use crate::ids::GameStepId;
use crate::passage::PassageGapLlmOutput;
use crate::policies::{GameDefinition, GameKind, GapFillScoringMode, ScoringPolicy};
use crate::result::GameResult;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use time::OffsetDateTime;
use uuid::Uuid;

pub struct GapFillEngine;

impl GapFillEngine {
    pub fn new() -> Self {
        Self
    }

    fn normalize(s: &str) -> String {
        s.trim().to_lowercase()
    }

    /// Deterministic shuffle order derived from session/step entropy.
    fn shuffle_strings(items: Vec<String>, seed: u64) -> Vec<String> {
        let mut idx: Vec<usize> = (0..items.len()).collect();
        idx.sort_by_key(|i| {
            let mut h = DefaultHasher::new();
            h.write_u64(seed);
            h.write_usize(*i);
            h.finish()
        });
        let mut out = Vec::with_capacity(items.len());
        for i in idx {
            if let Some(s) = items.get(i) {
                out.push(s.clone());
            }
        }
        out
    }

    /// Builds the display passage by replacing each hard word span with `___` (replace from high index to low so offsets stay valid).
    fn passage_with_gaps(passage: &PassageGapLlmOutput) -> String {
        let mut sorted: Vec<_> = passage.hard_words.iter().collect();
        sorted.sort_by_key(|h| std::cmp::Reverse(h.start_char));
        let mut s = passage.full_text.clone();
        for h in sorted {
            let c: String = s.chars().skip(h.start_char).take(h.end_char - h.start_char).collect();
            if c == h.surface {
                let before: String = s.chars().take(h.start_char).collect();
                let after: String = s.chars().skip(h.end_char).collect();
                s = format!("{before}___{after}");
            }
        }
        s
    }

    /// Picks `distractors_per_gap` wrong labels from fakes and other gap surfaces; always includes the correct surface.
    fn choices_for_gap(
        correct: &str,
        gap_ordinal: usize,
        all_correct: &[String],
        fake_words: &[String],
        distractors_per_gap: usize,
        seed: u64,
    ) -> Vec<String> {
        let mut pool: Vec<String> = fake_words.to_vec();
        for w in all_correct {
            if !w.eq_ignore_ascii_case(correct) {
                pool.push(w.clone());
            }
        }
        pool.sort();
        pool.dedup();
        pool.retain(|w| !w.eq_ignore_ascii_case(correct));

        let mut idx: Vec<usize> = (0..pool.len()).collect();
        idx.sort_by_key(|i| {
            let mut h = DefaultHasher::new();
            h.write_u64(seed);
            h.write_usize(gap_ordinal);
            h.write_usize(*i);
            h.finish()
        });

        let mut choices: Vec<String> = idx
            .into_iter()
            .take(distractors_per_gap)
            .filter_map(|i| pool.get(i).cloned())
            .collect();
        choices.push(correct.to_string());
        Self::shuffle_strings(choices, seed ^ 0x9e37_79b9_7f4a_7c15)
    }
}

impl Default for GapFillEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl GameEngine for GapFillEngine {
    fn kind(&self) -> GameKind {
        GameKind::GapFill
    }

    fn prepare_content(
        &self,
        input: &[LearningItem],
        definition: &GameDefinition,
    ) -> Result<PreparedContent, DomainError> {
        let _ = definition.gap_fill_config()?;
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
        })
    }

    fn generate_steps(
        &self,
        content: &PreparedContent,
        definition: &GameDefinition,
    ) -> Result<Vec<GameStep>, DomainError> {
        let cfg = definition.gap_fill_config()?;
        let passage = content.passage.as_ref().ok_or(DomainError::MissingPassage)?;

        let word_count = passage.full_text.split_whitespace().count() as u32;
        if word_count > cfg.max_passage_words {
            return Err(DomainError::InvalidPassageContext(format!(
                "passage too long: {word_count} words > {}",
                cfg.max_passage_words
            )));
        }

        let mut ordered: Vec<_> = passage.hard_words.iter().collect();
        ordered.sort_by_key(|h| h.start_char);

        let values: Vec<String> = ordered.iter().map(|h| h.surface.clone()).collect();
        if values.is_empty() {
            return Err(DomainError::NoSteps);
        }

        let display = Self::passage_with_gaps(passage);
        let mut slots: Vec<GapFillSlotPublic> = Vec::new();
        for (ord, hw) in ordered.iter().enumerate() {
            let mut h = DefaultHasher::new();
            (ord as u64).hash(&mut h);
            hw.id.hash(&mut h);
            let seed = h.finish();

            let choices = Self::choices_for_gap(
                &hw.surface,
                ord,
                &values,
                &passage.fake_words,
                cfg.distractors_per_gap,
                seed,
            );
            slots.push(GapFillSlotPublic {
                ordinal: ord,
                choices,
            });
        }

        let step = GameStep {
            id: GameStepId(Uuid::new_v4()),
            ordinal: 0,
            user_facing_step_prompt: UserFacingStepPrompt::GapFillPassage {
                text_with_gaps: display,
                slots,
            },
            expected_answer: ExpectedAnswer::GapFillSlots { values },
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
        let cfg = definition.gap_fill_config()?;
        let expected = match &step.expected_answer {
            ExpectedAnswer::GapFillSlots { values } => values,
            ExpectedAnswer::ExactText { .. } => {
                return Err(DomainError::InvalidTransition(
                    "expected gap_fill_slots".into(),
                ));
            }
        };
        let UserAnswer::GapFillSlots { selections } = answer else {
            return Err(DomainError::InvalidTransition(
                "answer must be gap_fill_slots".into(),
            ));
        };
        if selections.len() != expected.len() {
            return Err(DomainError::InvalidTransition(format!(
                "need {} slot answers got {}",
                expected.len(),
                selections.len()
            )));
        }

        let mut correct_n = 0i32;
        for (e, a) in expected.iter().zip(selections.iter()) {
            if Self::normalize(e) == Self::normalize(a) {
                correct_n += 1;
            }
        }
        let total = expected.len() as i32;
        let all_ok = correct_n == total;

        let per = match &definition.scoring_policy {
            ScoringPolicy::FixedPerCorrect { points } => *points,
        };
        let awarded = match cfg.scoring_mode {
            GapFillScoringMode::PerGap => correct_n * per,
            GapFillScoringMode::AllOrNothing => {
                if all_ok {
                    per
                } else {
                    0
                }
            }
        };

        let actual_summary = selections.join(" | ");
        let expected_summary = expected.join(" | ");

        Ok(StepEvaluation {
            is_correct: all_ok,
            awarded_points: awarded,
            expected: Some(expected_summary),
            actual: Some(actual_summary),
            explanation: None,
            evaluation_mode: EvaluationMode::Exact,
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
