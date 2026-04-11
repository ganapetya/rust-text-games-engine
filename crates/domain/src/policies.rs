use serde::{Deserialize, Serialize};

/// Game family: passage gap-fill, crossword, and per-word usage choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameKind {
    GapFill,
    CorrectUsage,
    Crossword,
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
            GameConfig::CorrectUsage(_) | GameConfig::Crossword(_) => {
                Err(crate::errors::DomainError::InvalidTransition(
                    "not a gap_fill definition".into(),
                ))
            }
        }
    }

    pub fn correct_usage_config(&self) -> Result<&CorrectUsageConfig, crate::errors::DomainError> {
        match &self.config {
            GameConfig::CorrectUsage(c) => Ok(c),
            GameConfig::GapFill(_) | GameConfig::Crossword(_) => {
                Err(crate::errors::DomainError::InvalidTransition(
                    "not a correct_usage definition".into(),
                ))
            }
        }
    }

    pub fn crossword_config(&self) -> Result<&CrosswordConfig, crate::errors::DomainError> {
        match &self.config {
            GameConfig::Crossword(c) => Ok(c),
            GameConfig::GapFill(_) | GameConfig::CorrectUsage(_) => {
                Err(crate::errors::DomainError::InvalidTransition(
                    "not a crossword definition".into(),
                ))
            }
        }
    }
}

/// Kind-specific rules JSON from `game_definitions.config` (tagged for future game types).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GameConfig {
    #[serde(rename = "gap_fill")]
    GapFill(GapFillPassageConfig),
    #[serde(rename = "correct_usage")]
    CorrectUsage(CorrectUsageConfig),
    #[serde(rename = "crossword")]
    Crossword(CrosswordConfig),
}

fn default_max_grid_dim() -> u32 {
    24
}

fn default_max_words() -> u32 {
    40
}

fn default_max_hint_chars() -> u32 {
    400
}

fn default_false() -> bool {
    false
}

fn default_game_time_seconds() -> i32 {
    0
}

fn default_crossword_difficulty() -> u8 {
    3
}

/// Rules for LLM authoring + UI for crossword (single step).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrosswordConfig {
    #[serde(default = "default_max_learning_items_for_llm")]
    pub max_learning_items_for_llm: u32,
    #[serde(default = "default_max_grid_dim")]
    pub max_grid_rows: u32,
    #[serde(default = "default_max_grid_dim")]
    pub max_grid_cols: u32,
    #[serde(default = "default_max_words")]
    pub max_words: u32,
    #[serde(default = "default_max_hint_chars")]
    pub max_hint_chars: u32,
    /// When true and `game_time_seconds` > 0, session gets a wall-clock limit at start.
    #[serde(default = "default_false")]
    pub is_time_game: bool,
    /// Seconds for whole session when timed; `0` means unlimited even if `is_time_game`.
    #[serde(default = "default_game_time_seconds")]
    pub game_time_seconds: i32,
    /// Used when the client does not pass `crossword_difficulty` in session options (1–3).
    #[serde(default = "default_crossword_difficulty")]
    pub default_difficulty: u8,
}

impl Default for CrosswordConfig {
    fn default() -> Self {
        Self {
            max_learning_items_for_llm: default_max_learning_items_for_llm(),
            max_grid_rows: default_max_grid_dim(),
            max_grid_cols: default_max_grid_dim(),
            max_words: default_max_words(),
            max_hint_chars: default_max_hint_chars(),
            is_time_game: false,
            game_time_seconds: 0,
            default_difficulty: default_crossword_difficulty(),
        }
    }
}

fn default_max_sentence_words_correct_usage() -> u32 {
    15
}

/// Rules for LLM batch + UI for “choose correct usage” (one step per hard word).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectUsageConfig {
    /// Cap on `learning_items` sent to the LLM (merged with `ContentRequest.limit` in start_session).
    #[serde(default = "default_max_learning_items_for_llm")]
    pub max_learning_items_for_llm: u32,
    /// Soft cap per sentence in prompts / validation (word count, whitespace-separated).
    #[serde(default = "default_max_sentence_words_correct_usage")]
    pub max_sentence_words: u32,
}

impl Default for CorrectUsageConfig {
    fn default() -> Self {
        Self {
            max_learning_items_for_llm: default_max_learning_items_for_llm(),
            max_sentence_words: default_max_sentence_words_correct_usage(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GapFillScoringMode {
    /// Each correct gap awards `scoring_policy` points (typically FixedPerCorrect per slot).
    PerGap,
    /// One award for the step if all gaps are correct.
    AllOrNothing,
}

fn default_max_llm_gap_slots() -> u32 {
    10
}

fn default_max_llm_sentences() -> u32 {
    5
}

fn default_max_learning_items_for_llm() -> u32 {
    100
}

/// Which LLM instruction bundle to use for passage generation (`engine` crate).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GapFillLlmTemplate {
    #[default]
    Standard,
    /// Distractors may show wrong case/gender; passage uses correct forms; learner picks/writes the right inflection.
    MorphologyDistractors,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapFillPassageConfig {
    pub max_passage_words: u32,
    pub distractors_per_gap: usize,
    pub allow_skip: bool,
    pub scoring_mode: GapFillScoringMode,
    /// Max `hard_words` entries the LLM may return; also enforced after reconciliation.
    #[serde(default = "default_max_llm_gap_slots")]
    pub max_llm_gap_slots: u32,
    /// Target max sentences in `full_text` (prompt contract; not validated heuristically in code).
    #[serde(default = "default_max_llm_sentences")]
    pub max_llm_sentences: u32,
    /// Cap on `learning_items` rows loaded from DB for the LLM payload (`start_session` merges with `ContentRequest.limit`).
    /// Ignored for inline `llm_source_texts` (item count follows text array length).
    #[serde(default = "default_max_learning_items_for_llm")]
    pub max_learning_items_for_llm: u32,
    #[serde(default)]
    pub llm_template: GapFillLlmTemplate,
}

impl Default for GapFillPassageConfig {
    fn default() -> Self {
        Self {
            max_passage_words: 600,
            distractors_per_gap: 2,
            allow_skip: false,
            scoring_mode: GapFillScoringMode::PerGap,
            max_llm_gap_slots: default_max_llm_gap_slots(),
            max_llm_sentences: default_max_llm_sentences(),
            max_learning_items_for_llm: default_max_learning_items_for_llm(),
            llm_template: GapFillLlmTemplate::Standard,
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
