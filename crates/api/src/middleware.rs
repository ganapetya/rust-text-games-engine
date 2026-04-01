use axum::{body::Body, extract::Request, middleware::Next, response::Response};
use tracing::Instrument;
use uuid::Uuid;

#[derive(Clone)]
pub struct RequestTrace {
    pub trace_id: String,
}

pub async fn request_trace_middleware(mut req: Request<Body>, next: Next) -> Response {
    let trace_id = req
        .headers()
        .get("x-trace-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    req.extensions_mut().insert(RequestTrace {
        trace_id: trace_id.clone(),
    });

    let span = tracing::info_span!(
        "http_request",
        trace_id = %trace_id,
    );

    async move { next.run(req).await }.instrument(span).await
}
