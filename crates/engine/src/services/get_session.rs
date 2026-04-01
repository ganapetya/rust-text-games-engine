use crate::deps::EngineDeps;
use crate::errors::AppError;
use shakti_game_domain::{GameSession, GameSessionId, UserId};

pub async fn get_game_session(
    deps: &EngineDeps,
    session_id: GameSessionId,
    user_id: UserId,
) -> Result<GameSession, AppError> {
    let mut session = deps.sessions.get(session_id).await?;
    if session.user_id != user_id {
        return Err(AppError::Forbidden);
    }
    let now = deps.clock.now();
    let before = session.state;
    session.check_session_expired(now)?;
    if session.state != before {
        deps.sessions.update(&session).await?;
    }
    Ok(session)
}
