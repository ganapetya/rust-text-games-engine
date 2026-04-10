pub mod answer;
pub mod content;
pub mod correct_usage_llm;
pub mod engine;
pub mod errors;
pub mod game_session;
pub mod game_step;
pub mod gap_fill;
pub mod ids;
pub mod passage;
pub mod policies;
pub mod result;
pub mod score;
pub mod usage_quiz;

pub use answer::{ExpectedAnswer, StepEvaluation, UserAnswer};
pub use content::{LearningItem, PreparedContent, PreparedItem};
pub use engine::{GameEngine, GameEngineRegistry};
pub use errors::DomainError;
pub use game_session::{GameSession, GameSessionState};
pub use game_step::{GameStep, GapFillSlotPublic, StepState, UserFacingStepPrompt};
pub use gap_fill::GapFillEngine;
pub use ids::{GameDefinitionId, GameSessionId, GameStepId, LearningItemId, UserId};
pub use passage::{PassageGapLlmOutput, PassageHardWordOccurrence, PASSAGE_LLM_SCHEMA_VERSION};
pub use policies::{
    CorrectUsageConfig, GameConfig, GameDefinition, GameKind, GapFillLlmTemplate,
    GapFillPassageConfig, GapFillScoringMode, ScoringPolicy, TimingPolicy,
};
pub use correct_usage_llm::{
    CorrectUsageLlmOutput, CorrectUsagePuzzleLlm, CORRECT_USAGE_LLM_SCHEMA_VERSION,
};
pub use usage_quiz::CorrectUsageEngine;
pub use result::GameResult;
pub use score::Score;
