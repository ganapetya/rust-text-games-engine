//! HTTP client for shakti-actors internal game billing routes.

use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct ActorsGameBillingClient {
    inner: Arc<Inner>,
}

struct Inner {
    http: reqwest::Client,
    deduct_url: Url,
    balance_url: Url,
    api_key: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BalanceReqBody {
    shakti_user_id: i64,
    trace_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceResp {
    pub success: bool,
    #[serde(default)]
    pub balance: Option<i64>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeductReqBody {
    pub shakti_user_id: i64,
    pub points: i64,
    pub request_id: String,
    pub api_name: String,
    pub endpoint: String,
    pub variant: String,
    pub trace_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeductResp {
    pub success: bool,
    #[serde(default)]
    pub balance_after: Option<i64>,
    #[serde(default)]
    pub error: Option<String>,
}

impl ActorsGameBillingClient {
    pub fn new(actors_base: &str, api_key: &str) -> Result<Self, String> {
        let base = actors_base.trim_end_matches('/');
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("billing reqwest client: {e}"))?;
        let deduct_url = Url::parse(&format!("{base}/api/internal/game-billing/deduct"))
            .map_err(|e| format!("invalid deduct URL: {e}"))?;
        let balance_url = Url::parse(&format!("{base}/api/internal/game-billing/balance"))
            .map_err(|e| format!("invalid balance URL: {e}"))?;
        Ok(Self {
            inner: Arc::new(Inner {
                http,
                deduct_url,
                balance_url,
                api_key: api_key.to_string(),
            }),
        })
    }

    pub async fn fetch_balance(&self, shakti_user_id: i64, trace_id: &str) -> Result<i64, String> {
        let body = BalanceReqBody {
            shakti_user_id,
            trace_id: trace_id.to_string(),
        };
        let res = self
            .inner
            .http
            .post(self.inner.balance_url.clone())
            .header("Authorization", format!("Bearer {}", self.inner.api_key))
            .header("X-Trace-Id", trace_id)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("balance request: {e}"))?;
        let status = res.status();
        let text = res.text().await.map_err(|e| format!("balance body: {e}"))?;
        let parsed: BalanceResp = serde_json::from_str(&text)
            .map_err(|e| format!("balance json {status}: {e}; body={text:?}"))?;
        if parsed.success {
            parsed
                .balance
                .ok_or_else(|| "balance response missing balance".to_string())
        } else {
            Err(parsed
                .error
                .unwrap_or_else(|| "balance request failed".to_string()))
        }
    }

    pub async fn deduct(&self, body: DeductReqBody) -> Result<DeductResp, String> {
        let trace = body.trace_id.clone();
        let res = self
            .inner
            .http
            .post(self.inner.deduct_url.clone())
            .header("Authorization", format!("Bearer {}", self.inner.api_key))
            .header("X-Trace-Id", &trace)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("deduct request: {e}"))?;
        let status = res.status();
        let text = res.text().await.map_err(|e| format!("deduct body: {e}"))?;
        serde_json::from_str(&text).map_err(|e| format!("deduct json {status}: {e}; body={text:?}"))
    }
}
