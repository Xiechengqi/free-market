use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use serde_json::Value;

/// soybean-admin expects `{ code, msg, data }` envelopes.
/// `code == "0000"` is success; non-zero is shown to the user.
/// Reserved logout codes: `8888,8889`; modal-logout: `7777,7778`; expired-token: `9999,9998,3333`.
pub const CODE_OK: &str = "0000";
pub const CODE_VALIDATION: &str = "4001";
pub const CODE_UNAUTHORIZED: &str = "4002";
pub const CODE_FORBIDDEN: &str = "4003";
pub const CODE_NOT_FOUND: &str = "4004";
pub const CODE_CONFLICT: &str = "4009";
pub const CODE_INTERNAL: &str = "5000";
pub const CODE_TOKEN_EXPIRED: &str = "9999";
pub const CODE_FORCE_LOGOUT: &str = "8888";

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub code: String,
    pub msg: String,
    pub data: Option<T>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            code: CODE_OK.to_string(),
            msg: "ok".to_string(),
            data: Some(data),
        }
    }

    pub fn ok_empty() -> ApiResponse<Value> {
        ApiResponse {
            code: CODE_OK.to_string(),
            msg: "ok".to_string(),
            data: Some(Value::Null),
        }
    }

    pub fn fail(code: &str, msg: impl Into<String>) -> ApiResponse<Value> {
        ApiResponse {
            code: code.to_string(),
            msg: msg.into(),
            data: None,
        }
    }
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> axum::response::Response {
        // soybean's axios wrapper reads response.data.code unconditionally; always send 200.
        (StatusCode::OK, Json(self)).into_response()
    }
}

pub type ApiResult<T> = Result<ApiResponse<T>, ApiError>;

#[derive(Debug)]
pub struct ApiError {
    pub code: String,
    pub msg: String,
}

impl ApiError {
    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self {
            code: CODE_UNAUTHORIZED.to_string(),
            msg: msg.into(),
        }
    }
    pub fn token_expired(msg: impl Into<String>) -> Self {
        Self {
            code: CODE_TOKEN_EXPIRED.to_string(),
            msg: msg.into(),
        }
    }
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self {
            code: CODE_FORBIDDEN.to_string(),
            msg: msg.into(),
        }
    }
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            code: CODE_VALIDATION.to_string(),
            msg: msg.into(),
        }
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self {
            code: CODE_NOT_FOUND.to_string(),
            msg: msg.into(),
        }
    }
    pub fn conflict(msg: impl Into<String>) -> Self {
        Self {
            code: CODE_CONFLICT.to_string(),
            msg: msg.into(),
        }
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            code: CODE_INTERNAL.to_string(),
            msg: msg.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        ApiResponse::<Value>::fail(&self.code, self.msg).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(value: sqlx::Error) -> Self {
        ApiError::internal(format!("db: {value}"))
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(value: anyhow::Error) -> Self {
        ApiError::internal(format!("{value:?}"))
    }
}

impl From<crate::error::AppError> for ApiError {
    fn from(value: crate::error::AppError) -> Self {
        use crate::error::AppError;
        match value {
            AppError::BadRequest(msg) => ApiError::bad_request(msg),
            AppError::NotFound(msg) => ApiError::not_found(msg),
            AppError::Conflict(msg) => ApiError::conflict(msg),
            AppError::Sqlx(err) => ApiError::internal(format!("db: {err}")),
            AppError::Anyhow(err) => ApiError::internal(format!("{err:?}")),
        }
    }
}
