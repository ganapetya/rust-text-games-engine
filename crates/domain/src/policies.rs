use serde::{Deserialize, Serialize};

/// Game family; only `gap_fill` is implemented (passage-based).
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
    pub config: GameConfig,
    pub scoring_policy: ScoringPolicy,
    pub timing_policy: TimingPolicy,
}

impl GameDefinition {
    /// Returns gap-fill config when this definition is a gap_fill game.
    pub fn gap_fill_config(&self) -> Result<&GapFillPassageConfig, crate::errors::DomainError> {
        match &self.config {
            GameConfig::GapFill(c) => Ok(c),
        }
    }
}

/// Kind-specific rules JSON from `game_definitions.config` (tagged for future game types).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GameConfig {
    #[serde(rename = "gap_fill")]
    GapFill(GapFillPassageConfig),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GapFillScoringMode {
    /// Each correct gap awards `scoring_policy` points (typically FixedPerCorrect per slot).
    PerGap,
    /// One award for the step if all gaps are correct.
    AllOrNothing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapFillPassageConfig {
    pub max_passage_words: u32,
    pub distractors_per_gap: usize,
    pub allow_skip: bool,
    pub scoring_mode: GapFillScoringMode,
}

impl Default for GapFillPassageConfig {
    fn default() -> Self {
        Self {
            max_passage_words: 600,
            distractors_per_gap: 2,
            allow_skip: false,
            scoring_mode: GapFillScoringMode::PerGap,
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
