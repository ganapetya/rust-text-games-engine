use crate::crossword::CrosswordDirection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UserAnswer {
    Text { value: String },
    GapFillSlots { selections: Vec<String> },
    /// Player grid: same dimensions as the prompt; `""` = empty letter cell; `"#"` = block.
    CrosswordCells {
        cells: Vec<Vec<String>>,
    },
}

/// One word’s canonical answer for server-side scoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrosswordExpectedWord {
    pub id: u32,
    pub start_row: usize,
    pub start_col: usize,
    pub direction: CrosswordDirection,
    pub answer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExpectedAnswer {
    ExactText { value: String },
    GapFillSlots { values: Vec<String> },
    Crossword {
        rows: usize,
        cols: usize,
        words: Vec<CrosswordExpectedWord>,
    },
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
