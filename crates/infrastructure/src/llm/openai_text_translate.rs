use async_openai::config::OpenAIConfig;
use async_openai::types::chat::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestSystemMessageContent, ChatCompletionRequestUserMessage,
    ChatCompletionRequestUserMessageContent, CreateChatCompletionRequestArgs, ResponseFormat,
};
use async_openai::Client;
use async_trait::async_trait;
use serde::Deserialize;
use shakti_game_engine_core::llm::strip_code_fences;
use shakti_game_translation::{
    translation_system_prompt, translation_user_message_json, LlmTextTranslator, LlmTokenUsage,
    TranslationError, TranslationParams,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct OpenAiLlmTextTranslator {
    client: Client<OpenAIConfig>,
    model: Arc<str>,
}

impl OpenAiLlmTextTranslator {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        let key = api_key.into();
        let config = OpenAIConfig::new().with_api_key(key);
        Self {
            client: Client::with_config(config),
            model: Arc::from(model.into()),
        }
    }

    pub fn into_arc(self) -> Arc<dyn LlmTextTranslator> {
        Arc::new(self)
    }
}

#[derive(Deserialize)]
struct TranslateLlmJson {
    translated_text: String,
}

#[async_trait]
impl LlmTextTranslator for OpenAiLlmTextTranslator {
    async fn translate(
        &self,
        user_id: &str,
        trace_id: Option<&str>,
        params: TranslationParams,
    ) -> Result<(String, LlmTokenUsage), TranslationError> {
        tracing::info!(
            user_id = %user_id,
            trace_id = trace_id.unwrap_or(""),
            model = %self.model,
            source_lang = %params.source_lang,
            target_lang = %params.target_lang,
            chars_in = params.text.chars().count(),
            "llm full-text translate (openai)"
        );

        let system_msg = ChatCompletionRequestSystemMessage {
            content: ChatCompletionRequestSystemMessageContent::Text(
                translation_system_prompt().to_string(),
            ),
            name: None,
        };

        let user_json = serde_json::to_string(&translation_user_message_json(
            &params.source_lang,
            &params.target_lang,
            &params.text,
        ))
        .map_err(|e| TranslationError::Api(e.to_string()))?;

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
            .map_err(|e| TranslationError::Api(format!("openai request build: {e}")))?;

        let response = self
            .client
            .chat()
            .create(request)
            .await
            .map_err(|e| TranslationError::Api(format!("openai chat completion: {e}")))?;

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
            .ok_or(TranslationError::EmptyResponse)?;

        let cleaned = strip_code_fences(&text);
        let parsed: TranslateLlmJson = serde_json::from_str(cleaned.trim()).map_err(|e| {
            TranslationError::InvalidJson(format!("{e}; snippet: {}", cleaned.chars().take(200).collect::<String>()))
        })?;

        let out = parsed.translated_text.trim().to_string();
        if out.is_empty() {
            return Err(TranslationError::EmptyResponse);
        }
        Ok((out, usage))
    }
}
