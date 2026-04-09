use crate::ports::{
    Clock, ContentProvider, GameDefinitionRepository, GameSessionRepository, HardWordsRepository,
    LlmContentPreparer, SessionEventRepository,
};
use shakti_game_domain::GameEngineRegistry;
use shakti_game_translation::LlmTextTranslator;
use std::sync::Arc;

pub struct EngineDeps {
    pub sessions: Arc<dyn GameSessionRepository>,
    pub definitions: Arc<dyn GameDefinitionRepository>,
    pub content: Arc<dyn ContentProvider>,
    pub hard_words: Arc<dyn HardWordsRepository>,
    pub events: Arc<dyn SessionEventRepository>,
    pub clock: Arc<dyn Clock>,
    pub engines: Arc<GameEngineRegistry>,
    pub llm_preparer: Arc<dyn LlmContentPreparer>,
    pub llm_translator: Arc<dyn LlmTextTranslator>,
    /// When true, session materialization may attach dev-only fields to `base_context` and the API may expose them.
    pub dev_expose_gap_solution: bool,
}
