use async_trait::async_trait;
use shakti_game_domain::{
    GameDefinition, GameSession, GameSessionId, GapFillConfig, LearningItem, UserId,
};
use time::OffsetDateTime;

use crate::errors::AppError;

#[derive(Debug, Clone, Default)]
pub struct ContentRequest {
    pub source: String,
    pub limit: i64,
    pub language: Option<String>,
}

#[async_trait]
pub trait GameSessionRepository: Send + Sync {
    async fn insert(&self, session: &GameSession) -> Result<(), AppError>;
    async fn get(&self, id: GameSessionId) -> Result<GameSession, AppError>;
    async fn update(&self, session: &GameSession) -> Result<(), AppError>;
}

#[async_trait]
pub trait GameDefinitionRepository: Send + Sync {
    async fn get(
        &self,
        id: shakti_game_domain::GameDefinitionId,
    ) -> Result<GameDefinition, AppError>;
    async fn get_default_gap_fill(&self) -> Result<GameDefinition, AppError>;
}

#[async_trait]
pub trait SessionEventRepository: Send + Sync {
    async fn append(
        &self,
        session_id: GameSessionId,
        event_type: &str,
        payload: serde_json::Value,
    ) -> Result<(), AppError>;
}

pub trait Clock: Send + Sync {
    fn now(&self) -> OffsetDateTime;
}

#[async_trait]
pub trait ContentProvider: Send + Sync {
    async fn fetch_learning_items(
        &self,
        user_id: UserId,
        request: ContentRequest,
    ) -> Result<Vec<LearningItem>, AppError>;
}

/// Async LLM preparation: model instructions + items → validated [`LearningItem`] list for [`GapFillEngine`].
#[async_trait]
pub trait LlmContentPreparer: Send + Sync {
    async fn prepare_gap_fill_learning_items(
        &self,
        user_id: UserId,
        trace_id: Option<&str>,
        items: &[LearningItem],
        config: &GapFillConfig,
    ) -> Result<Vec<LearningItem>, AppError>;
}
