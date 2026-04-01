use crate::deps::ApplicationDeps;
use crate::errors::AppError;
use shakti_game_domain::{GameSessionId, GameSessionState};

pub async fn start_game_session(
    deps: &ApplicationDeps,
    session_id: GameSessionId,
    user_id: shakti_game_domain::UserId,
) -> Result<shakti_game_domain::GameSession, AppError> {
    let mut session = deps.sessions.get(session_id).await?;
    if session.user_id != user_id {
        return Err(AppError::Forbidden);
    }
    if session.state != GameSessionState::Prepared {
        return Err(AppError::Conflict(format!(
            "session must be prepared, got {:?}",
            session.state
        )));
    }
    let now = deps.clock.now();
    session.start(now)?;
    deps.sessions.update(&session).await?;
    deps.events
        .append(
            session_id,
            "session_started",
            serde_json::json!({ "at": now }),
        )
        .await?;
    Ok(session)
}
