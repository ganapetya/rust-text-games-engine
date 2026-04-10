//! LogosCat wallet state stored under `base_context._billing` (alongside `_session`).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use shakti_game_pricing::GameBillingRates;

pub const BILLING_CONTEXT_KEY: &str = "_billing";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionWalletState {
    pub shakti_user_id: i64,
    pub billing_rates: GameBillingRates,
    #[serde(default)]
    pub llm_spend_suspended: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_balance: Option<i64>,
}

pub fn wallet_from_deferred(deferred: &Value) -> Option<SessionWalletState> {
    let uid = deferred.get("shaktiUserId").and_then(|v| v.as_i64())?;
    let rates: GameBillingRates = deferred
        .get("billingRates")
        .and_then(|v| serde_json::from_value(v.clone()).ok())?;
    Some(SessionWalletState {
        shakti_user_id: uid,
        billing_rates: rates,
        llm_spend_suspended: false,
        cached_balance: None,
    })
}

pub fn read_wallet_from_base(base: &Value) -> Option<SessionWalletState> {
    base.get(BILLING_CONTEXT_KEY)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

pub fn write_wallet_to_base(base: &mut Value, wallet: &SessionWalletState) -> Result<(), String> {
    let obj = base
        .as_object_mut()
        .ok_or_else(|| "base_context must be a JSON object".to_string())?;
    let v = serde_json::to_value(wallet).map_err(|e| e.to_string())?;
    obj.insert(BILLING_CONTEXT_KEY.to_string(), v);
    Ok(())
}

pub fn wallet_llm_blocked(wallet: &SessionWalletState) -> bool {
    wallet.llm_spend_suspended
}
