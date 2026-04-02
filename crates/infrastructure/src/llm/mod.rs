//! LLM backends for [`shakti_game_engine_core::LlmContentPreparer`].
mod openai_gap_fill;

pub use openai_gap_fill::OpenAiGapFillPreparer;

use shakti_game_engine_core::llm::MockLlmContentPreparer;
use shakti_game_engine_core::LlmContentPreparer;
use std::sync::Arc;

/// Selects mock vs OpenAI at process startup (`GAME_ENGINE_LLM_MODE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmMode {
    Mock,
    OpenAi,
}

impl LlmMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "mock" => Some(LlmMode::Mock),
            "openai" => Some(LlmMode::OpenAi),
            _ => None,
        }
    }
}

/// Builds the shared [`LlmContentPreparer`] for [`shakti_game_engine_core::EngineDeps`].
pub fn build_llm_preparer(
    mode: LlmMode,
    openai_api_key: Option<String>,
    openai_model: String,
) -> Result<Arc<dyn LlmContentPreparer>, String> {
    match mode {
        LlmMode::Mock => Ok(Arc::new(MockLlmContentPreparer)),
        LlmMode::OpenAi => {
            let key = openai_api_key.filter(|s| !s.is_empty()).ok_or_else(|| {
                "OPENAI_API_KEY is required when GAME_ENGINE_LLM_MODE=openai".to_string()
            })?;
            Ok(OpenAiGapFillPreparer::new(key, openai_model).into_arc())
        }
    }
}
