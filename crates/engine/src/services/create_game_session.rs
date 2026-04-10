//! Opens a **Draft** session: stores deferred content options; LLM and steps run in [`super::start_session::start_game_session`].

use crate::deps::EngineDeps;
use crate::errors::AppError;
use crate::ports::{ContentRequest, SessionBillingBootstrap};
use serde::{Deserialize, Serialize};
use shakti_game_domain::{GameKind, GameSession, GameSessionId, UserId};
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionOptions {
    /// Overrides per-step time limit from the loaded [`GameDefinition`] when set.
    #[serde(default)]
    pub step_time_limit_secs: Option<u32>,
    /// Locale codes (e.g. BCP-47) allowed for optional full-text translation hints; normalized on materialize (max 16).
    #[serde(default)]
    pub hint_translation_languages: Option<Vec<String>>,
}

pub struct CreateGameSessionCommand {
    pub user_id: UserId,
    pub trace_id: Option<String>,
    pub game_kind: GameKind,
    pub definition_id: Option<shakti_game_domain::GameDefinitionId>,
    pub content_request: ContentRequest,
    pub options: SessionOptions,
    /// Original `contentPackage` from bootstrap (audit); omitted for public create.
    pub content_package_audit: Option<serde_json::Value>,
    pub billing: SessionBillingBootstrap,
}

/// Persists `Draft` + deferred payload; no LLM or steps yet.
pub async fn create_game_session(
    deps: &EngineDeps,
    cmd: CreateGameSessionCommand,
) -> Result<GameSession, AppError> {
    tracing::info!(
        user_id = %cmd.user_id.0,
        trace_id = cmd.trace_id.as_deref().unwrap_or(""),
        "create_game_session (draft)"
    );
    let mut definition = match (cmd.game_kind, cmd.definition_id) {
        (_, Some(id)) => deps.definitions.get(id).await?,
        (GameKind::GapFill, None) => deps.definitions.get_default_gap_fill().await?,
        (GameKind::CorrectUsage, None) => deps.definitions.get_default_correct_usage().await?,
    };

    if definition.kind != cmd.game_kind {
        return Err(AppError::BadRequest(format!(
            "definition kind {:?} does not match requested {:?}",
            definition.kind, cmd.game_kind
        )));
    }

    if let Some(secs) = cmd.options.step_time_limit_secs {
        definition.timing_policy.per_step_limit_secs = Some(secs); // per-request override; not persisted to definition row
    }

    let mut deferred = serde_json::json!({
        "content_request": cmd.content_request,
        "session_options": cmd.options,
        "trace_id": cmd.trace_id,
    });
    if let Some(pkg) = &cmd.content_package_audit {
        if let Some(obj) = deferred.as_object_mut() {
            obj.insert("content_package".to_string(), pkg.clone());
        }
    }
    if cmd.billing.shakti_user_id.is_some() || cmd.billing.billing_rates.is_some() {
        if let Some(obj) = deferred.as_object_mut() {
            if let Some(uid) = cmd.billing.shakti_user_id {
                obj.insert("shaktiUserId".to_string(), serde_json::json!(uid));
            }
            if let Some(ref rates) = cmd.billing.billing_rates {
                obj.insert(
                    "billingRates".to_string(),
                    serde_json::to_value(rates).map_err(|e| {
                        AppError::Repository(format!("billingRates json: {e}"))
                    })?,
                );
            }
        }
    }

    let session = GameSession::new_draft(
        GameSessionId(Uuid::new_v4()),
        cmd.user_id,
        definition.id,
        definition,
        deferred,
    );

    deps.sessions.insert(&session).await?; // INSERT draft row (+ no steps)

    deps.events
        .append(
            session.id,
            "session_created",
            serde_json::json!({
                "state": shakti_game_domain::GameSessionState::Draft,
                "steps_count": 0,
            }),
        )
        .await?;

    Ok(session)
}
