use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UserAnswer {
    Text { value: String },
    GapFillSlots { selections: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExpectedAnswer {
    ExactText { value: String },
    GapFillSlots { values: Vec<String> },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationMode {
    Exact,
    Normalized,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepEvaluation {
    pub is_correct: bool,
    pub awarded_points: i32,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub explanation: Option<String>,
    pub evaluation_mode: EvaluationMode,
    /// When set, `(correct_gaps, total_gaps)` for passage scoring aggregates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap_stats: Option<(i32, i32)>,
}
