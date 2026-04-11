use crate::deps::EngineDeps;
use crate::errors::AppError;
use shakti_game_domain::{GameKind, GameSessionId, GameStepId, UserAnswer};

fn game_kind_branch(kind: GameKind) -> &'static str {
    match kind {
        GameKind::GapFill => "gap_fill",
        GameKind::CorrectUsage => "correct_usage",
        GameKind::Crossword => "crossword",
    }
}

pub struct SubmitAnswerCommand {
    pub session_id: GameSessionId,
    pub step_id: GameStepId,
    pub user_id: shakti_game_domain::UserId,
    pub answer: UserAnswer,
}

pub async fn submit_answer(
    deps: &EngineDeps,
    cmd: SubmitAnswerCommand,
) -> Result<shakti_game_domain::GameSession, AppError> {
    let mut session = deps.sessions.get(cmd.session_id).await?;
    if session.user_id != cmd.user_id {
        return Err(AppError::Forbidden);
    }

    let now = deps.clock.now();
    let before = session.state;
    session.check_session_expired(now)?;
    if session.state != before {
        deps.sessions.update(&session).await?;
        return Err(AppError::Conflict("session timed out".into()));
    }
    if session.state == shakti_game_domain::GameSessionState::Completed
        || session.state == shakti_game_domain::GameSessionState::TimedOut
    {
        return Err(AppError::Conflict("session already finished".into()));
    }

    let step_index = session
        .steps
        .iter()
        .position(|s| s.id == cmd.step_id)
        .ok_or_else(|| AppError::NotFound("step not found".into()))?;

    if step_index != session.current_step_index {
        return Err(AppError::BadRequest("not the current step".into()));
    }

    if session.is_step_timed_out(step_index, now) {
        session.timeout_current_step(now)?;
        deps.sessions.update(&session).await?;
        deps.events
            .append(
                cmd.session_id,
                "step_timed_out",
                serde_json::json!({ "step_index": step_index }),
            )
            .await?;
        return Ok(session);
    }

    let step = session
        .steps
        .get(step_index)
        .ok_or_else(|| AppError::NotFound("step".into()))?
        .clone();

    let definition = session.definition().map_err(AppError::Domain)?.clone();
    let game_branch = game_kind_branch(definition.kind);
    let engine = deps.engines.get(definition.kind)?;
    let evaluation = engine.evaluate_answer(&step, &cmd.answer, now, &definition)?; // per-gap or all-or-nothing scoring

    let submitted = cmd.answer.clone();
    session.record_evaluation(step_index, evaluation.clone(), submitted, now)?; // persists score, may complete session
    // Correct-usage is an explicitly multi-step game: submit deterministically advances
    // to the next active step (except on the final step, where state becomes completed).
    let mut auto_advanced = false;
    if definition.kind == GameKind::CorrectUsage
        && session.state == shakti_game_domain::GameSessionState::InProgress
    {
        session.advance(now)?;
        auto_advanced = true;
    }
    deps.sessions.update(&session).await?; // write `user_answer` + `evaluation` JSON on the step row
    deps.events
        .append(
            cmd.session_id,
            "answer_submitted",
            serde_json::json!({
                "step_id": cmd.step_id,
                "step_index": step_index,
                "game_kind": definition.kind,
                "event_branch": game_branch,
                "correct": evaluation.is_correct,
                "points": evaluation.awarded_points,
                "gap_stats": evaluation.gap_stats,
                "auto_advanced": auto_advanced,
            }),
        )
        .await?;
    if auto_advanced {
        deps.events
            .append(
                cmd.session_id,
                "step_advanced",
                serde_json::json!({
                    "game_kind": definition.kind,
                    "event_branch": game_branch,
                    "trigger": "submit_auto",
                    "current_step_index": session.current_step_index,
                    "state": session.state,
                }),
            )
            .await?;
    }

    Ok(session)
}
