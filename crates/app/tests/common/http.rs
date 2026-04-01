//! Axum router `oneshot` helpers — use from any `tests/*.rs` scenario module via `crate::common::http`.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tower::ServiceExt;

const TRACE_HEADER: &str = "test-trace-integration";

pub async fn json_roundtrip(
    app: Router,
    method: &str,
    path_and_query: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(path_and_query)
        .header("content-type", "application/json")
        .header("x-trace-id", TRACE_HEADER);

    let req = if let Some(b) = body {
        builder = builder.header("content-length", b.to_string().len());
        builder.body(Body::from(b.to_string())).expect("request")
    } else {
        builder.body(Body::empty()).expect("request")
    };

    let response = app.oneshot(req).await.expect("response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let v: Value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| panic!("invalid JSON: {}", String::from_utf8_lossy(&bytes)))
    };
    (status, v)
}

pub fn assert_status(status: StatusCode, expected: StatusCode, body: &Value) {
    assert_eq!(
        status,
        expected,
        "unexpected status; body: {}",
        serde_json::to_string_pretty(body).unwrap_or_default()
    );
}

pub fn parse_json<T: DeserializeOwned>(body: &Value) -> T {
    serde_json::from_value(body.clone()).expect("deserialize")
}
