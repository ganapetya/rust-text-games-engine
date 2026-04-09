//! Reusable LLM **full-text translation** for Shakti game engine (and future game kinds).
//! OpenAI (and other) adapters live in `shakti-game-infrastructure`.

mod mock;
mod prompt;

pub use mock::MockLlmTextTranslator;
pub use prompt::{translation_system_prompt, translation_user_message_json};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationParams {
    pub source_lang: String,
    pub target_lang: String,
    pub text: String,
}

#[derive(Debug, thiserror::Error)]
pub enum TranslationError {
    #[error("empty translated text from model")]
    EmptyResponse,
    #[error("invalid model JSON: {0}")]
    InvalidJson(String),
    #[error("api: {0}")]
    Api(String),
}

/// Async full-text translation (same model family as gap-fill in production).
#[async_trait]
pub trait LlmTextTranslator: Send + Sync {
    async fn translate(
        &self,
        user_id: &str,
        trace_id: Option<&str>,
        params: TranslationParams,
    ) -> Result<String, TranslationError>;
}
