use serde::{Deserialize, Serialize};

/// Game family; v0.1 only supports gap_fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameKind {
    GapFill,
}

/// Static definition for a game type (loaded from DB / seeded).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameDefinition {
    pub id: crate::ids::GameDefinitionId,
    pub kind: GameKind,
    pub version: i32,
    pub name: String,
    pub config: GapFillConfig,
    pub scoring_policy: ScoringPolicy,
    pub timing_policy: TimingPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapFillConfig {
    pub steps_count: usize,
    pub distractors_per_step: usize,
    pub allow_skip: bool,
}

impl Default for GapFillConfig {
    fn default() -> Self {
        Self {
            steps_count: 10,
            distractors_per_step: 2,
            allow_skip: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScoringPolicy {
    FixedPerCorrect { points: i32 },
}

impl Default for ScoringPolicy {
    fn default() -> Self {
        ScoringPolicy::FixedPerCorrect { points: 10 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingPolicy {
    pub per_step_limit_secs: Option<u32>,
    pub session_limit_secs: Option<u32>,
    pub auto_advance_on_timeout: bool,
}

impl Default for TimingPolicy {
    fn default() -> Self {
        Self {
            per_step_limit_secs: None,
            session_limit_secs: None,
            auto_advance_on_timeout: false,
        }
    }
}
