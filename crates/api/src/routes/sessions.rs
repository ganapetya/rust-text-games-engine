use axum::{
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use shakti_game_domain::{
    GameKind, GameSession, GameSessionId, GameSessionState, GameStep, StepState, UserAnswer,
    UserFacingStepPrompt, UserId,
};
use shakti_game_engine_core::{
    advance_session, create_game_session, get_game_result, get_game_session, play_again_gap_fill,
    read_session_ui_hints, read_wallet_from_base, request_translation_hint, start_game_session,
    submit_answer, ContentRequest, CreateGameSessionCommand, SessionBillingBootstrap, SessionOptions,
    SubmitAnswerCommand,
};
use shakti_game_pricing::GameBillingRates;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::error::ApiError;
use crate::middleware::RequestTrace;
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/game-sessions/bootstrap", post(bootstrap_session))
        .route("/game-sessions", post(create_session))
        .route("/game-sessions/{session_id}/start", post(start_session))
        .route("/game-sessions/{session_id}/play-again", post(play_again_session))
        .route(
            "/game-sessions/{session_id}/hint/translation",
            post(translation_hint),
        )
        .route("/game-sessions/{session_id}", get(get_session))
        .route(
            "/game-sessions/{session_id}/steps/{step_id}/answer",
            post(submit_step_answer),
        )
        .route("/game-sessions/{session_id}/advance", post(advance))
        .route("/game-sessions/{session_id}/result", get(result))
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
    pub shakti_user_id: Option<i64>,
    #[serde(default)]
    pub billing_rates: Option<GameBillingRates>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContentReqDto {
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default = "default_limit")]
    pub limit: i64,
    pub language: Option<String>,
    #[serde(default)]
    pub llm_source_texts: Option<Vec<String>>,
    #[serde(default)]
    pub llm_hard_words: Option<Vec<String>>,
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
    pub hint_translation_languages: Option<Vec<String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionResp {
    pub session_id: Uuid,
    pub state: GameSessionState,
    pub current_step_index: usize,
    pub steps_count: usize,
}

/// Server-to-server: creates a draft session from a JSON `contentPackage` (stored in DB for audit).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapSessionReq {
    pub user_id: Uuid,
    pub language: String,
    pub game_kind: GameKind,
    pub definition_id: Option<Uuid>,
    pub content_package: serde_json::Value,
    #[serde(default)]
    pub options: SessionOptionsDto,
    pub trace_id: Option<String>,
    #[serde(default)]
    pub shakti_user_id: Option<i64>,
    #[serde(default)]
    pub billing_rates: Option<GameBillingRates>,
}

fn extract_bootstrap_service_token(headers: &HeaderMap) -> Option<String> {
    if let Some(raw) = headers
        .get("x-shakti-game-service-key")
        .and_then(|v| v.to_str().ok())
    {
        let t = raw.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    if let Some(raw) = headers.get(AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        let h = raw.trim();
        const PREFIX: &str = "Bearer ";
        if h.len() > PREFIX.len() && h[..PREFIX.len()].eq_ignore_ascii_case(PREFIX) {
            let t = h[PREFIX.len()..].trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

fn constant_time_eq_str(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

fn verify_bootstrap_service_key(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let Some(expected) = state.service_api_key.as_ref() else {
        return Err(ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: "game session bootstrap is not configured (set GAME_ENGINE_SERVICE_API_KEY)"
                .into(),
        });
    };
    let Some(got) = extract_bootstrap_service_token(headers) else {
        return Err(ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: "missing Authorization: Bearer or X-Shakti-Game-Service-Key".into(),
        });
    };
    if !constant_time_eq_str(expected.as_ref(), &got) {
        return Err(ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: "invalid service key".into(),
        });
    }
    Ok(())
}

async fn bootstrap_session(
    Extension(trace): Extension<RequestTrace>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<BootstrapSessionReq>,
) -> Result<Json<CreateSessionResp>, ApiError> {
    verify_bootstrap_service_key(&state, &headers)?;

    if !body.content_package.is_object() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "contentPackage must be a JSON object".into(),
        });
    }

    let mut dto: ContentReqDto = serde_json::from_value(body.content_package.clone()).map_err(|e| {
        ApiError {
            status: StatusCode::BAD_REQUEST,
            message: format!("contentPackage: {e}"),
        }
    })?;

    let lang = body.language.trim();
    if lang.is_empty() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "language must be non-empty".into(),
        });
    }
    dto.language = Some(lang.to_string());

    let trace_for_cmd = body
        .trace_id
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| Some(trace.trace_id.clone()));

    tracing::info!(
        user_id = %body.user_id,
        trace_id = %trace.trace_id,
        "bootstrap_game_session"
    );

    let cmd = CreateGameSessionCommand {
        user_id: UserId(body.user_id),
        trace_id: trace_for_cmd,
        game_kind: body.game_kind,
        definition_id: body.definition_id.map(shakti_game_domain::GameDefinitionId),
        content_request: ContentRequest {
            source: dto.source,
            limit: dto.limit,
            language: dto.language,
            llm_source_texts: dto.llm_source_texts,
            llm_hard_words: dto.llm_hard_words,
        },
        options: SessionOptions {
            step_time_limit_secs: body.options.step_time_limit_secs,
            hint_translation_languages: body.options.hint_translation_languages.clone(),
        },
        content_package_audit: Some(body.content_package),
        billing: SessionBillingBootstrap {
            shakti_user_id: body.shakti_user_id,
            billing_rates: body.billing_rates,
        },
    };
    let session = create_game_session(&state.deps, cmd)
        .await
        .map_err(ApiError::from_app_err)?;
    Ok(Json(CreateSessionResp {
        session_id: session.id.0,
        state: session.state,
        current_step_index: session.current_step_index,
        steps_count: session.steps.len(),
    }))
}

async fn create_session(
    Extension(trace): Extension<RequestTrace>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateSessionReq>,
) -> Result<Json<CreateSessionResp>, ApiError> {
    let cmd = CreateGameSessionCommand {
        user_id: UserId(body.user_id),
        trace_id: Some(trace.trace_id.clone()),
        game_kind: body.game_kind,
        definition_id: body.definition_id.map(shakti_game_domain::GameDefinitionId),
        content_request: ContentRequest {
            source: body.content_request.source,
            limit: body.content_request.limit,
            language: body.content_request.language,
            llm_source_texts: body.content_request.llm_source_texts,
            llm_hard_words: body.content_request.llm_hard_words,
        },
        options: SessionOptions {
            step_time_limit_secs: body.options.step_time_limit_secs,
            hint_translation_languages: body.options.hint_translation_languages.clone(),
        },
        content_package_audit: None,
        billing: SessionBillingBootstrap {
            shakti_user_id: body.shakti_user_id,
            billing_rates: body.billing_rates,
        },
    };
    let session = create_game_session(&state.deps, cmd)
        .await
        .map_err(ApiError::from_app_err)?;
    Ok(Json(CreateSessionResp {
        session_id: session.id.0,
        state: session.state,
        current_step_index: session.current_step_index,
        steps_count: session.steps.len(),
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserActionBody {
    pub user_id: Uuid,
}

async fn start_session(
    Extension(trace): Extension<RequestTrace>,
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<UserActionBody>,
) -> Result<Json<SessionPublicView>, ApiError> {
    let sid = GameSessionId(session_id);
    tracing::info!(
        user_id = %body.user_id,
        trace_id = %trace.trace_id,
        "start_game_session"
    );
    let session = start_game_session(&state.deps, sid, UserId(body.user_id))
        .await
        .map_err(ApiError::from_app_err)?;
    Ok(Json(
        session_public_view(&state, &session, trace.trace_id.as_str())
            .await,
    ))
}

async fn play_again_session(
    Extension(trace): Extension<RequestTrace>,
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<UserActionBody>,
) -> Result<Json<SessionPublicView>, ApiError> {
    let sid = GameSessionId(session_id);
    tracing::info!(
        user_id = %body.user_id,
        trace_id = %trace.trace_id,
        "play_again_gap_fill"
    );
    let session = play_again_gap_fill(&state.deps, sid, UserId(body.user_id))
        .await
        .map_err(ApiError::from_app_err)?;
    Ok(Json(
        session_public_view(&state, &session, trace.trace_id.as_str())
            .await,
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationHintBody {
    pub user_id: Uuid,
    pub target_language: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationHintResp {
    pub translated_text: String,
    pub source_language: String,
    pub target_language: String,
}

async fn translation_hint(
    Extension(trace): Extension<RequestTrace>,
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<TranslationHintBody>,
) -> Result<Json<TranslationHintResp>, ApiError> {
    tracing::info!(
        user_id = %body.user_id,
        trace_id = %trace.trace_id,
        "translation_hint"
    );
    let out = request_translation_hint(
        &state.deps,
        GameSessionId(session_id),
        UserId(body.user_id),
        &body.target_language,
        Some(trace.trace_id.as_str()),
    )
    .await
    .map_err(ApiError::from_app_err)?;
    Ok(Json(TranslationHintResp {
        translated_text: out.translated_text,
        source_language: out.source_language,
        target_language: out.target_language,
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSessionQuery {
    pub user_id: Uuid,
}

async fn get_session(
    Extension(trace): Extension<RequestTrace>,
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<Uuid>,
    Query(q): Query<GetSessionQuery>,
) -> Result<Json<SessionPublicView>, ApiError> {
    tracing::info!(user_id = %q.user_id, "get_game_session");
    let session = get_game_session(&state.deps, GameSessionId(session_id), UserId(q.user_id))
        .await
        .map_err(ApiError::from_app_err)?;
    Ok(Json(
        session_public_view(&state, &session, trace.trace_id.as_str())
            .await,
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnswerReq {
    pub user_id: Uuid,
    pub answer: UserAnswer,
}

async fn submit_step_answer(
    State(state): State<Arc<AppState>>,
    Path((session_id, step_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<AnswerReq>,
) -> Result<Json<AnswerResp>, ApiError> {
    tracing::info!(user_id = %body.user_id, "submit_answer");
    let cmd = SubmitAnswerCommand {
        session_id: GameSessionId(session_id),
        step_id: shakti_game_domain::GameStepId(step_id),
        user_id: UserId(body.user_id),
        answer: body.answer,
    };
    let session = submit_answer(&state.deps, cmd)
        .await
        .map_err(ApiError::from_app_err)?;
    let current = session.current_step();
    let eval = current.and_then(|s| s.evaluation.clone());
    Ok(Json(AnswerResp {
        step_id,
        correct: eval.as_ref().map(|e| e.is_correct).unwrap_or(false),
        awarded_points: eval.as_ref().map(|e| e.awarded_points).unwrap_or(0),
        next_state: current.map(|s| s.state).unwrap_or(StepState::Pending),
        current_score: ScoreDto::from(&session.score),
        session_state: session.state,
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
}

async fn advance(
    Extension(trace): Extension<RequestTrace>,
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<UserActionBody>,
) -> Result<Json<SessionPublicView>, ApiError> {
    let session = advance_session(&state.deps, GameSessionId(session_id), UserId(body.user_id))
        .await
        .map_err(ApiError::from_app_err)?;
    Ok(Json(
        session_public_view(&state, &session, trace.trace_id.as_str())
            .await,
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultQuery {
    pub user_id: Uuid,
}

async fn result(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<Uuid>,
    Query(q): Query<ResultQuery>,
) -> Result<Json<shakti_game_domain::GameResult>, ApiError> {
    let res = get_game_result(&state.deps, GameSessionId(session_id), UserId(q.user_id))
        .await
        .map_err(ApiError::from_app_err)?;
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
    /// Passage source language after materialize (from `_session`); UI translation hints.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_language: Option<String>,
    /// Allowed target locales for `POST .../hint/translation` (normalized at materialize).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hint_translation_languages: Vec<String>,
    /// When `GAME_ENGINE_DEV_EXPOSE_GAP_SOLUTION` is set: LLM request summary (sources, hard words, language).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev_llm_inputs: Option<serde_json::Value>,
    /// LogosCat coins (shakti-actors wallet) when session has `_billing`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet_balance: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_spend_suspended: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepPublic {
    pub id: Uuid,
    pub ordinal: usize,
    pub state: StepState,
    pub user_facing_step_prompt: UserFacingStepPrompt,
    pub user_answer: Option<UserAnswer>,
    pub evaluation: Option<shakti_game_domain::StepEvaluation>,
    /// Correct answer per gap (ordinal 0 … n−1) when `GAME_ENGINE_DEV_EXPOSE_GAP_SOLUTION` is set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev_gap_solution: Option<Vec<String>>,
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

fn dev_llm_inputs_from_base(
    base: &serde_json::Value,
    expose: bool,
) -> Option<serde_json::Value> {
    if !expose {
        return None;
    }
    base.get("_dev_llm_inputs").cloned()
}

fn sync_to_public_view(session: &GameSession, dev_expose_gap_solution: bool) -> SessionPublicView {
    let current_step = session
        .current_step()
        .map(|s| step_public(s, dev_expose_gap_solution));
    let (source_language, hint_translation_languages) =
        read_session_ui_hints(&session.base_context);
    SessionPublicView {
        session_id: session.id.0,
        user_id: session.user_id.0,
        state: session.state,
        current_step_index: session.current_step_index,
        steps_count: session.steps.len(),
        score: ScoreDto::from(&session.score),
        current_step,
        source_language,
        hint_translation_languages,
        dev_llm_inputs: dev_llm_inputs_from_base(&session.base_context, dev_expose_gap_solution),
        wallet_balance: None,
        llm_spend_suspended: None,
    }
}

const WALLET_BALANCE_CACHE_TTL: Duration = Duration::from_secs(10);

async fn session_public_view(
    state: &AppState,
    session: &GameSession,
    trace_id: &str,
) -> SessionPublicView {
    let mut v = sync_to_public_view(session, state.dev_expose_gap_solution);
    let Some(wallet) = read_wallet_from_base(&session.base_context) else {
        return v;
    };
    v.llm_spend_suspended = Some(wallet.llm_spend_suspended);
    let sid = session.id.0;
    let now = Instant::now();
    let from_cache = state.balance_cache.lock().ok().and_then(|c| {
        c.get(&sid).and_then(|(t, b)| {
            if now.duration_since(*t) < WALLET_BALANCE_CACHE_TTL {
                Some(*b)
            } else {
                None
            }
        })
    });
    if let Some(b) = from_cache {
        v.wallet_balance = Some(b);
        return v;
    }
    if let Some(ref client) = state.billing_client {
        match client
            .fetch_balance(wallet.shakti_user_id, trace_id)
            .await
        {
            Ok(b) => {
                if let Ok(mut c) = state.balance_cache.lock() {
                    c.insert(sid, (now, b));
                }
                v.wallet_balance = Some(b);
            }
            Err(_) => {
                v.wallet_balance = wallet.cached_balance;
            }
        }
    } else {
        v.wallet_balance = wallet.cached_balance;
    }
    v
}

fn dev_gap_solution(step: &GameStep, expose: bool) -> Option<Vec<String>> {
    if !expose {
        return None;
    }
    match &step.expected_answer {
        shakti_game_domain::ExpectedAnswer::GapFillSlots { values } => Some(values.clone()),
        _ => None,
    }
}

fn step_public(step: &GameStep, dev_expose_gap_solution: bool) -> StepPublic {
    StepPublic {
        id: step.id.0,
        ordinal: step.ordinal,
        state: step.state,
        user_facing_step_prompt: step.user_facing_step_prompt.clone(),
        user_answer: step.user_answer.clone(),
        evaluation: step.evaluation.clone(),
        dev_gap_solution: dev_gap_solution(step, dev_expose_gap_solution),
    }
}
