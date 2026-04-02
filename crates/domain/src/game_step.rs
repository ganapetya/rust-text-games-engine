use crate::answer::{ExpectedAnswer, StepEvaluation, UserAnswer};
use crate::ids::GameStepId;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepState {
    Pending,
    Active,
    Answered,
    Evaluated,
    TimedOut,
    Skipped,
}

/// Payload shown to the player for one step (API / UI). Distinct from internal engine-only state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UserFacingStepPrompt {
    GapFill {
        text_with_gap: String,
        choices: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStep {
    pub id: GameStepId,
    pub ordinal: usize,
    #[serde(rename = "userFacingStepPrompt")]
    pub user_facing_step_prompt: UserFacingStepPrompt,
    pub expected_answer: ExpectedAnswer,
    pub user_answer: Option<UserAnswer>,
    pub evaluation: Option<StepEvaluation>,
    pub deadline_at: Option<OffsetDateTime>,
    pub state: StepState,
}
