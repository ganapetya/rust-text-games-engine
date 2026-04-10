use async_trait::async_trait;

use crate::{LlmTextTranslator, LlmTokenUsage, TranslationError, TranslationParams};

/// Deterministic stub for tests and `GAME_ENGINE_LLM_MODE=mock`.
#[derive(Debug, Default, Clone)]
pub struct MockLlmTextTranslator;

#[async_trait]
impl LlmTextTranslator for MockLlmTextTranslator {
    async fn translate(
        &self,
        user_id: &str,
        trace_id: Option<&str>,
        params: TranslationParams,
    ) -> Result<(String, LlmTokenUsage), TranslationError> {
        tracing::info!(
            user_id = %user_id,
            trace_id = trace_id.unwrap_or(""),
            source_lang = %params.source_lang,
            target_lang = %params.target_lang,
            "mock llm translate"
        );
        Ok((
            format!(
                "[{}→{}] {}",
                params.source_lang, params.target_lang, params.text
            ),
            LlmTokenUsage::default(),
        ))
    }
}
