use crate::deps::EngineDeps;
use crate::errors::AppError;
use shakti_game_domain::{GameSessionId, UserId};

pub async fn advance_session(
    deps: &EngineDeps,
    session_id: GameSessionId,
    user_id: UserId,
) -> Result<shakti_game_domain::GameSession, AppError> {
    let mut session = deps.sessions.get(session_id).await?;
    if session.user_id != user_id {
        return Err(AppError::Forbidden);
    }
    let now = deps.clock.now();
    let before = session.state;
    session.check_session_expired(now)?;
    if session.state != before {
        deps.sessions.update(&session).await?;
        return Err(AppError::Conflict("session timed out".into()));
    }
    session.advance(now)?;
    deps.sessions.update(&session).await?;
    deps.events
        .append(
            session_id,
            "step_advanced",
            serde_json::json!({
                "current_step_index": session.current_step_index,
                "state": session.state,
            }),
        )
        .await?;
    Ok(session)
}
