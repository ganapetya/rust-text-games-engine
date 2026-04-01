use crate::ports::{
    Clock, ContentProvider, GameDefinitionRepository, GameSessionRepository, SessionEventRepository,
};
use shakti_game_domain::GameEngineRegistry;
use std::sync::Arc;

pub struct ApplicationDeps {
    pub sessions: Arc<dyn GameSessionRepository>,
    pub definitions: Arc<dyn GameDefinitionRepository>,
    pub content: Arc<dyn ContentProvider>,
    pub events: Arc<dyn SessionEventRepository>,
    pub clock: Arc<dyn Clock>,
    pub engines: Arc<GameEngineRegistry>,
}
