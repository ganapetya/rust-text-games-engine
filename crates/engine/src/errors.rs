use shakti_game_domain::DomainError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error(transparent)]
    Domain(#[from] DomainError),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("forbidden")]
    Forbidden,

    #[error("repository error: {0}")]
    Repository(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("llm preparation failed: {0}")]
    LlmPreparation(String),
}
