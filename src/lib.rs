//! Rust client for the Sequence Platform API.
//!
//! ```no_run
//! use sequence_rs::prelude::*;
//! use sequence_rs::{Credentials, Sequence};
//!
//! # async fn run() -> eyre::Result<()> {
//! let creds = Credentials::from_env().expect("SEQUENCE_API_KEY missing");
//! let client = Sequence::new(creds);
//! let page = client.accounts(&Default::default()).await?;
//! for account in page.items {
//!     println!("{} {}", account.id, account.name);
//! }
//! # Ok(()) }
//! ```

mod clients;
pub mod sequence;
mod stream;

pub use sequence_rs_http as http;
pub use sequence_rs_model as model;

// List-endpoint query builders, re-exported (the `clients` module is private).
pub use clients::base::{
    AccountRole, ListAccountTransfersParams, ListAccountsParams, ListRuleExecutionsParams,
    ListRulesParams,
};

use bon::Builder;
use secrecy::{ExposeSecret, Secret};
use sequence_rs_http::HttpError;
use std::collections::HashMap;
use std::env;
use thiserror::Error;

pub use crate::sequence::Sequence;
pub use sequence_rs_http::{RateLimit, RetryPolicy, DEFAULT_RATE_LIMIT_PER_MINUTE};

pub mod prelude {
    pub use crate::clients::BaseClient;
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("json parse error: {0}")]
    ParseJson(#[from] serde_json::Error),

    #[error("url parse error: {0}")]
    ParseUrl(#[from] url::ParseError),

    // Boxed because HttpError carries a response body that can be large.
    #[error("http error: {0}")]
    Http(Box<HttpError>),

    #[error("input/output error: {0}")]
    Io(#[from] std::io::Error),

    #[error("model error: {0}")]
    Model(#[from] model::error::ModelError),

    #[error("api error: {0}")]
    Api(#[from] model::error::ApiError),

    #[error("request timed out after the retry budget ({0:?})")]
    Timeout(std::time::Duration),

    #[error("missing credentials: set {0}")]
    MissingCredentials(&'static str),
}

pub type ClientResult<T> = Result<T, ClientError>;

impl From<HttpError> for ClientError {
    fn from(err: HttpError) -> Self {
        match &err {
            // Lift the `{ error: { code, message } }` envelope to a typed `Api`
            // error; otherwise keep the raw `Http` error.
            HttpError::Status { body, .. } => {
                match serde_json::from_str::<model::error::ApiErrorEnvelope>(body) {
                    Ok(env) => ClientError::Api(env.error),
                    Err(_) => ClientError::Http(Box::new(err)),
                }
            }
            HttpError::Timeout { after } => ClientError::Timeout(*after),
            HttpError::Client(_) => ClientError::Http(Box::new(err)),
        }
    }
}

/// Production base URL (the spec's `production` server).
pub const DEFAULT_API_BASE_URL: &str = "https://api.getsequence.io/platform/v1/";

/// Default page size for the `*_stream` helpers (the spec's max, for fewer round-trips).
pub const DEFAULT_PAGE_SIZE: u32 = 100;

#[derive(Debug, Clone)]
pub struct Config {
    pub api_base_url: String,
    /// Default page size used by the `*_stream` auto-paginators.
    pub default_page_size: u32,
    /// Client-side rate limiting. `None` disables it.
    pub rate_limit: Option<RateLimit>,
    /// How transient failures (`429`, `5xx`, network blips) are retried.
    pub retry: RetryPolicy,
    /// Sent as `x-called-reason` — a short description of what your code is
    /// doing (the spec recommends it for AI agents).
    pub called_reason: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_base_url: String::from(DEFAULT_API_BASE_URL),
            default_page_size: DEFAULT_PAGE_SIZE,
            rate_limit: Some(RateLimit::default()),
            retry: RetryPolicy::default(),
            called_reason: None,
        }
    }
}

impl Config {
    /// Disable retries: one attempt, the typed error returned immediately.
    /// Composes, e.g. `Config::default().no_retries()`.
    pub fn no_retries(mut self) -> Self {
        self.retry = RetryPolicy::none();
        self
    }
}

#[derive(Debug, Clone, Builder)]
#[builder(on(String, into), on(Secret<String>, into))]
pub struct Credentials {
    pub api_key: Secret<String>,
}

impl Default for Credentials {
    fn default() -> Self {
        Self {
            api_key: Secret::new(String::new()),
        }
    }
}

impl Credentials {
    /// Load `SEQUENCE_API_KEY` from the process environment.
    pub fn from_env() -> Option<Self> {
        #[cfg(feature = "env-file")]
        {
            let _ = dotenvy::dotenv();
        }
        let api_key = env::var("SEQUENCE_API_KEY").ok()?;
        Some(Self {
            api_key: api_key.into(),
        })
    }

    /// Build the `Authorization: Bearer …` header.
    pub fn auth_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert(
            "Authorization".to_owned(),
            format!("Bearer {}", self.api_key.expose_secret()),
        );
        headers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;

    #[test]
    fn credentials_builder() {
        let creds = Credentials::builder()
            .api_key("seq_test_123".to_string())
            .build();
        assert_eq!(creds.api_key.expose_secret(), "seq_test_123");
    }

    #[test]
    fn auth_header_is_bearer() {
        let creds = Credentials::builder()
            .api_key("seq_test_123".to_string())
            .build();
        let headers = creds.auth_headers();
        assert_eq!(headers.get("Authorization").unwrap(), "Bearer seq_test_123");
    }

    #[test]
    fn default_base_url_matches_spec() {
        assert_eq!(
            Config::default().api_base_url,
            "https://api.getsequence.io/platform/v1/"
        );
    }
}
