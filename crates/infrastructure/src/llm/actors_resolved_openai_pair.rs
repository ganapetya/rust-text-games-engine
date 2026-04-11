//! Single lazy OpenAI key fetch shared by gap-fill and full-text translation (same model).

use async_trait::async_trait;
use reqwest::Url;
use shakti_game_domain::{
    CorrectUsageLlmOutput, CrosswordHintsLlmOutput, GameDefinition, LearningItem,
    PassageGapLlmOutput, UserId,
};
use shakti_game_engine_core::{AppError, LlmContentPreparer};
use shakti_game_translation::{LlmTextTranslator, LlmTokenUsage, TranslationError, TranslationParams};
use tokio::sync::RwLock;

use super::openai_gap_fill::OpenAiGapFillPreparer;
use super::openai_text_translate::OpenAiLlmTextTranslator;

pub struct ActorsResolvedOpenAiPair {
    key_fetch_url: Url,
    model: std::sync::Arc<str>,
    http: reqwest::Client,
    cached: RwLock<Option<(OpenAiGapFillPreparer, OpenAiLlmTextTranslator)>>,
}

impl ActorsResolvedOpenAiPair {
    pub fn new(
        actors_internal_base: impl Into<String>,
        key_name: impl Into<String>,
        consumer_service: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<Self, String> {
        let base = actors_internal_base.into();
        let base = base.trim_end_matches('/').to_string();
        let key_name = key_name.into();
        let consumer = consumer_service.into();
        let full = format!("{base}/api/keys/get/{key_name}/{consumer}");
        let key_fetch_url =
            Url::parse(&full).map_err(|e| format!("invalid actors OpenAI key URL {full:?}: {e}"))?;

        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("reqwest client: {e}"))?;

        Ok(Self {
            key_fetch_url,
            model: std::sync::Arc::from(model.into()),
            http,
            cached: RwLock::new(None),
        })
    }

    async fn ensure_inner(
        &self,
    ) -> Result<(OpenAiGapFillPreparer, OpenAiLlmTextTranslator), AppError> {
        {
            let g = self.cached.read().await;
            if let Some(p) = g.as_ref() {
                return Ok(p.clone());
            }
        }

        let mut g = self.cached.write().await;
        if let Some(p) = g.as_ref() {
            return Ok(p.clone());
        }

        tracing::info!(
            url = %self.key_fetch_url,
            "fetching OpenAI API key from shakti-actors (gap-fill + translate)"
        );

        let resp = self
            .http
            .get(self.key_fetch_url.clone())
            .send()
            .await
            .map_err(|e| {
                AppError::LlmPreparation(format!(
                    "cannot reach shakti-actors for OpenAI key ({e}); check SHAKTI_ACTORS_INTERNAL_URL and network"
                ))
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let hint = if status == reqwest::StatusCode::NOT_FOUND {
                "install key via deployment install-api-keys.sh or POST /api/keys/store (keyName openai_main)"
            } else {
                "check shakti-actors logs"
            };
            return Err(AppError::LlmPreparation(format!(
                "actors returned {status} when fetching OpenAI key — {hint}"
            )));
        }

        let key = resp.text().await.map_err(|e| {
            AppError::LlmPreparation(format!("reading actors key response body: {e}"))
        })?;
        let key = key.trim();
        if key.is_empty() {
            return Err(AppError::LlmPreparation(
                "empty OpenAI key from shakti-actors".into(),
            ));
        }

        let preparer = OpenAiGapFillPreparer::new(key.to_string(), self.model.as_ref());
        let translator = OpenAiLlmTextTranslator::new(key.to_string(), self.model.as_ref());
        let pair = (preparer.clone(), translator.clone());
        *g = Some(pair.clone());
        Ok(pair)
    }
}

#[async_trait]
impl LlmContentPreparer for ActorsResolvedOpenAiPair {
    async fn build_passage_gap_context(
        &self,
        user_id: UserId,
        trace_id: Option<&str>,
        learning_items: &[LearningItem],
        registered_hard_words: &[String],
        language: &str,
        definition: &GameDefinition,
    ) -> Result<(PassageGapLlmOutput, LlmTokenUsage), AppError> {
        let (prep, _) = self.ensure_inner().await?;
        prep.build_passage_gap_context(
            user_id,
            trace_id,
            learning_items,
            registered_hard_words,
            language,
            definition,
        )
        .await
    }

    async fn build_correct_usage_context(
        &self,
        user_id: UserId,
        trace_id: Option<&str>,
        learning_items: &[LearningItem],
        registered_hard_words: &[String],
        language: &str,
        definition: &GameDefinition,
    ) -> Result<(CorrectUsageLlmOutput, LlmTokenUsage), AppError> {
        let (prep, _) = self.ensure_inner().await?;
        prep.build_correct_usage_context(
            user_id,
            trace_id,
            learning_items,
            registered_hard_words,
            language,
            definition,
        )
        .await
    }

    async fn build_crossword_hints(
        &self,
        user_id: UserId,
        trace_id: Option<&str>,
        learning_items: &[LearningItem],
        registered_hard_words: &[String],
        language: &str,
        definition: &GameDefinition,
    ) -> Result<(CrosswordHintsLlmOutput, LlmTokenUsage), AppError> {
        let (prep, _) = self.ensure_inner().await?;
        prep.build_crossword_hints(
            user_id,
            trace_id,
            learning_items,
            registered_hard_words,
            language,
            definition,
        )
        .await
    }
}

#[async_trait]
impl LlmTextTranslator for ActorsResolvedOpenAiPair {
    async fn translate(
        &self,
        user_id: &str,
        trace_id: Option<&str>,
        params: TranslationParams,
    ) -> Result<(String, LlmTokenUsage), TranslationError> {
        let (_, trans) = self
            .ensure_inner()
            .await
            .map_err(|e| TranslationError::Api(e.to_string()))?;
        trans.translate(user_id, trace_id, params).await
    }
}
