use shakti_game_domain::{GameSessionState, GameStep, StepState};
use shakti_game_engine_core::AppError;
use uuid::Uuid;

pub fn session_state_to_db(s: GameSessionState) -> &'static str {
    match s {
        GameSessionState::Draft => "draft",
        GameSessionState::Prepared => "prepared",
        GameSessionState::InProgress => "in_progress",
        GameSessionState::Completed => "completed",
        GameSessionState::TimedOut => "timed_out",
        GameSessionState::Cancelled => "cancelled",
    }
}

pub fn session_state_from_db(s: &str) -> Result<GameSessionState, AppError> {
    match s {
        "draft" => Ok(GameSessionState::Draft),
        "prepared" => Ok(GameSessionState::Prepared),
        "in_progress" => Ok(GameSessionState::InProgress),
        "completed" => Ok(GameSessionState::Completed),
        "timed_out" => Ok(GameSessionState::TimedOut),
        "cancelled" => Ok(GameSessionState::Cancelled),
        _ => Err(AppError::Repository(format!("unknown session state {s}"))),
    }
}

pub fn step_state_to_db(s: StepState) -> &'static str {
    match s {
        StepState::Pending => "pending",
        StepState::Active => "active",
        StepState::Answered => "answered",
        StepState::Evaluated => "evaluated",
        StepState::TimedOut => "timed_out",
        StepState::Skipped => "skipped",
    }
}

pub fn step_state_from_db(s: &str) -> Result<StepState, AppError> {
    match s {
        "pending" => Ok(StepState::Pending),
        "active" => Ok(StepState::Active),
        "answered" => Ok(StepState::Answered),
        "evaluated" => Ok(StepState::Evaluated),
        "timed_out" => Ok(StepState::TimedOut),
        "skipped" => Ok(StepState::Skipped),
        _ => Err(AppError::Repository(format!("unknown step state {s}"))),
    }
}

pub fn step_from_json(
    id: Uuid,
    ordinal: i32,
    state: &str,
    user_facing_step_prompt: serde_json::Value,
    expected: serde_json::Value,
    user_answer: Option<serde_json::Value>,
    evaluation: Option<serde_json::Value>,
    deadline_at: Option<time::OffsetDateTime>,
) -> Result<GameStep, AppError> {
    Ok(GameStep {
        id: shakti_game_domain::GameStepId(id),
        ordinal: ordinal as usize,
        user_facing_step_prompt: serde_json::from_value(user_facing_step_prompt)
            .map_err(|e| AppError::Repository(e.to_string()))?,
        expected_answer: serde_json::from_value(expected)
            .map_err(|e| AppError::Repository(e.to_string()))?,
        user_answer: user_answer
            .map(|v| serde_json::from_value(v))
            .transpose()
            .map_err(|e| AppError::Repository(e.to_string()))?,
        evaluation: evaluation
            .map(|v| serde_json::from_value(v))
            .transpose()
            .map_err(|e| AppError::Repository(e.to_string()))?,
        deadline_at,
        state: step_state_from_db(state)?,
    })
}
