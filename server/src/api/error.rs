use axum::{
    body::to_bytes,
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
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

    /// Best-fit code for an HTTP status produced outside a handler (e.g. an
    /// Axum extractor rejection or routing fallback). Other 4xx statuses map to
    /// `InvalidRequest` and 5xx to `Internal`.
    pub fn for_status(status: StatusCode) -> ErrorCode {
        match status {
            StatusCode::NOT_FOUND => ErrorCode::NotFound,
            StatusCode::BAD_REQUEST => ErrorCode::InvalidRequest,
            StatusCode::CONFLICT => ErrorCode::Conflict,
            StatusCode::UNAUTHORIZED => ErrorCode::Unauthorized,
            StatusCode::PAYLOAD_TOO_LARGE => ErrorCode::PayloadTooLarge,
            StatusCode::SERVICE_UNAVAILABLE => ErrorCode::ServiceUnavailable,
            s if s.is_server_error() => ErrorCode::Internal,
            _ => ErrorCode::InvalidRequest,
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

/// Middleware guaranteeing every error response carries the structured
/// `{ code, error }` body — including framework-level failures that never reach
/// a handler, such as `Path`/`Json` extractor rejections and routing fallbacks,
/// which otherwise return Axum's default `text/plain` body. Error responses that
/// already have a JSON body (everything built via [`ApiError`]) pass through
/// untouched.
pub async fn ensure_coded_errors(request: Request, next: Next) -> Response {
    let response = next.run(request).await;

    let status = response.status();
    if !status.is_client_error() && !status.is_server_error() {
        return response;
    }

    let already_json = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("application/json"));
    if already_json {
        return response;
    }

    let (parts, body) = response.into_parts();
    let body = to_bytes(body, usize::MAX).await.unwrap_or_default();
    let message = String::from_utf8_lossy(&body).trim().to_string();
    let message = if message.is_empty() {
        status
            .canonical_reason()
            .unwrap_or("Request failed")
            .to_string()
    } else {
        message
    };

    let mut coded = (
        status,
        Json(ErrorResponse {
            code: ErrorCode::for_status(status),
            error: message,
        }),
    )
        .into_response();

    // Preserve headers added by inner layers (e.g. CORS), but keep the
    // Content-Type/Content-Length that match the new JSON body.
    let headers = coded.headers_mut();
    for (name, value) in parts.headers.iter() {
        if name == header::CONTENT_TYPE || name == header::CONTENT_LENGTH {
            continue;
        }
        headers.append(name, value.clone());
    }

    coded
}
