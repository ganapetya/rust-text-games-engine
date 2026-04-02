pub mod deps;
pub mod errors;
pub mod llm;
pub mod ports;
pub mod services;

pub use deps::EngineDeps;
pub use errors::AppError;
pub use ports::{
    Clock, ContentProvider, ContentRequest, GameDefinitionRepository, GameSessionRepository,
    HardWordsRepository, LlmContentPreparer, SessionEventRepository,
};
pub use services::{
    advance_session, create_game_session, get_game_result, get_game_session, start_game_session,
    submit_answer, CreateGameSessionCommand, SessionOptions, SubmitAnswerCommand,
};
