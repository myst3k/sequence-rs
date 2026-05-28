//! HTTP layer for `sequence-rs`: a `reqwest` wrapper behind the
//! `BaseHttpClient` trait, with retries and client-side rate limiting.

mod common;
mod reqwest;
mod retry;

pub use self::reqwest::{ReqwestClient as HttpClient, ReqwestError as HttpError};
pub use common::{BaseHttpClient, Headers, Query};
pub use retry::{RateLimit, RetryPolicy, DEFAULT_RATE_LIMIT_PER_MINUTE};
