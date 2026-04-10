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

/// One gap in the passage UI: shuffled choices (correct + distractors).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapFillSlotPublic {
    pub ordinal: usize,
    pub choices: Vec<String>,
}

/// Payload shown to the player for one step (API / UI). Distinct from internal engine-only state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UserFacingStepPrompt {
    GapFillPassage {
        /// Passage with `___` markers in place of each hidden word (in ascending gap order).
        text_with_gaps: String,
        slots: Vec<GapFillSlotPublic>,
    },
    /// Pick which of three sentences uses the target word correctly (order is shuffled vs LLM output).
    CorrectUsageChoice {
        word: String,
        /// Three options, labels A/B/C by index 0/1/2 in the UI.
        options: Vec<String>,
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
