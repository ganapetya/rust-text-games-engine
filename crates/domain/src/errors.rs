use crate::game_session::GameSessionState;
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    #[error("invalid session transition from {from:?} to action requiring {expected:?}")]
    InvalidSessionState {
        from: GameSessionState,
        expected: GameSessionState,
    },

    #[error("step already answered")]
    StepAlreadyAnswered,

    #[error("step not active")]
    StepNotActive,

    #[error("step timed out")]
    StepTimedOut,

    #[error("wrong step for current index")]
    WrongStep,

    #[error("session already completed")]
    SessionCompleted,

    #[error("unsupported game kind")]
    UnsupportedGameKind,

    #[error("invalid transition: {0}")]
    InvalidTransition(String),

    #[error("no steps generated")]
    NoSteps,

    #[error("not enough learning items (need {need}, got {got})")]
    NotEnoughItems { need: usize, got: usize },

    #[error("invalid passage context: {0}")]
    InvalidPassageContext(String),

    #[error("missing passage in prepared content")]
    MissingPassage,
}
