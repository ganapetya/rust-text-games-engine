use async_openai::config::OpenAIConfig;
use async_openai::types::chat::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestSystemMessageContent, ChatCompletionRequestUserMessage,
    ChatCompletionRequestUserMessageContent, CreateChatCompletionRequestArgs, ResponseFormat,
};
use async_openai::Client;
use async_trait::async_trait;
use shakti_game_domain::{
    CorrectUsageLlmOutput, GameDefinition, LearningItem, PassageGapLlmOutput, UserId,
};
use shakti_game_engine_core::llm::{
    correct_usage_system_prompt, correct_usage_user_message_json, parse_correct_usage_response,
    parse_passage_gap_response, passage_gap_system_prompt, passage_gap_user_message_json,
    reconcile_hard_word_spans,
};
use shakti_game_engine_core::{AppError, LlmContentPreparer};
use shakti_game_translation::LlmTokenUsage;
use std::sync::Arc;

/// OpenAI Chat Completions → JSON → validated [`PassageGapLlmOutput`].
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
    async fn build_passage_gap_context(
        &self,
        user_id: UserId,
        trace_id: Option<&str>,
        learning_items: &[LearningItem],
        registered_hard_words: &[String],
        language: &str,
        definition: &GameDefinition,
    ) -> Result<(PassageGapLlmOutput, LlmTokenUsage), AppError> {
        let gap = definition.gap_fill_config().map_err(AppError::from)?;

        tracing::info!(
            user_id = %user_id.0,
            trace_id = trace_id.unwrap_or(""),
            model = %self.model,
            items_in = learning_items.len(),
            "llm passage gap build (openai)"
        );

        let system_msg = ChatCompletionRequestSystemMessage {
            content: ChatCompletionRequestSystemMessageContent::Text(
                passage_gap_system_prompt(gap, language),
            ),
            name: None,
        };

        let user_json = serde_json::to_string(&passage_gap_user_message_json(
            learning_items,
            registered_hard_words,
            language,
            gap,
        ))
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

        let usage = response
            .usage
            .map(|u| LlmTokenUsage {
                prompt_tokens: u.prompt_tokens as u64,
                completion_tokens: u.completion_tokens as u64,
            })
            .unwrap_or_default();

        let text = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .ok_or_else(|| AppError::LlmPreparation("empty openai response".into()))?;

        let mut out =
            parse_passage_gap_response(&text).map_err(AppError::LlmPreparation)?;
        let model_gap_count = out.hard_words.len();
        reconcile_hard_word_spans(&mut out).map_err(AppError::LlmPreparation)?;
        tracing::info!(
            user_id = %user_id.0,
            trace_id = trace_id.unwrap_or(""),
            model_gap_entries = model_gap_count,
            kept_gaps = out.hard_words.len(),
            "passage hard_words reconciled (orphan model entries dropped if not in full_text)"
        );
        out.validate_against_gap_fill_config(gap)
            .map_err(|e| AppError::LlmPreparation(e.to_string()))?;
        Ok((out, usage))
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
        let cfg = definition.correct_usage_config().map_err(AppError::from)?;

        tracing::info!(
            user_id = %user_id.0,
            trace_id = trace_id.unwrap_or(""),
            model = %self.model,
            items_in = learning_items.len(),
            words = registered_hard_words.len(),
            "llm correct_usage build (openai)"
        );

        let system_msg = ChatCompletionRequestSystemMessage {
            content: ChatCompletionRequestSystemMessageContent::Text(
                correct_usage_system_prompt(cfg, language),
            ),
            name: None,
        };

        let user_json = serde_json::to_string(&correct_usage_user_message_json(
            learning_items,
            registered_hard_words,
            language,
            cfg,
        ))
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

        let usage = response
            .usage
            .map(|u| LlmTokenUsage {
                prompt_tokens: u.prompt_tokens as u64,
                completion_tokens: u.completion_tokens as u64,
            })
            .unwrap_or_default();

        let text = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .ok_or_else(|| AppError::LlmPreparation("empty openai response".into()))?;

        let out = parse_correct_usage_response(&text).map_err(AppError::LlmPreparation)?;
        out.validate(registered_hard_words, cfg.max_sentence_words)
            .map_err(AppError::from)?;
        Ok((out, usage))
    }
}
