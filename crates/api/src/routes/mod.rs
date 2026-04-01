mod health;
mod sessions;

use crate::middleware::request_trace_middleware;
use crate::state::AppState;
use axum::{middleware, Router};
use std::sync::Arc;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .merge(health::routes())
        .nest("/api/v1", sessions::routes())
        .layer(middleware::from_fn(request_trace_middleware))
        .with_state(Arc::new(state))
}
