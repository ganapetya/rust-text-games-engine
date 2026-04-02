use crate::ids::{LearningItemId, UserId};
use crate::passage::PassageGapLlmOutput;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningItem {
    pub id: LearningItemId,
    pub user_id: UserId,
    pub source_text: String,
    pub context_text: Option<String>,
    pub hard_fragment: String,
    pub lemma: Option<String>,
    pub language: String,
    pub metadata: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedItem {
    pub learning_item_id: LearningItemId,
    pub payload: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentProvenance {
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedContent {
    pub items: Vec<PreparedItem>,
    pub provenance: ContentProvenance,
    /// Set after LLM validation when building a passage session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passage: Option<PassageGapLlmOutput>,
}
