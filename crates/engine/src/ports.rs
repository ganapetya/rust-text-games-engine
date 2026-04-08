use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use shakti_game_domain::{
    GameDefinition, GameSession, GameSessionId, GameStep, LearningItem, PassageGapLlmOutput,
    UserId,
};
use time::OffsetDateTime;

use crate::errors::AppError;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContentRequest {
    pub source: String,
    pub limit: i64,
    pub language: Option<String>,
    /// When set to at least one non-empty string after trim, DB learning rows are skipped;
    /// items are synthesized for the LLM prompt ([`crate::services::start_game_session`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_source_texts: Option<Vec<String>>,
    /// When set, registered hard words are not loaded from the DB; use this list for the LLM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_hard_words: Option<Vec<String>>,
}

#[async_trait]
pub trait GameSessionRepository: Send + Sync {
    async fn insert(&self, session: &GameSession) -> Result<(), AppError>;
    async fn get(&self, id: GameSessionId) -> Result<GameSession, AppError>;
    async fn update(&self, session: &GameSession) -> Result<(), AppError>;
    /// Inserts new step rows for a session (e.g. after materializing a draft on start).
    async fn insert_steps(
        &self,
        session_id: GameSessionId,
        steps: &[GameStep],
    ) -> Result<(), AppError>;
    /// Removes all steps for a session (replay / recovery from partial start).
    async fn delete_steps(&self, session_id: GameSessionId) -> Result<(), AppError>;
    /// Inserts steps + updates session in one transaction, serialized per session id.
    /// Returns `false` if the row was no longer `draft` (another request won the race); caller should `get()`.
    async fn persist_materialized_start(&self, session: &GameSession) -> Result<bool, AppError>;
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

/// User vocabulary (per language) for passage gap-fill prompts.
#[async_trait]
pub trait HardWordsRepository: Send + Sync {
    async fn fetch_registered(
        &self,
        user_id: UserId,
        language: &str,
    ) -> Result<Vec<String>, AppError>;
}

/// Async LLM: learning history + hard words → validated [`PassageGapLlmOutput`].
#[async_trait]
pub trait LlmContentPreparer: Send + Sync {
    async fn build_passage_gap_context(
        &self,
        user_id: UserId,
        trace_id: Option<&str>,
        learning_items: &[LearningItem],
        registered_hard_words: &[String],
        language: &str,
        definition: &GameDefinition,
    ) -> Result<PassageGapLlmOutput, AppError>;
}
