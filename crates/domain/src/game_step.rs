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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepPrompt {
    GapFill {
        text_with_gap: String,
        choices: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStep {
    pub id: GameStepId,
    pub ordinal: usize,
    pub prompt: StepPrompt,
    pub expected_answer: ExpectedAnswer,
    pub user_answer: Option<UserAnswer>,
    pub evaluation: Option<StepEvaluation>,
    pub deadline_at: Option<OffsetDateTime>,
    pub state: StepState,
}
