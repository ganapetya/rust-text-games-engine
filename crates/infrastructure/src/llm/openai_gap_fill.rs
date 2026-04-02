use async_openai::config::OpenAIConfig;
use async_openai::types::chat::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestSystemMessageContent, ChatCompletionRequestUserMessage,
    ChatCompletionRequestUserMessageContent, CreateChatCompletionRequestArgs, ResponseFormat,
};
use async_openai::Client;
use async_trait::async_trait;
use shakti_game_domain::{GapFillConfig, LearningItem, UserId};
use shakti_game_engine_core::llm::{
    gap_fill_system_prompt, gap_fill_user_message_json, parse_gap_fill_response,
};
use shakti_game_engine_core::{AppError, LlmContentPreparer};
use std::sync::Arc;

/// OpenAI Chat Completions → JSON envelope → validated [`LearningItem`] list.
///
/// Wire via [`crate::build_llm_preparer`] when `GAME_ENGINE_LLM_MODE=openai`.
#[derive(Clone)]
pub struct OpenAiGapFillPreparer {
    client: Client<OpenAIConfig>,
    model: Arc<str>,
}

impl OpenAiGapFillPreparer {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        let key = api_key.into();
        let config = OpenAIConfig::new().with_api_key(key);
        Self {
            client: Client::with_config(config),
            model: Arc::from(model.into()),
        }
    }

    pub fn into_arc(self) -> Arc<dyn LlmContentPreparer> {
        Arc::new(self)
    }
}

#[async_trait]
impl LlmContentPreparer for OpenAiGapFillPreparer {
    async fn prepare_gap_fill_learning_items(
        &self,
        user_id: UserId,
        trace_id: Option<&str>,
        items: &[LearningItem],
        config: &GapFillConfig,
    ) -> Result<Vec<LearningItem>, AppError> {
        tracing::info!(
            user_id = %user_id.0,
            trace_id = trace_id.unwrap_or(""),
            model = %self.model,
            items_in = items.len(),
            "llm gap-fill preparation (openai)"
        );

        let system_msg = ChatCompletionRequestSystemMessage {
            content: ChatCompletionRequestSystemMessageContent::Text(
                gap_fill_system_prompt().into(),
            ),
            name: None,
        };

        let user_json = serde_json::to_string(&gap_fill_user_message_json(items, config))
            .map_err(|e| AppError::LlmPreparation(e.to_string()))?;

        let user_msg = ChatCompletionRequestUserMessage {
            content: ChatCompletionRequestUserMessageContent::Text(user_json),
            name: None,
        };

        let messages = vec![
            ChatCompletionRequestMessage::System(system_msg),
            ChatCompletionRequestMessage::User(user_msg),
        ];

        let request = CreateChatCompletionRequestArgs::default()
            .model(self.model.to_string())
            .messages(messages)
            .response_format(ResponseFormat::JsonObject)
            .build()
            .map_err(|e| AppError::LlmPreparation(format!("openai request build: {e}")))?;

        let response = self
            .client
            .chat()
            .create(request)
            .await
            .map_err(|e| AppError::LlmPreparation(format!("openai chat completion: {e}")))?;

        let text = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .ok_or_else(|| AppError::LlmPreparation("empty openai response".into()))?;

        parse_gap_fill_response(&text).map_err(AppError::LlmPreparation)
    }
}
