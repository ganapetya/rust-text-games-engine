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
                "OpenAI API key required: set OPENAI_API_KEY, or place the key in openai.key.secret (or OPENAI_KEY_FILE), when using openai LLM mode"
                    .to_string()
            })?;
            Ok(OpenAiGapFillPreparer::new(key, openai_model).into_arc())
        }
    }
}
