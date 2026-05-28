//! Shared test scaffolding: build a `Sequence` pointed at a wiremock server.
// Each integration-test binary uses a different subset of these helpers.
#![allow(dead_code)]

use sequence_rs::{Config, Credentials, RetryPolicy, Sequence};

pub const TEST_KEY: &str = "seq_test_key";

/// A client pointed at `base_url`, with throttling and retries disabled so
/// tests are deterministic and fast. Use [`client_with`] to override.
pub fn client(base_url: &str) -> Sequence {
    client_with(base_url, RetryPolicy::none(), None)
}

/// A client with explicit retry policy / rate limit.
pub fn client_with(
    base_url: &str,
    retry: RetryPolicy,
    rate_limit: Option<sequence_rs::RateLimit>,
) -> Sequence {
    Sequence::with_config(
        Credentials::new(TEST_KEY),
        Config {
            api_base_url: format!("{}/", base_url.trim_end_matches('/')),
            rate_limit,
            retry,
            ..Default::default()
        },
    )
}

/// Wrap a `data` body in the Sequence response envelope.
pub fn envelope(data: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "data": data, "requestId": "req-test" })
}

/// The spec's `{ error: { code, message } }` envelope.
pub fn error_envelope(code: &str, message: &str) -> serde_json::Value {
    serde_json::json!({ "error": { "code": code, "message": message } })
}
