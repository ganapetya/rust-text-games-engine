use crate::deps::ApplicationDeps;
use crate::errors::AppError;
use shakti_game_domain::{GameSessionId, GameSessionState, UserId};

pub async fn get_game_result(
    deps: &ApplicationDeps,
    session_id: GameSessionId,
    user_id: UserId,
) -> Result<shakti_game_domain::GameResult, AppError> {
    let session = deps.sessions.get(session_id).await?;
    if session.user_id != user_id {
        return Err(AppError::Forbidden);
    }
    if !matches!(
        session.state,
        GameSessionState::Completed | GameSessionState::TimedOut
    ) {
        return Err(AppError::Conflict(format!(
            "session not finished: {:?}",
            session.state
        )));
    }
    let definition = session.definition().map_err(AppError::Domain)?.clone();
    let engine = deps.engines.get(definition.kind)?;
    engine
        .finalize(&session, &definition)
        .map_err(AppError::Domain)
}
