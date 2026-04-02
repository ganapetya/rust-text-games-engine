//! LLM-assisted passage generation: prompts + items + vocabulary → validated [`PassageGapLlmOutput`].
//! Implementations: [`MockLlmContentPreparer`] here; OpenAI in `shakti-game-infrastructure`.

mod json;
mod mock;
mod prompt;

pub use json::{
    parse_passage_gap_response, reconcile_hard_word_spans, strip_code_fences,
};
pub use mock::MockLlmContentPreparer;
pub use prompt::{passage_gap_system_prompt, passage_gap_user_message_json};
