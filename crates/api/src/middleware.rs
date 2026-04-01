use axum::{
    body::Body,
    extract::Request,
    http::{HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};
use tracing::Instrument;
use uuid::Uuid;

static X_TRACE_ID: HeaderName = HeaderName::from_static("x-trace-id");

#[derive(Clone)]
pub struct RequestTrace {
    pub trace_id: String,
}

pub async fn request_trace_middleware(mut req: Request<Body>, next: Next) -> Response {
    let trace_id = req
        .headers()
        .get(&X_TRACE_ID)
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

    let mut response = async move { next.run(req).await }.instrument(span).await;

    let header_val = HeaderValue::from_str(&trace_id).unwrap_or_else(|_| {
        HeaderValue::from_str(&Uuid::new_v4().to_string()).expect("uuid header value")
    });
    response
        .headers_mut()
        .insert(X_TRACE_ID.clone(), header_val);

    response
}
