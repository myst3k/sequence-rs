use serde::Deserialize;
use thiserror::Error;

pub type ModelResult<T> = Result<T, ModelError>;

/// Outer error envelope: `{ "error": { "code": "...", "message": "..." } }`.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiErrorEnvelope {
    pub error: ApiError,
}

/// Body returned by the API on a non-2xx response.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ApiError {}

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("json parse error: {0}")]
    ParseJson(#[from] serde_json::Error),

    #[error("input/output error: {0}")]
    Io(#[from] std::io::Error),

    /// A request failed client-side validation before being sent.
    #[error("validation error: {0}")]
    Validation(String),
}
