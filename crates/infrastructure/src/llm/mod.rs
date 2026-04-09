//! LLM backends for [`shakti_game_engine_core::LlmContentPreparer`] and [`shakti_game_translation::LlmTextTranslator`].
mod actors_resolved_openai_pair;
mod openai_gap_fill;
mod openai_text_translate;

pub use openai_gap_fill::OpenAiGapFillPreparer;
pub use openai_text_translate::OpenAiLlmTextTranslator;

use actors_resolved_openai_pair::ActorsResolvedOpenAiPair;

use shakti_game_engine_core::llm::MockLlmContentPreparer;
use shakti_game_engine_core::LlmContentPreparer;
use shakti_game_translation::MockLlmTextTranslator;
use shakti_game_translation::LlmTextTranslator;
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

/// Builds shared gap-fill preparer + full-text translator for [`shakti_game_engine_core::EngineDeps`].
///
/// OpenAI key resolution (in order): non-empty `openai_api_key`, else
/// [`ActorsResolvedOpenAiPair`] when `shakti_actors_internal_base` is set
/// (single lazy fetch for `openai_main` — shared by passage LLM and translation).
pub fn build_llm_stack(
    mode: LlmMode,
    openai_api_key: Option<String>,
    openai_model: String,
    shakti_actors_internal_base: Option<String>,
    shakti_actors_openai_key_name: String,
    shakti_actors_key_consumer_service: String,
) -> Result<(Arc<dyn LlmContentPreparer>, Arc<dyn LlmTextTranslator>), String> {
    match mode {
        LlmMode::Mock => Ok((
            Arc::new(MockLlmContentPreparer),
            Arc::new(MockLlmTextTranslator),
        )),
        LlmMode::OpenAi => {
            if let Some(key) = openai_api_key.filter(|s| !s.is_empty()) {
                let prep = OpenAiGapFillPreparer::new(key.clone(), openai_model.as_str()).into_arc();
                let trans = OpenAiLlmTextTranslator::new(key, openai_model.as_str()).into_arc();
                return Ok((prep, trans));
            }
            if let Some(base) = shakti_actors_internal_base.map(|s| s.trim().to_string()) {
                if !base.is_empty() {
                    let pair = Arc::new(ActorsResolvedOpenAiPair::new(
                        base,
                        shakti_actors_openai_key_name,
                        shakti_actors_key_consumer_service,
                        openai_model,
                    )?);
                    let preparer: Arc<dyn LlmContentPreparer> = pair.clone() as Arc<dyn LlmContentPreparer>;
                    let translator: Arc<dyn LlmTextTranslator> = pair as Arc<dyn LlmTextTranslator>;
                    return Ok((preparer, translator));
                }
            }
            Err(
                "OpenAI LLM mode: set OPENAI_API_KEY / openai.key.secret, or SHAKTI_ACTORS_INTERNAL_URL (key openai_main in Shakti DB)"
                    .to_string(),
            )
        }
    }
}
