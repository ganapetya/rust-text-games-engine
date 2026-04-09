use crate::answer::{ExpectedAnswer, StepEvaluation, UserAnswer};
use crate::errors::DomainError;
use crate::game_step::{GameStep, StepState};
use crate::ids::{GameDefinitionId, GameSessionId, UserId};
use crate::policies::{GameDefinition, GapFillScoringMode, ScoringPolicy};
use crate::score::Score;
use serde::{Deserialize, Serialize};
use serde_json::json;
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
    /// Validated LLM JSON (`PassageGapLlmOutput`) once materialized.
    #[serde(default)]
    pub base_context: serde_json::Value,
    /// Payload from create until start consumes it (`deferred_payload`).
    #[serde(default)]
    pub deferred_payload: Option<serde_json::Value>,
    #[serde(skip)]
    pub definition: Option<GameDefinition>,
}

impl GameSession {
    fn passage_total_points(definition: &GameDefinition, num_gaps: usize) -> Result<i32, DomainError> {
        let gap = definition.gap_fill_config()?;
        let per = match &definition.scoring_policy {
            ScoringPolicy::FixedPerCorrect { points } => *points,
        };
        Ok(match gap.scoring_mode {
            GapFillScoringMode::PerGap => per * num_gaps as i32,
            GapFillScoringMode::AllOrNothing => per,
        })
    }

    fn num_gaps_from_steps(steps: &[GameStep]) -> usize {
        steps
            .first()
            .map(|s| match &s.expected_answer {
                ExpectedAnswer::GapFillSlots { values } => values.len(),
                ExpectedAnswer::ExactText { .. } => 1,
            })
            .unwrap_or(0)
    }

    /// Refreshes `score.total_points` from the current steps and definition (e.g. after materializing a draft).
    pub fn recompute_total_points(&mut self) -> Result<(), DomainError> {
        let def = self.definition()?;
        let n = Self::num_gaps_from_steps(&self.steps);
        self.score.total_points = Self::passage_total_points(def, n)?.max(0);
        Ok(())
    }

    /// Session waiting for `start_game_session` (no steps yet).
    pub fn new_draft(
        id: GameSessionId,
        user_id: UserId,
        definition_id: GameDefinitionId,
        definition: GameDefinition,
        deferred_payload: serde_json::Value,
    ) -> Self {
        Self {
            id,
            user_id,
            definition_id,
            state: GameSessionState::Draft,
            steps: vec![],
            current_step_index: 0,
            score: Score {
                total_points: 0,
                earned_points: 0,
                correct_count: 0,
                answered_count: 0,
            },
            started_at: None,
            completed_at: None,
            expires_at: None,
            base_context: json!({}),
            deferred_payload: Some(deferred_payload),
            definition: Some(definition),
        }
    }

    /// Playable session after steps are built (typically still `Prepared` until `start`).
    pub fn new(
        id: GameSessionId,
        user_id: UserId,
        definition_id: GameDefinitionId,
        steps: Vec<GameStep>,
        definition: GameDefinition,
        base_context: serde_json::Value,
    ) -> Result<Self, DomainError> {
        let ng = Self::num_gaps_from_steps(&steps);
        let total = Self::passage_total_points(&definition, ng)?;
        Ok(Self {
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
            base_context,
            deferred_payload: None,
            definition: Some(definition),
        })
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

    /// Moves `Prepared` or materialized `Draft` (non-empty steps) into `InProgress` and activates the current step.
    pub fn start(&mut self, now: OffsetDateTime) -> Result<(), DomainError> {
        let ok_prepared = self.state == GameSessionState::Prepared;
        let ok_draft = self.state == GameSessionState::Draft && !self.steps.is_empty();
        if !(ok_prepared || ok_draft) {
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
        let add = match self.steps.get(idx).map(|s| &s.expected_answer) {
            Some(ExpectedAnswer::GapFillSlots { values }) => values.len() as i32,
            _ => 1,
        };
        self.score.answered_count += add;
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
        submitted_answer: UserAnswer,
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

        let points = evaluation.awarded_points;

        let Some(step) = self.steps.get_mut(step_index) else {
            return Err(DomainError::WrongStep);
        };
        if step.state != StepState::Active {
            return Err(DomainError::StepNotActive);
        }

        evaluation.awarded_points = points;

        step.user_answer = Some(submitted_answer);
        step.evaluation = Some(evaluation.clone());
        step.state = StepState::Evaluated;

        self.score.earned_points += points;
        if let Some((c, t)) = evaluation.gap_stats {
            self.score.correct_count += c;
            self.score.answered_count += t;
        } else {
            self.score.answered_count += 1;
            if evaluation.is_correct {
                self.score.correct_count += 1;
            }
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
    use crate::passage::{PassageGapLlmOutput, PassageHardWordOccurrence, PASSAGE_LLM_SCHEMA_VERSION};
    use crate::policies::{GameConfig, GapFillPassageConfig, GapFillScoringMode, TimingPolicy};
    use serde_json::json;
    use uuid::Uuid;

    fn def() -> GameDefinition {
        GameDefinition {
            id: crate::ids::GameDefinitionId(Uuid::nil()),
            kind: crate::policies::GameKind::GapFill,
            version: 1,
            name: "test".into(),
            config: GameConfig::GapFill(GapFillPassageConfig {
                max_passage_words: 600,
                distractors_per_gap: 1,
                allow_skip: false,
                scoring_mode: GapFillScoringMode::PerGap,
                ..Default::default()
            }),
            scoring_policy: ScoringPolicy::FixedPerCorrect { points: 10 },
            timing_policy: TimingPolicy {
                per_step_limit_secs: Some(60),
                session_limit_secs: None,
                auto_advance_on_timeout: false,
            },
        }
    }

    fn sample_passage() -> PassageGapLlmOutput {
        PassageGapLlmOutput {
            schema_version: PASSAGE_LLM_SCHEMA_VERSION,
            full_text: "Han gikk hjem og leser.".into(),
            hard_words: vec![
                PassageHardWordOccurrence {
                    id: 0,
                    start_char: 4,
                    end_char: 8,
                    surface: "gikk".into(),
                },
                PassageHardWordOccurrence {
                    id: 1,
                    start_char: 17,
                    end_char: 22,
                    surface: "leser".into(),
                },
            ],
            fake_words: vec!["sprang".into(), "sov".into()],
        }
    }

    fn sample_items() -> Vec<crate::content::LearningItem> {
        vec![crate::content::LearningItem {
            id: crate::ids::LearningItemId(Uuid::new_v4()),
            user_id: UserId(Uuid::new_v4()),
            source_text: "Han gikk til butikken.".into(),
            context_text: Some("Han gikk til butikken i går.".into()),
            hard_fragment: "gikk".into(),
            lemma: None,
            language: "no".into(),
            metadata: json!({}),
        }]
    }

    fn make_session() -> GameSession {
        let engine = GapFillEngine::new();
        let items = sample_items();
        let d = def();
        let mut prep = engine.prepare_content(&items, &d).unwrap();
        prep.passage = Some(sample_passage());
        let steps = engine.generate_steps(&prep, &d).unwrap();
        let ctx = serde_json::to_value(prep.passage.as_ref().unwrap()).unwrap();
        GameSession::new(
            GameSessionId(Uuid::new_v4()),
            items[0].user_id,
            d.id,
            steps,
            d,
            ctx,
        )
        .unwrap()
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
    fn submit_evaluation_completes_passage_step() {
        let engine = GapFillEngine::new();
        let mut s = make_session();
        let now = OffsetDateTime::now_utc();
        s.start(now).unwrap();
        let def = s.definition().unwrap().clone();
        let step = s.current_step().unwrap().clone();
        let ev = engine
            .evaluate_answer(
                &step,
                &UserAnswer::GapFillSlots {
                    selections: vec!["gikk".into(), "leser".into()],
                },
                now,
                &def,
            )
            .unwrap();
        s.record_evaluation(
            0,
            ev,
            UserAnswer::GapFillSlots {
                selections: vec!["gikk".into(), "leser".into()],
            },
            now,
        )
        .unwrap();
        assert_eq!(s.state, GameSessionState::Completed);
        assert_eq!(s.score.earned_points, 20);
        assert_eq!(s.score.correct_count, 2);
    }

    #[test]
    fn double_evaluate_fails() {
        let engine = GapFillEngine::new();
        let mut s = make_session();
        let now = OffsetDateTime::now_utc();
        s.start(now).unwrap();
        let def = s.definition().unwrap().clone();
        let step = s.current_step().unwrap().clone();
        let ev = engine
            .evaluate_answer(
                &step,
                &UserAnswer::GapFillSlots {
                    selections: vec!["gikk".into(), "leser".into()],
                },
                now,
                &def,
            )
            .unwrap();
        let ans = UserAnswer::GapFillSlots {
            selections: vec!["gikk".into(), "leser".into()],
        };
        s.record_evaluation(0, ev.clone(), ans.clone(), now).unwrap();
        assert!(s.record_evaluation(0, ev, ans, now).is_err());
    }

    #[test]
    fn timeout_blocks_answer() {
        let engine = GapFillEngine::new();
        let mut s = make_session();
        let t0 = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        s.start(t0).unwrap();
        let past = t0 + time::Duration::hours(1);
        assert!(s.is_step_timed_out(0, past));
        let def = s.definition().unwrap().clone();
        let step = s.current_step().unwrap().clone();
        let ev = engine
            .evaluate_answer(
                &step,
                &UserAnswer::GapFillSlots {
                    selections: vec!["gikk".into(), "leser".into()],
                },
                past,
                &def,
            )
            .unwrap();
        assert!(s
            .record_evaluation(
                0,
                ev,
                UserAnswer::GapFillSlots {
                    selections: vec!["gikk".into(), "leser".into()],
                },
                past,
            )
            .is_err());
    }
}
