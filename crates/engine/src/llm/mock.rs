use async_trait::async_trait;
use serde_json::json;
use shakti_game_domain::{GapFillConfig, LearningItem, UserId};

use crate::errors::AppError;
use crate::ports::LlmContentPreparer;

/// Deterministic stand-in for an OpenAI call: enriches metadata and returns the same items.
#[derive(Debug, Default, Clone)]
pub struct MockLlmContentPreparer;

#[async_trait]
impl LlmContentPreparer for MockLlmContentPreparer {
    async fn prepare_gap_fill_learning_items(
        &self,
        user_id: UserId,
        trace_id: Option<&str>,
        items: &[LearningItem],
        _config: &GapFillConfig,
    ) -> Result<Vec<LearningItem>, AppError> {
        tracing::info!(
            user_id = %user_id.0,
            trace_id = trace_id.unwrap_or(""),
            mode = "mock",
            items_in = items.len(),
            "llm gap-fill preparation (mock)"
        );
        let mut out = Vec::with_capacity(items.len());
        for mut li in items.iter().cloned() {
            let mut meta = li
                .metadata
                .as_object()
                .cloned()
                .unwrap_or_default();
            meta.insert(
                "llm_preparation".to_string(),
                json!({ "mode": "mock", "prepared": true }),
            );
            li.metadata = serde_json::Value::Object(meta);
            out.push(li);
        }
        Ok(out)
    }
}
