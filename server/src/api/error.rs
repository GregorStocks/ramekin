use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use utoipa::ToSchema;

/// Machine-readable error code surfaced in every error response.
///
/// Clients branch on this instead of parsing the human-readable `error`
/// message or guessing from the HTTP status, so changing the wording of a
/// message is never a contract change. Each code maps to exactly one HTTP
/// status (see [`ErrorCode::status`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The requested resource does not exist or is not visible to the caller.
    NotFound,
    /// The request was malformed or failed validation.
    InvalidRequest,
    /// The request conflicts with existing state (e.g. a duplicate).
    Conflict,
    /// Authentication is missing, malformed, or expired.
    Unauthorized,
    /// The request body was larger than the server accepts.
    PayloadTooLarge,
    /// An upstream dependency (LLM, scraping target, ...) was unavailable.
    ServiceUnavailable,
    /// An unexpected server-side failure.
    Internal,
}

impl ErrorCode {
    /// The HTTP status that this code is always returned with.
    pub fn status(self) -> StatusCode {
        match self {
            ErrorCode::NotFound => StatusCode::NOT_FOUND,
            ErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
            ErrorCode::Conflict => StatusCode::CONFLICT,
            ErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorCode::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            ErrorCode::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// Shared error response body returned by every endpoint.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ErrorResponse {
    /// Machine-readable error code; branch on this, not on `error`.
    pub code: ErrorCode,
    /// Human-readable message for display and debugging.
    pub error: String,
}

/// An API error: a code (which determines the HTTP status) plus a
/// human-readable message. Build one with the constructors below and return
/// `err.into_response()` (or return `Result<_, ApiError>` and use `?`).
#[derive(Debug, Clone)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
}

impl ApiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// 404 — resource not found.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    /// 400 — malformed request or failed validation.
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidRequest, message)
    }

    /// 409 — conflict with existing state.
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Conflict, message)
    }

    /// 401 — missing, invalid, or expired authentication.
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unauthorized, message)
    }

    /// 413 — request body too large.
    pub fn payload_too_large(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::PayloadTooLarge, message)
    }

    /// 503 — an upstream dependency was unavailable.
    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::ServiceUnavailable, message)
    }

    /// 500 — unexpected server-side failure.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.code.status(),
            Json(ErrorResponse {
                code: self.code,
                error: self.message,
            }),
        )
            .into_response()
    }
}
