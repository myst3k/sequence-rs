use std::sync::Arc;
use std::time::Duration;

use reqwest::header::RETRY_AFTER;
use reqwest::{Method, RequestBuilder, StatusCode};
use serde_json::Value;

use super::retry::Throttle;
use super::{BaseHttpClient, Headers, Query, RateLimit, RetryPolicy};

#[derive(thiserror::Error, Debug)]
pub enum ReqwestError {
    #[error("request: {0}")]
    Client(#[from] reqwest::Error),

    /// Non-2xx response. `body` is the raw payload — for Sequence a JSON error
    /// object that callers can deserialize (`sequence-rs-model::error::ApiError`).
    #[error("status {status}: {body}")]
    Status { status: StatusCode, body: String },

    /// The retry budget (`RetryPolicy::max_elapsed_time`) elapsed; the in-flight
    /// request was cancelled.
    #[error("timed out after {after:?}")]
    Timeout { after: Duration },
}

#[derive(Debug, Clone)]
pub struct ReqwestClient {
    client: reqwest::Client,
    retry: RetryPolicy,
    throttle: Option<Arc<Throttle>>,
}

fn build_client() -> reqwest::Client {
    reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(30))
        .user_agent(concat!("sequence-rs/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("reqwest client with default options must build")
}

impl Default for ReqwestClient {
    fn default() -> Self {
        Self {
            client: build_client(),
            retry: RetryPolicy::default(),
            throttle: Some(Arc::new(Throttle::new(
                RateLimit::default().requests_per_minute,
            ))),
        }
    }
}

impl ReqwestClient {
    /// Build a client with an explicit retry policy and (optional) client-side
    /// rate limit. `rate_limit: None` disables proactive throttling.
    pub fn with_config(retry: RetryPolicy, rate_limit: Option<RateLimit>) -> Self {
        Self {
            client: build_client(),
            retry,
            throttle: rate_limit.map(|rl| Arc::new(Throttle::new(rl.requests_per_minute))),
        }
    }

    /// Whether the failure is transient and worth retrying. Decided by the
    /// failure alone, not the method or idempotency key — callers make a
    /// mutating retry safe by supplying a key (re-sent on every attempt).
    fn is_retryable(&self, err: &ReqwestError) -> bool {
        match err {
            // Network blips: timeouts and connection failures.
            ReqwestError::Client(e) => e.is_timeout() || e.is_connect(),
            ReqwestError::Status { status, .. } => {
                matches!(status.as_u16(), 429 | 500 | 502 | 503 | 504)
            }
            // Produced by the budget deadline, never observed here.
            ReqwestError::Timeout { .. } => false,
        }
    }

    async fn request<D>(
        &self,
        method: Method,
        url: &str,
        headers: Option<&Headers>,
        add_data: D,
    ) -> Result<String, ReqwestError>
    where
        D: Fn(RequestBuilder) -> RequestBuilder,
    {
        let run = async {
            let mut attempt: u32 = 1;
            loop {
                // Proactive throttle gate (also paces retries).
                if let Some(throttle) = &self.throttle {
                    throttle.acquire().await;
                }

                let (result, retry_after) = self
                    .send_once(method.clone(), url, headers, &add_data)
                    .await;

                match result {
                    Ok(body) => return Ok(body),
                    Err(err) => {
                        if attempt < self.retry.max_attempts && self.is_retryable(&err) {
                            let delay = self.retry.backoff(attempt, retry_after);
                            tracing::debug!(
                                %method, url, attempt, ?delay, retry_after,
                                error = %err,
                                "retrying request after transient failure"
                            );
                            tokio::time::sleep(delay).await;
                            attempt += 1;
                            continue;
                        }
                        return Err(err);
                    }
                }
            }
        };

        // `max_elapsed_time` is a hard deadline over the whole operation
        // (attempts, backoffs, throttle waits): on expiry the in-flight request
        // is cancelled and a `Timeout` returned.
        match self.retry.max_elapsed_time {
            Some(budget) => tokio::time::timeout(budget, run)
                .await
                .unwrap_or_else(|_| Err(ReqwestError::Timeout { after: budget })),
            None => run.await,
        }
    }

    /// Perform a single attempt. Returns the result plus any parsed
    /// `Retry-After` (seconds) so the caller can honour it on a `429`.
    async fn send_once<D>(
        &self,
        method: Method,
        url: &str,
        headers: Option<&Headers>,
        add_data: &D,
    ) -> (Result<String, ReqwestError>, Option<u64>)
    where
        D: Fn(RequestBuilder) -> RequestBuilder,
    {
        let mut request = self.client.request(method.clone(), url);
        if let Some(headers) = headers {
            let headers = headers
                .try_into()
                .expect("auth/idempotency headers contain only ASCII");
            request = request.headers(headers);
        }
        request = add_data(request);

        let response = match request.send().await {
            Ok(resp) => resp,
            Err(e) => return (Err(ReqwestError::Client(e)), None),
        };

        let status = response.status();
        let retry_after = response
            .headers()
            .get(RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<u64>().ok());

        let body = match response.text().await {
            Ok(b) => b,
            Err(e) => return (Err(ReqwestError::Client(e)), retry_after),
        };

        tracing::debug!(%method, url, status = status.as_u16(), "request completed");

        if status.is_success() {
            (Ok(body), retry_after)
        } else {
            (Err(ReqwestError::Status { status, body }), retry_after)
        }
    }
}

impl BaseHttpClient for ReqwestClient {
    type Error = ReqwestError;

    #[inline]
    async fn get(
        &self,
        url: &str,
        headers: Option<&Headers>,
        payload: &Query,
    ) -> Result<String, Self::Error> {
        self.request(Method::GET, url, headers, |req| req.query(payload))
            .await
    }

    #[inline]
    async fn post(
        &self,
        url: &str,
        headers: Option<&Headers>,
        payload: &Value,
    ) -> Result<String, Self::Error> {
        self.request(Method::POST, url, headers, |req| req.json(payload))
            .await
    }
}
