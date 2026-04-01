use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use shakti_game_application::AppError;

#[derive(Serialize)]
pub struct ErrorBody {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
}

pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
    pub trace_id: Option<String>,
}

impl ApiError {
    pub fn from_app_err(e: AppError, trace_id: Option<String>) -> Self {
        let (status, message) = match &e {
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, e.to_string()),
            AppError::Forbidden => (StatusCode::FORBIDDEN, e.to_string()),
            AppError::Conflict(_) => (StatusCode::CONFLICT, e.to_string()),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, e.to_string()),
            AppError::Domain(de) => (StatusCode::BAD_REQUEST, de.to_string()),
            AppError::Repository(_) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };
        ApiError {
            status,
            message,
            trace_id,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            error: self.message,
            trace_id: self.trace_id,
        };
        (self.status, Json(body)).into_response()
    }
}
