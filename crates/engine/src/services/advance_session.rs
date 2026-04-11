use crate::deps::EngineDeps;
use crate::errors::AppError;
use shakti_game_domain::{GameKind, GameSessionId, UserId};

fn game_kind_branch(kind: GameKind) -> &'static str {
    match kind {
        GameKind::GapFill => "gap_fill",
        GameKind::CorrectUsage => "correct_usage",
        GameKind::Crossword => "crossword",
    }
}

pub async fn advance_session(
    deps: &EngineDeps,
    session_id: GameSessionId,
    user_id: UserId,
) -> Result<shakti_game_domain::GameSession, AppError> {
    let mut session = deps.sessions.get(session_id).await?;
    let definition = session.definition().map_err(AppError::Domain)?.clone();
    let game_branch = game_kind_branch(definition.kind);
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
                "game_kind": definition.kind,
                "event_branch": game_branch,
                "current_step_index": session.current_step_index,
                "state": session.state,
            }),
        )
        .await?;
    Ok(session)
}
