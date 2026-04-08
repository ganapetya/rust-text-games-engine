//! LLM backends for [`shakti_game_engine_core::LlmContentPreparer`].
mod actors_resolving_openai_gap_fill;
mod openai_gap_fill;

pub use openai_gap_fill::OpenAiGapFillPreparer;

use actors_resolving_openai_gap_fill::ActorsResolvedOpenAiGapFillPreparer;

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
///
/// OpenAI key resolution (in order): non-empty `openai_api_key`, else
/// [`ActorsResolvedOpenAiGapFillPreparer`] when `shakti_actors_internal_base` is set
/// (fetches `openai_main` from shakti-actors on first LLM use).
pub fn build_llm_preparer(
    mode: LlmMode,
    openai_api_key: Option<String>,
    openai_model: String,
    shakti_actors_internal_base: Option<String>,
    shakti_actors_openai_key_name: String,
    shakti_actors_key_consumer_service: String,
) -> Result<Arc<dyn LlmContentPreparer>, String> {
    match mode {
        LlmMode::Mock => Ok(Arc::new(MockLlmContentPreparer)),
        LlmMode::OpenAi => {
            if let Some(key) = openai_api_key.filter(|s| !s.is_empty()) {
                return Ok(OpenAiGapFillPreparer::new(key, openai_model).into_arc());
            }
            if let Some(base) = shakti_actors_internal_base.map(|s| s.trim().to_string()) {
                if !base.is_empty() {
                    return ActorsResolvedOpenAiGapFillPreparer::new(
                        base,
                        shakti_actors_openai_key_name,
                        shakti_actors_key_consumer_service,
                        openai_model,
                    )
                    .map(|p| p.into_arc());
                }
            }
            Err(
                "OpenAI LLM mode: set OPENAI_API_KEY / openai.key.secret, or SHAKTI_ACTORS_INTERNAL_URL (key openai_main in Shakti DB)"
                    .to_string(),
            )
        }
    }
}
