use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("服務不可用: {0}")]
    ServiceUnavailable(String),

    #[error("數據庫錯誤: {0}")]
    DatabaseError(String),

    #[error("資源不存在")]
    NotFound,

    #[error("無效請求: {0}")]
    BadRequest(String),

    #[error("內部服務器錯誤: {0}")]
    InternalError(String),

    #[error("HTTP 錯誤: {0}")]
    HttpError(String),

    #[error("序列化錯誤: {0}")]
    SerializationError(String),

    #[error("驗證錯誤: {0}")]
    ValidationError(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg),
            AppError::DatabaseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::NotFound => (StatusCode::NOT_FOUND, "資源不存在".to_string()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::HttpError(msg) => (StatusCode::BAD_GATEWAY, msg),
            AppError::SerializationError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::ValidationError(msg) => (StatusCode::BAD_REQUEST, msg),
        };

        let body = Json(json!({
            "error": error_message,
            "status": status.as_u16()
        }));

        (status, body).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
