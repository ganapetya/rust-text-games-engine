use crate::correct_usage_llm::CorrectUsageLlmOutput;
use crate::crossword::CrosswordLlmOutput;
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
    /// Set after LLM validation for `correct_usage` sessions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correct_usage_batch: Option<CorrectUsageLlmOutput>,
    /// Set after LLM validation for `crossword` sessions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crossword: Option<CrosswordLlmOutput>,
    /// Deterministic seed for UI randomization (e.g. crossword difficulty picks).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_seed: Option<u64>,
    /// Session language for RTL/LTR grid layout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crossword_ui_language: Option<String>,
    /// Session difficulty 1–3 when set by `start_session` / `play_again`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crossword_difficulty: Option<u8>,
}
