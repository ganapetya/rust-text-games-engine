pub mod billing;
pub mod deps;
pub mod errors;
pub mod llm;
pub mod ports;
pub mod services;

pub use deps::EngineDeps;
pub use errors::AppError;
pub use billing::{read_wallet_from_base, wallet_from_deferred, SessionWalletState};
pub use ports::{
    BillingChargeScheduler, Clock, ContentProvider, ContentRequest, DynBillingChargeScheduler,
    GameDefinitionRepository, GameLlmChargeArgs, GameSessionRepository, HardWordsRepository,
    LlmContentPreparer, SessionBillingBootstrap, SessionEventRepository,
};
pub use services::{
    advance_session, create_game_session, get_game_result, get_game_session, play_again_gap_fill,
    read_session_ui_hints, request_translation_hint, start_game_session, submit_answer,
    CreateGameSessionCommand, SessionOptions, SubmitAnswerCommand, TranslationHintOutput,
};
