//! Create a new game session: load definition → fetch content → optional LLM enrichment →
//! domain `prepare_content` / `generate_steps` → persist session and emit `session_created`.
//!
//! LLM runs in the **application** layer ([`crate::ports::LlmContentPreparer`]), not inside
//! [`shakti_game_domain::GameEngine`] (which stays synchronous and rule-based).

use crate::deps::EngineDeps;
use crate::errors::AppError;
use crate::ports::ContentRequest;
use serde::{Deserialize, Serialize};
use shakti_game_domain::{GameKind, GameSession, GameSessionId, GameSessionState, UserId};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionOptions {
    /// Overrides per-step time limit from the loaded [`GameDefinition`] when set.
    #[serde(default)]
    pub step_time_limit_secs: Option<u32>,
    /// When true, runs async LLM preparation on fetched items before [`shakti_game_domain::GameEngine::prepare_content`].
    #[serde(default)]
    pub llm_preparation_enabled: bool,
}

pub struct CreateGameSessionCommand {
    pub user_id: UserId,
    /// Correlates with `x-trace-id` when the session is created over HTTP.
    pub trace_id: Option<String>,
    pub game_kind: GameKind,
    pub definition_id: Option<shakti_game_domain::GameDefinitionId>,
    pub content_request: ContentRequest,
    pub options: SessionOptions,
}

/// Orchestrates session creation: I/O and optional LLM here; pure game logic in [`shakti_game_domain::GameEngine`].
pub async fn create_game_session(
    deps: &EngineDeps,
    cmd: CreateGameSessionCommand,
) -> Result<GameSession, AppError> {
    // --- Game kind gate (v0.1 only implements gap_fill end-to-end) ---
    if cmd.game_kind != GameKind::GapFill {
        return Err(AppError::BadRequest("only gap_fill supported".into()));
    }

    // --- Load game definition (rules, steps_count, distractors, scoring, timing template) ---
    // Either by explicit id from the client, or the seeded default gap_fill definition.
    let mut definition = if let Some(id) = cmd.definition_id {
        deps.definitions.get(id).await?
    } else {
        deps.definitions.get_default_gap_fill().await?
    };

    // --- Optional per-request timing override (does not change DB definition row) ---
    if let Some(secs) = cmd.options.step_time_limit_secs {
        definition.timing_policy.per_step_limit_secs = Some(secs);
    }

    // --- Resolve the domain engine for this game kind (e.g. GapFillEngine) ---
    // Used only for sync steps: prepare_content, generate_steps, later evaluate/finalize.
    let engine = deps.engines.get(definition.kind)?;

    // --- Fetch candidate learning items from the content provider (e.g. Postgres) ---
    // `content_request` selects source/limit/language; returns rows owned by this user.
    let items = deps
        .content
        .fetch_learning_items(cmd.user_id, cmd.content_request.clone())
        .await?;

    // --- Ensure we have enough raw items to build `steps_count` steps (before/after LLM) ---
    let need = definition.config.steps_count;
    if items.len() < need {
        return Err(AppError::BadRequest(format!(
            "not enough learning items: need {}, have {}",
            need,
            items.len()
        )));
    }

    // --- Optional async LLM: rewrite/enrich items (LLM messages + JSON) before domain preparation ---
    // When disabled, we keep the DB-backed `items` as-is. When enabled, `LlmContentPreparer`
    // returns a new `Vec<LearningItem>` that must still cover at least `need` entries.
    let items = if cmd.options.llm_preparation_enabled {
        tracing::info!(
            user_id = %cmd.user_id.0,
            ?cmd.trace_id,
            llm_preparation = true,
            "running LLM preparation before gap_fill prepare_content"
        );
        let out = deps
            .llm_preparer
            .prepare_gap_fill_learning_items(
                cmd.user_id,
                cmd.trace_id.as_deref(),
                &items,
                &definition.config,
            )
            .await?;
        if out.len() < need {
            return Err(AppError::BadRequest(format!(
                "LLM returned too few learning items: need {}, have {}",
                need,
                out.len()
            )));
        }
        out
    } else {
        items
    };

    // --- Domain: wrap items as PreparedContent (e.g. JSON payloads) then build GameSteps ---
    // Synchronous, deterministic gap-fill logic (gaps, distractors, ordinals).
    let prepared = engine.prepare_content(&items, &definition.config)?;
    let steps = engine.generate_steps(&prepared, &definition.config)?;
    if steps.is_empty() {
        return Err(AppError::BadRequest("no steps generated".into()));
    }

    // --- Aggregate: new session id, user, definition ref, materialized steps + policies ---
    let session = GameSession::new(
        GameSessionId(Uuid::new_v4()),
        cmd.user_id,
        definition.id,
        steps,
        definition,
    );

    // --- Persist session (steps, state, etc.) and append an audit/event row for observability ---
    deps.sessions.insert(&session).await?;
    deps.events
        .append(
            session.id,
            "session_created",
            serde_json::json!({
                "state": GameSessionState::Prepared,
                "steps_count": session.steps.len(),
                "llm_preparation_enabled": cmd.options.llm_preparation_enabled,
            }),
        )
        .await?;

    Ok(session)
}
