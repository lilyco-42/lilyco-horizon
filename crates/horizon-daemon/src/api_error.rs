use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use horizon_core::error::CoreError;
use serde::Serialize;

#[derive(Serialize)]
struct ErrorBody {
    code: String,
    message: String,
}

pub struct ApiError {
    status: StatusCode,
    code: String,
    message: String,
}

impl From<CoreError> for ApiError {
    fn from(err: CoreError) -> Self {
        let (status, code) = match &err {
            CoreError::Validation(_) => (StatusCode::BAD_REQUEST, "VALIDATION_ERROR"),
            CoreError::Io { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "IO_ERROR"),
            CoreError::Toml(_) => (StatusCode::INTERNAL_SERVER_ERROR, "TOML_ERROR"),
            CoreError::Json(_) => (StatusCode::BAD_REQUEST, "JSON_ERROR"),
        };
        Self {
            status,
            code: code.to_string(),
            message: err.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            code: self.code,
            message: self.message,
        };
        (self.status, Json(body)).into_response()
    }
}
