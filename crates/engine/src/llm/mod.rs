//! LLM-assisted preparation: prompt + learning items → validated JSON → domain [`LearningItem`]s.
//! Implementations live here ([`MockLlmContentPreparer`]) and in `shakti-game-infrastructure` (OpenAI).

mod json;
mod mock;
mod prompt;

pub use json::{parse_gap_fill_response, strip_code_fences};
pub use mock::MockLlmContentPreparer;
pub use prompt::{gap_fill_system_prompt, gap_fill_user_message_json};
