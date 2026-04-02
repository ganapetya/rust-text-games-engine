use crate::ports::{
    Clock, ContentProvider, GameDefinitionRepository, GameSessionRepository, HardWordsRepository,
    LlmContentPreparer, SessionEventRepository,
};
use shakti_game_domain::GameEngineRegistry;
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
}
