//! LLM-assisted passage generation: prompts + items + vocabulary → validated [`PassageGapLlmOutput`].
//! Implementations: [`MockLlmContentPreparer`] here; OpenAI in `shakti-game-infrastructure`.

mod json;
mod mock;
mod prompt;
mod prompt_correct_usage;
mod prompt_crossword;

pub use json::{
    parse_correct_usage_response, parse_crossword_hints_response, parse_passage_gap_response,
    reconcile_hard_word_spans, strip_code_fences,
};
pub use mock::MockLlmContentPreparer;
pub use prompt::{passage_gap_system_prompt, passage_gap_user_message_json};
pub use prompt_correct_usage::{correct_usage_system_prompt, correct_usage_user_message_json};
pub use prompt_crossword::{crossword_hints_system_prompt, crossword_hints_user_message};
