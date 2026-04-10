//! Async coin debit + `_billing` patch on `game_sessions.base_context`.

use crate::game_billing::client::{ActorsGameBillingClient, DeductReqBody};
use shakti_game_domain::GameSessionId;
use shakti_game_engine_core::billing::{read_wallet_from_base, write_wallet_to_base};
use shakti_game_engine_core::ports::{BillingChargeScheduler, GameLlmChargeArgs, GameSessionRepository};
use std::sync::Arc;
use uuid::Uuid;

pub struct PgBillingChargeScheduler {
    client: ActorsGameBillingClient,
    sessions: Arc<dyn GameSessionRepository>,
}

impl PgBillingChargeScheduler {
    pub fn new(client: ActorsGameBillingClient, sessions: Arc<dyn GameSessionRepository>) -> Self {
        Self { client, sessions }
    }
}

async fn apply_wallet_flags(
    sessions: &Arc<dyn GameSessionRepository>,
    session_id: GameSessionId,
    llm_spend_suspended: bool,
    cached_balance: Option<i64>,
) {
    let Ok(mut session) = sessions.get(session_id).await else {
        tracing::warn!(session_id = %session_id.0, "billing patch: session not found");
        return;
    };
    let Some(mut wallet) = read_wallet_from_base(&session.base_context) else {
        tracing::warn!(session_id = %session_id.0, "billing patch: no _billing in base_context");
        return;
    };
    wallet.llm_spend_suspended = llm_spend_suspended;
    if let Some(b) = cached_balance {
        wallet.cached_balance = Some(b);
    }
    if let Err(e) = write_wallet_to_base(&mut session.base_context, &wallet) {
        tracing::error!(session_id = %session_id.0, error = %e, "billing patch: write_wallet_to_base");
        return;
    }
    if let Err(e) = sessions.update(&session).await {
        tracing::error!(session_id = %session_id.0, error = %e, "billing patch: session update");
    }
}

impl BillingChargeScheduler for PgBillingChargeScheduler {
    fn schedule_game_llm_charge(&self, args: GameLlmChargeArgs) {
        let client = self.client.clone();
        let sessions = self.sessions.clone();
        tokio::spawn(async move {
            tracing::info!(
                user_id = %args.shakti_user_id,
                trace_id = %args.trace_id,
                session_id = %args.session_id.0,
                coins = args.coins,
                endpoint = args.endpoint,
                "game_llm billing deduct (async)"
            );
            let request_id = Uuid::new_v4().to_string();
            let body = DeductReqBody {
                shakti_user_id: args.shakti_user_id,
                points: args.coins,
                request_id,
                api_name: "openai".to_string(),
                endpoint: args.endpoint.to_string(),
                variant: args.variant,
                trace_id: args.trace_id.clone(),
            };
            match client.deduct(body).await {
                Ok(resp) => {
                    if !resp.success {
                        tracing::warn!(
                            user_id = %args.shakti_user_id,
                            trace_id = %args.trace_id,
                            session_id = %args.session_id.0,
                            error = ?resp.error,
                            "game billing deduct declined"
                        );
                        apply_wallet_flags(&sessions, args.session_id, true, resp.balance_after)
                            .await;
                    } else {
                        let bal = resp.balance_after;
                        let suspend = bal.map(|b| b <= 0).unwrap_or(false);
                        apply_wallet_flags(&sessions, args.session_id, suspend, bal).await;
                    }
                }
                Err(e) => {
                    tracing::error!(
                        user_id = %args.shakti_user_id,
                        trace_id = %args.trace_id,
                        session_id = %args.session_id.0,
                        error = %e,
                        "game billing deduct transport error"
                    );
                    apply_wallet_flags(&sessions, args.session_id, true, None).await;
                }
            }
        });
    }
}
