use crate::answer::{StepEvaluation, UserAnswer};
use crate::errors::DomainError;
use crate::game_step::{GameStep, StepState};
use crate::ids::{GameDefinitionId, GameSessionId, UserId};
use crate::policies::{GameDefinition, ScoringPolicy};
use crate::score::Score;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameSessionState {
    Draft,
    Prepared,
    InProgress,
    Completed,
    TimedOut,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSession {
    pub id: GameSessionId,
    pub user_id: UserId,
    pub definition_id: GameDefinitionId,
    pub state: GameSessionState,
    pub steps: Vec<GameStep>,
    pub current_step_index: usize,
    pub score: Score,
    pub started_at: Option<OffsetDateTime>,
    pub completed_at: Option<OffsetDateTime>,
    pub expires_at: Option<OffsetDateTime>,
    #[serde(skip)]
    pub definition: Option<GameDefinition>,
}

impl GameSession {
    pub fn new(
        id: GameSessionId,
        user_id: UserId,
        definition_id: GameDefinitionId,
        steps: Vec<GameStep>,
        definition: GameDefinition,
    ) -> Self {
        let total = steps.len() as i32
            * match &definition.scoring_policy {
                ScoringPolicy::FixedPerCorrect { points } => *points,
            };
        Self {
            id,
            user_id,
            definition_id,
            state: GameSessionState::Prepared,
            steps,
            current_step_index: 0,
            score: Score {
                total_points: total.max(0),
                earned_points: 0,
                correct_count: 0,
                answered_count: 0,
            },
            started_at: None,
            completed_at: None,
            expires_at: None,
            definition: Some(definition),
        }
    }

    pub fn definition(&self) -> Result<&GameDefinition, DomainError> {
        self.definition
            .as_ref()
            .ok_or_else(|| DomainError::InvalidTransition("missing definition".into()))
    }

    fn per_step_deadline(
        &self,
        from: OffsetDateTime,
    ) -> Result<Option<OffsetDateTime>, DomainError> {
        let pol = &self.definition()?.timing_policy;
        Ok(pol
            .per_step_limit_secs
            .map(|s| from + time::Duration::seconds(s as i64)))
    }

    pub fn start(&mut self, now: OffsetDateTime) -> Result<(), DomainError> {
        if self.state != GameSessionState::Prepared {
            return Err(DomainError::InvalidSessionState {
                from: self.state,
                expected: GameSessionState::Prepared,
            });
        }
        self.state = GameSessionState::InProgress;
        self.started_at = Some(now);
        if let Some(pol) = &self.definition {
            if let Some(limit) = pol.timing_policy.session_limit_secs {
                self.expires_at = Some(now + time::Duration::seconds(limit as i64));
            }
        }
        let deadline = self.per_step_deadline(now)?;
        if let Some(step) = self.steps.get_mut(self.current_step_index) {
            step.state = StepState::Active;
            step.deadline_at = deadline;
        }
        Ok(())
    }

    pub fn check_session_expired(&mut self, now: OffsetDateTime) -> Result<(), DomainError> {
        if self.state != GameSessionState::InProgress {
            return Ok(());
        }
        if let Some(exp) = self.expires_at {
            if now >= exp {
                self.state = GameSessionState::TimedOut;
                self.completed_at = Some(now);
                if let Some(step) = self.steps.get_mut(self.current_step_index) {
                    if matches!(step.state, StepState::Active | StepState::Pending) {
                        step.state = StepState::TimedOut;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn is_step_timed_out(&self, step_index: usize, now: OffsetDateTime) -> bool {
        let Some(step) = self.steps.get(step_index) else {
            return false;
        };
        matches!(step.state, StepState::Active)
            && step.deadline_at.map(|d| now > d).unwrap_or(false)
    }

    pub fn timeout_current_step(&mut self, now: OffsetDateTime) -> Result<(), DomainError> {
        if self.state != GameSessionState::InProgress {
            return Err(DomainError::InvalidSessionState {
                from: self.state,
                expected: GameSessionState::InProgress,
            });
        }
        let idx = self.current_step_index;
        let Some(step) = self.steps.get_mut(idx) else {
            return Err(DomainError::InvalidTransition("no current step".into()));
        };
        if step.state != StepState::Active {
            return Err(DomainError::StepNotActive);
        }
        step.state = StepState::TimedOut;
        self.score.answered_count += 1;
        if self.current_step_index + 1 >= self.steps.len() {
            self.state = GameSessionState::Completed;
            self.completed_at = Some(now);
        } else {
            self.current_step_index += 1;
            let deadline = self.per_step_deadline(now)?;
            if let Some(next) = self.steps.get_mut(self.current_step_index) {
                next.state = StepState::Active;
                next.deadline_at = deadline;
            }
        }
        Ok(())
    }

    pub fn record_evaluation(
        &mut self,
        step_index: usize,
        mut evaluation: StepEvaluation,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        if matches!(
            self.state,
            GameSessionState::Completed | GameSessionState::TimedOut | GameSessionState::Cancelled
        ) {
            return Err(DomainError::SessionCompleted);
        }
        if self.state != GameSessionState::InProgress {
            return Err(DomainError::InvalidSessionState {
                from: self.state,
                expected: GameSessionState::InProgress,
            });
        }
        if step_index != self.current_step_index {
            return Err(DomainError::WrongStep);
        }
        if self.is_step_timed_out(step_index, now) {
            return Err(DomainError::StepTimedOut);
        }

        let points = match &self.definition()?.scoring_policy {
            ScoringPolicy::FixedPerCorrect { points } => {
                if evaluation.is_correct {
                    *points
                } else {
                    0
                }
            }
        };

        let Some(step) = self.steps.get_mut(step_index) else {
            return Err(DomainError::WrongStep);
        };
        if step.state != StepState::Active {
            return Err(DomainError::StepNotActive);
        }

        evaluation.awarded_points = points;

        step.user_answer = evaluation
            .actual
            .clone()
            .map(|value| UserAnswer::Text { value });
        step.evaluation = Some(evaluation.clone());
        step.state = StepState::Evaluated;

        self.score.earned_points += points;
        self.score.answered_count += 1;
        if evaluation.is_correct {
            self.score.correct_count += 1;
        }

        if step_index + 1 >= self.steps.len() {
            self.state = GameSessionState::Completed;
            self.completed_at = Some(now);
        }

        Ok(())
    }

    pub fn advance(&mut self, now: OffsetDateTime) -> Result<(), DomainError> {
        if self.state != GameSessionState::InProgress {
            return Err(DomainError::InvalidSessionState {
                from: self.state,
                expected: GameSessionState::InProgress,
            });
        }
        let idx = self.current_step_index;
        let Some(step) = self.steps.get(idx) else {
            return Err(DomainError::InvalidTransition("no step".into()));
        };
        if step.state != StepState::Evaluated && step.state != StepState::Skipped {
            return Err(DomainError::InvalidTransition(
                "current step not finished".into(),
            ));
        }
        if idx + 1 >= self.steps.len() {
            self.state = GameSessionState::Completed;
            self.completed_at = Some(now);
            return Ok(());
        }
        self.current_step_index = idx + 1;
        let deadline = self.per_step_deadline(now)?;
        if let Some(next) = self.steps.get_mut(self.current_step_index) {
            next.state = StepState::Active;
            next.deadline_at = deadline;
        }
        Ok(())
    }

    pub fn current_step(&self) -> Option<&GameStep> {
        self.steps.get(self.current_step_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::GameEngine;
    use crate::gap_fill::GapFillEngine;
    use crate::policies::{GapFillConfig, TimingPolicy};
    use serde_json::json;
    use uuid::Uuid;

    fn def() -> GameDefinition {
        GameDefinition {
            id: crate::ids::GameDefinitionId(Uuid::nil()),
            kind: crate::policies::GameKind::GapFill,
            version: 1,
            name: "test".into(),
            config: GapFillConfig {
                steps_count: 2,
                distractors_per_step: 1,
                allow_skip: false,
            },
            scoring_policy: ScoringPolicy::FixedPerCorrect { points: 10 },
            timing_policy: TimingPolicy {
                per_step_limit_secs: Some(60),
                session_limit_secs: None,
                auto_advance_on_timeout: false,
            },
        }
    }

    fn sample_items() -> Vec<crate::content::LearningItem> {
        vec![
            crate::content::LearningItem {
                id: crate::ids::LearningItemId(Uuid::new_v4()),
                user_id: UserId(Uuid::new_v4()),
                source_text: "Han gikk til butikken.".into(),
                context_text: Some("Han gikk til butikken i går.".into()),
                hard_fragment: "gikk".into(),
                lemma: None,
                language: "no".into(),
                metadata: json!({}),
            },
            crate::content::LearningItem {
                id: crate::ids::LearningItemId(Uuid::new_v4()),
                user_id: UserId(Uuid::new_v4()),
                source_text: "Jeg ser katt".into(),
                context_text: None,
                hard_fragment: "ser".into(),
                lemma: None,
                language: "no".into(),
                metadata: json!({}),
            },
        ]
    }

    fn make_session() -> GameSession {
        let engine = GapFillEngine::new();
        let items = sample_items();
        let d = def();
        let prep = engine.prepare_content(&items, &d.config).unwrap();
        let steps = engine.generate_steps(&prep, &d.config).unwrap();
        GameSession::new(
            GameSessionId(Uuid::new_v4()),
            items[0].user_id,
            d.id,
            steps,
            d,
        )
    }

    #[test]
    fn start_activates_first_step() {
        let mut s = make_session();
        let now = OffsetDateTime::now_utc();
        s.start(now).unwrap();
        assert_eq!(s.state, GameSessionState::InProgress);
        assert_eq!(s.steps[0].state, StepState::Active);
        assert!(s.steps[0].deadline_at.is_some());
    }

    #[test]
    fn cannot_start_twice() {
        let mut s = make_session();
        let now = OffsetDateTime::now_utc();
        s.start(now).unwrap();
        assert!(s.start(now).is_err());
    }

    #[test]
    fn submit_evaluation_completes_last_step() {
        let mut s = make_session();
        let now = OffsetDateTime::now_utc();
        s.start(now).unwrap();
        // two steps — evaluate first, advance, evaluate second
        let ev = StepEvaluation {
            is_correct: true,
            awarded_points: 0,
            expected: Some("gikk".into()),
            actual: Some("gikk".into()),
            explanation: None,
            evaluation_mode: crate::answer::EvaluationMode::Exact,
        };
        s.record_evaluation(0, ev, now).unwrap();
        assert_eq!(s.state, GameSessionState::InProgress);
        s.advance(now).unwrap();
        let ev2 = StepEvaluation {
            is_correct: true,
            awarded_points: 0,
            expected: Some("ser".into()),
            actual: Some("ser".into()),
            explanation: None,
            evaluation_mode: crate::answer::EvaluationMode::Exact,
        };
        s.record_evaluation(1, ev2, now).unwrap();
        assert_eq!(s.state, GameSessionState::Completed);
        assert_eq!(s.score.earned_points, 20);
    }

    #[test]
    fn double_evaluate_fails() {
        let mut s = make_session();
        let now = OffsetDateTime::now_utc();
        s.start(now).unwrap();
        let ev = StepEvaluation {
            is_correct: true,
            awarded_points: 0,
            expected: Some("gikk".into()),
            actual: Some("gikk".into()),
            explanation: None,
            evaluation_mode: crate::answer::EvaluationMode::Exact,
        };
        s.record_evaluation(0, ev.clone(), now).unwrap();
        assert!(s.record_evaluation(0, ev, now).is_err());
    }

    #[test]
    fn timeout_blocks_answer() {
        let mut s = make_session();
        let t0 = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        s.start(t0).unwrap();
        let past = t0 + time::Duration::hours(1);
        assert!(s.is_step_timed_out(0, past));
        let ev = StepEvaluation {
            is_correct: true,
            awarded_points: 0,
            expected: Some("gikk".into()),
            actual: Some("gikk".into()),
            explanation: None,
            evaluation_mode: crate::answer::EvaluationMode::Exact,
        };
        assert!(s.record_evaluation(0, ev, past).is_err());
    }
}
