use crate::answer::{EvaluationMode, ExpectedAnswer, StepEvaluation, UserAnswer};
use crate::content::{ContentProvenance, LearningItem, PreparedContent, PreparedItem};
use crate::engine::GameEngine;
use crate::errors::DomainError;
use crate::game_session::GameSession;
use crate::game_step::{GameStep, StepPrompt, StepState};
use crate::ids::GameStepId;
use crate::policies::{GameDefinition, GameKind, GapFillConfig};
use crate::result::GameResult;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use time::OffsetDateTime;
use uuid::Uuid;

pub struct GapFillEngine;

impl GapFillEngine {
    pub fn new() -> Self {
        Self
    }

    fn text_with_gap(item: &LearningItem) -> String {
        let base = item
            .context_text
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(&item.source_text);
        if let Some(pos) = base.find(&item.hard_fragment) {
            let mut s = String::with_capacity(base.len() + 4);
            s.push_str(&base[..pos]);
            s.push_str(" ___ ");
            s.push_str(&base[pos + item.hard_fragment.len()..]);
            s
        } else {
            format!("{base} (___)")
        }
    }

    fn normalize(s: &str) -> String {
        s.trim().to_lowercase()
    }

    fn pick_distractors(
        correct: &str,
        others: &[&LearningItem],
        n: usize,
        seed: u64,
    ) -> Vec<String> {
        let mut cands: Vec<String> = others
            .iter()
            .map(|o| o.hard_fragment.clone())
            .filter(|f| !f.eq_ignore_ascii_case(correct))
            .collect();
        cands.sort();
        cands.dedup();
        // deterministic shuffle from seed
        let mut idx: Vec<usize> = (0..cands.len()).collect();
        idx.sort_by_key(|i| {
            let mut h = DefaultHasher::new();
            h.write_u64(seed);
            h.write_usize(*i);
            h.finish()
        });
        let mut out: Vec<String> = idx
            .into_iter()
            .take(n)
            .filter_map(|i| cands.get(i).cloned())
            .collect();
        out.push(correct.to_string());
        out.sort_by_key(|s| {
            let mut h = DefaultHasher::new();
            h.write_u64(seed);
            h.write(s.as_bytes());
            h.finish()
        });
        out
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
        config: &GapFillConfig,
    ) -> Result<PreparedContent, DomainError> {
        let take = config.steps_count.min(input.len());
        if take == 0 {
            return Err(DomainError::NoSteps);
        }
        let items: Vec<PreparedItem> = input
            .iter()
            .take(take)
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
        })
    }

    fn generate_steps(
        &self,
        content: &PreparedContent,
        config: &GapFillConfig,
    ) -> Result<Vec<GameStep>, DomainError> {
        let mut items: Vec<LearningItem> = Vec::new();
        for pi in &content.items {
            if let Ok(li) = serde_json::from_value::<LearningItem>(pi.payload.clone()) {
                items.push(li);
            }
        }
        if items.is_empty() {
            return Err(DomainError::NoSteps);
        }
        let mut steps = Vec::new();
        for (ordinal, item) in items.iter().enumerate() {
            let text = Self::text_with_gap(item);
            let correct = item.hard_fragment.clone();
            let others: Vec<&LearningItem> = items
                .iter()
                .enumerate()
                .filter_map(|(i, x)| if i != ordinal { Some(x) } else { None })
                .collect();
            let seed = ordinal as u64 ^ item.id.0.as_u128() as u64;
            let distractors =
                Self::pick_distractors(&correct, &others, config.distractors_per_step, seed);
            let prompt = StepPrompt::GapFill {
                text_with_gap: text,
                choices: distractors,
            };
            steps.push(GameStep {
                id: GameStepId(Uuid::new_v4()),
                ordinal,
                prompt,
                expected_answer: ExpectedAnswer::ExactText { value: correct },
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
        _config: &GapFillConfig,
    ) -> Result<StepEvaluation, DomainError> {
        let ExpectedAnswer::ExactText { value: expected } = &step.expected_answer;
        let UserAnswer::Text { value } = answer;
        let ok = Self::normalize(value) == Self::normalize(expected);
        Ok(StepEvaluation {
            is_correct: ok,
            awarded_points: 0, // filled by engine layer from policy
            expected: Some(expected.clone()),
            actual: Some(value.clone()),
            explanation: None,
            evaluation_mode: EvaluationMode::Exact,
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
