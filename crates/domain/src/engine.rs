use crate::answer::{StepEvaluation, UserAnswer};
use crate::content::{LearningItem, PreparedContent};
use crate::errors::DomainError;
use crate::game_session::GameSession;
use crate::game_step::GameStep;
use crate::policies::{GameDefinition, GameKind, GapFillConfig};
use crate::result::GameResult;
use std::collections::HashMap;
use std::sync::Arc;
use time::OffsetDateTime;

/// Per-game-type engine: pure, synchronous.
pub trait GameEngine: Send + Sync {
    fn kind(&self) -> GameKind;

    fn prepare_content(
        &self,
        input: &[LearningItem],
        config: &GapFillConfig,
    ) -> Result<PreparedContent, DomainError>;

    fn generate_steps(
        &self,
        content: &PreparedContent,
        config: &GapFillConfig,
    ) -> Result<Vec<GameStep>, DomainError>;

    fn evaluate_answer(
        &self,
        step: &GameStep,
        answer: &UserAnswer,
        _now: OffsetDateTime,
        config: &GapFillConfig,
    ) -> Result<StepEvaluation, DomainError>;

    fn finalize(
        &self,
        session: &GameSession,
        definition: &GameDefinition,
    ) -> Result<GameResult, DomainError>;
}

pub struct GameEngineRegistry {
    engines: HashMap<GameKind, Arc<dyn GameEngine>>,
}

impl GameEngineRegistry {
    pub fn new() -> Self {
        Self {
            engines: HashMap::new(),
        }
    }

    pub fn register(&mut self, engine: Arc<dyn GameEngine>) {
        self.engines.insert(engine.kind(), engine);
    }

    pub fn get(&self, kind: GameKind) -> Result<Arc<dyn GameEngine>, DomainError> {
        self.engines
            .get(&kind)
            .cloned()
            .ok_or(DomainError::UnsupportedGameKind)
    }
}

impl Default for GameEngineRegistry {
    fn default() -> Self {
        Self::new()
    }
}
