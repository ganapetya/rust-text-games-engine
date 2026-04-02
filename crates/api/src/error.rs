use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use shakti_game_engine_core::AppError;

#[derive(Serialize)]
pub struct ErrorBody {
    pub error: String,
}

pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl ApiError {
    pub fn from_app_err(e: AppError) -> Self {
        let (status, message) = match &e {
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, e.to_string()),
            AppError::Forbidden => (StatusCode::FORBIDDEN, e.to_string()),
            AppError::Conflict(_) => (StatusCode::CONFLICT, e.to_string()),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, e.to_string()),
            AppError::Domain(de) => (StatusCode::BAD_REQUEST, de.to_string()),
            AppError::Repository(_) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::LlmPreparation(_) => (StatusCode::BAD_GATEWAY, e.to_string()),
        };
        ApiError { status, message }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            error: self.message,
        };
        (self.status, Json(body)).into_response()
    }
}
