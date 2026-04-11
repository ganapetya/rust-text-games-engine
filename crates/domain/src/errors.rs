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

    #[error("step not active — duplicate submit or client out of sync with the server; refresh the page and submit once per step")]
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

    #[error("missing correct usage batch in prepared content")]
    MissingCorrectUsageBatch,

    #[error("missing crossword payload in prepared content")]
    MissingCrossword,

    #[error("invalid crossword context: {0}")]
    InvalidCrosswordContext(String),
}
