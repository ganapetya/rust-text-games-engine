use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use shakti_game_application::{
    advance_session, create_game_session, get_game_result, get_game_session, start_game_session,
    submit_answer, ContentRequest, CreateGameSessionCommand, SessionOptions, SubmitAnswerCommand,
};
use shakti_game_domain::{
    GameKind, GameSession, GameSessionId, GameSessionState, GameStep, StepPrompt, StepState,
    UserAnswer, UserId,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::error::ApiError;
use crate::middleware::RequestTrace;
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/game-sessions", post(create_session))
        .route("/game-sessions/{session_id}/start", post(start_session))
        .route("/game-sessions/{session_id}", get(get_session))
        .route(
            "/game-sessions/{session_id}/steps/{step_id}/answer",
            post(submit_step_answer),
        )
        .route("/game-sessions/{session_id}/advance", post(advance))
        .route("/game-sessions/{session_id}/result", get(result))
}

fn effective_trace(trace: &RequestTrace, body: Option<&str>) -> String {
    body.filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| trace.trace_id.clone())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionReq {
    pub user_id: Uuid,
    pub game_kind: GameKind,
    pub definition_id: Option<Uuid>,
    #[serde(default)]
    pub content_request: ContentReqDto,
    #[serde(default)]
    pub options: SessionOptionsDto,
    #[serde(default)]
    pub trace_id: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContentReqDto {
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default = "default_limit")]
    pub limit: i64,
    pub language: Option<String>,
}

fn default_source() -> String {
    "hard_words".into()
}

fn default_limit() -> i64 {
    10
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionOptionsDto {
    pub step_time_limit_secs: Option<u32>,
    #[serde(default)]
    pub llm_preparation_enabled: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionResp {
    pub session_id: Uuid,
    pub state: GameSessionState,
    pub current_step_index: usize,
    pub steps_count: usize,
    pub trace_id: String,
}

async fn create_session(
    State(state): State<Arc<AppState>>,
    Extension(trace): Extension<RequestTrace>,
    Json(body): Json<CreateSessionReq>,
) -> Result<Json<CreateSessionResp>, ApiError> {
    let trace_id = effective_trace(&trace, body.trace_id.as_deref());
    let cmd = CreateGameSessionCommand {
        user_id: UserId(body.user_id),
        game_kind: body.game_kind,
        definition_id: body.definition_id.map(shakti_game_domain::GameDefinitionId),
        content_request: ContentRequest {
            source: body.content_request.source,
            limit: body.content_request.limit,
            language: body.content_request.language,
        },
        options: SessionOptions {
            step_time_limit_secs: body.options.step_time_limit_secs,
            llm_preparation_enabled: body.options.llm_preparation_enabled,
        },
    };
    let session = create_game_session(&state.deps, cmd)
        .await
        .map_err(|e| ApiError::from_app_err(e, Some(trace_id.clone())))?;
    Ok(Json(CreateSessionResp {
        session_id: session.id.0,
        state: session.state,
        current_step_index: session.current_step_index,
        steps_count: session.steps.len(),
        trace_id,
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserActionBody {
    pub user_id: Uuid,
    #[serde(default)]
    pub trace_id: Option<String>,
}

async fn start_session(
    State(state): State<Arc<AppState>>,
    Extension(trace): Extension<RequestTrace>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<UserActionBody>,
) -> Result<Json<SessionPublicView>, ApiError> {
    let trace_id = effective_trace(&trace, body.trace_id.as_deref());
    let sid = GameSessionId(session_id);
    tracing::info!(user_id = %body.user_id, "start_game_session");
    let session = start_game_session(&state.deps, sid, UserId(body.user_id))
        .await
        .map_err(|e| ApiError::from_app_err(e, Some(trace_id)))?;
    Ok(Json(to_public_view(&session)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSessionQuery {
    pub user_id: Uuid,
    #[serde(default)]
    pub trace_id: Option<String>,
}

async fn get_session(
    State(state): State<Arc<AppState>>,
    Extension(trace): Extension<RequestTrace>,
    Path(session_id): Path<Uuid>,
    Query(q): Query<GetSessionQuery>,
) -> Result<Json<SessionPublicView>, ApiError> {
    let trace_id = effective_trace(&trace, q.trace_id.as_deref());
    tracing::info!(user_id = %q.user_id, "get_game_session");
    let session = get_game_session(&state.deps, GameSessionId(session_id), UserId(q.user_id))
        .await
        .map_err(|e| ApiError::from_app_err(e, Some(trace_id)))?;
    Ok(Json(to_public_view(&session)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnswerReq {
    pub user_id: Uuid,
    pub answer: UserAnswer,
    #[serde(default)]
    pub trace_id: Option<String>,
}

async fn submit_step_answer(
    State(state): State<Arc<AppState>>,
    Extension(trace): Extension<RequestTrace>,
    Path((session_id, step_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<AnswerReq>,
) -> Result<Json<AnswerResp>, ApiError> {
    let trace_id = effective_trace(&trace, body.trace_id.as_deref());
    tracing::info!(user_id = %body.user_id, "submit_answer");
    let cmd = SubmitAnswerCommand {
        session_id: GameSessionId(session_id),
        step_id: shakti_game_domain::GameStepId(step_id),
        user_id: UserId(body.user_id),
        answer: body.answer,
    };
    let session = submit_answer(&state.deps, cmd)
        .await
        .map_err(|e| ApiError::from_app_err(e, Some(trace_id.clone())))?;
    let current = session.current_step();
    let eval = current.and_then(|s| s.evaluation.clone());
    Ok(Json(AnswerResp {
        step_id,
        correct: eval.as_ref().map(|e| e.is_correct).unwrap_or(false),
        awarded_points: eval.as_ref().map(|e| e.awarded_points).unwrap_or(0),
        next_state: current.map(|s| s.state).unwrap_or(StepState::Pending),
        current_score: ScoreDto::from(&session.score),
        session_state: session.state,
        trace_id,
    }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnswerResp {
    pub step_id: Uuid,
    pub correct: bool,
    pub awarded_points: i32,
    pub next_state: StepState,
    pub current_score: ScoreDto,
    pub session_state: GameSessionState,
    pub trace_id: String,
}

async fn advance(
    State(state): State<Arc<AppState>>,
    Extension(trace): Extension<RequestTrace>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<UserActionBody>,
) -> Result<Json<SessionPublicView>, ApiError> {
    let trace_id = effective_trace(&trace, body.trace_id.as_deref());
    let session = advance_session(&state.deps, GameSessionId(session_id), UserId(body.user_id))
        .await
        .map_err(|e| ApiError::from_app_err(e, Some(trace_id)))?;
    Ok(Json(to_public_view(&session)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultQuery {
    pub user_id: Uuid,
    #[serde(default)]
    pub trace_id: Option<String>,
}

async fn result(
    State(state): State<Arc<AppState>>,
    Extension(trace): Extension<RequestTrace>,
    Path(session_id): Path<Uuid>,
    Query(q): Query<ResultQuery>,
) -> Result<Json<shakti_game_domain::GameResult>, ApiError> {
    let trace_id = effective_trace(&trace, q.trace_id.as_deref());
    let res = get_game_result(&state.deps, GameSessionId(session_id), UserId(q.user_id))
        .await
        .map_err(|e| ApiError::from_app_err(e, Some(trace_id)))?;
    Ok(Json(res))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPublicView {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub state: GameSessionState,
    pub current_step_index: usize,
    pub steps_count: usize,
    pub score: ScoreDto,
    pub current_step: Option<StepPublic>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepPublic {
    pub id: Uuid,
    pub ordinal: usize,
    pub state: StepState,
    pub prompt: StepPrompt,
    pub user_answer: Option<UserAnswer>,
    pub evaluation: Option<shakti_game_domain::StepEvaluation>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreDto {
    pub total_points: i32,
    pub earned_points: i32,
    pub accuracy: f32,
}

impl From<&shakti_game_domain::Score> for ScoreDto {
    fn from(s: &shakti_game_domain::Score) -> Self {
        ScoreDto {
            total_points: s.total_points,
            earned_points: s.earned_points,
            accuracy: s.accuracy(),
        }
    }
}

fn to_public_view(session: &GameSession) -> SessionPublicView {
    let current_step = session.current_step().map(|s| step_public(s));
    SessionPublicView {
        session_id: session.id.0,
        user_id: session.user_id.0,
        state: session.state,
        current_step_index: session.current_step_index,
        steps_count: session.steps.len(),
        score: ScoreDto::from(&session.score),
        current_step,
    }
}

fn step_public(step: &GameStep) -> StepPublic {
    StepPublic {
        id: step.id.0,
        ordinal: step.ordinal,
        state: step.state,
        prompt: step.prompt.clone(),
        user_answer: step.user_answer.clone(),
        evaluation: step.evaluation.clone(),
    }
}
