mod helpers;

use std::time::{Duration, Instant};

use helpers::{client_with, envelope, error_envelope};
use sequence_rs::model::rule::TriggerRuleRequest;
use sequence_rs::model::transfer::CreateTransferRequest;
use sequence_rs::prelude::*;
use sequence_rs::{
    ClientError, Config, Credentials, ListAccountsParams, RateLimit, RetryPolicy, Sequence,
};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Fast retry policy so tests don't actually sleep for long.
fn fast_retry(max_attempts: u32) -> RetryPolicy {
    RetryPolicy {
        max_attempts,
        initial_backoff: Duration::from_millis(1),
        max_backoff: Duration::from_millis(5),
        multiplier: 2.0,
        respect_retry_after: false,
        jitter: false,
        max_elapsed_time: None,
    }
}

fn empty_accounts() -> serde_json::Value {
    envelope(json!({ "items": [], "pagination": { "page": 1, "pageSize": 10 } }))
}

fn a_transfer() -> serde_json::Value {
    envelope(json!({
        "id": "t1", "amountInCents": 10000, "direction": "INTERNAL", "origin": "USER",
        "source": null, "destination": null, "status": "PROCESSING",
        "ruleId": null, "ruleExecutionId": null, "errorCode": null,
        "createdAt": "2024-04-23T09:15:00Z", "completedAt": null
    }))
}

#[tokio::test]
async fn retries_on_429_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .respond_with(
            ResponseTemplate::new(429).set_body_json(error_envelope("RATE_LIMIT_EXCEEDED", "slow")),
        )
        .up_to_n_times(2)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(empty_accounts()))
        .with_priority(2)
        .mount(&server)
        .await;

    let client = client_with(&server.uri(), fast_retry(3), None);
    client
        .accounts(&ListAccountsParams::default())
        .await
        .unwrap();
    assert_eq!(server.received_requests().await.unwrap().len(), 3);
}

#[tokio::test]
async fn no_retries_makes_a_single_attempt_and_returns_immediately() {
    let server = MockServer::start().await;
    // A 429 that the *default* policy would retry — proves retries are off.
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .respond_with(
            ResponseTemplate::new(429).set_body_json(error_envelope("RATE_LIMIT_EXCEEDED", "slow")),
        )
        .mount(&server)
        .await;

    let client = Sequence::with_config(
        Credentials::new("k"),
        Config {
            api_base_url: format!("{}/", server.uri()),
            rate_limit: None,
            ..Default::default()
        }
        .no_retries(),
    );

    let err = client
        .accounts(&ListAccountsParams::default())
        .await
        .unwrap_err();
    match err {
        ClientError::Api(api) => assert_eq!(api.code, "RATE_LIMIT_EXCEEDED"),
        other => panic!("expected typed Api error, got {other:?}"),
    }
    // Exactly one attempt — no retry.
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn gives_up_after_max_attempts() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .respond_with(
            ResponseTemplate::new(429).set_body_json(error_envelope("RATE_LIMIT_EXCEEDED", "slow")),
        )
        .mount(&server)
        .await;

    let client = client_with(&server.uri(), fast_retry(2), None);
    let err = client
        .accounts(&ListAccountsParams::default())
        .await
        .unwrap_err();
    assert!(matches!(err, ClientError::Api(_)));
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn retries_transient_post_regardless_of_idempotency_key() {
    // Retries are decided by the failure being transient, not by the key. A
    // POST without a key still retries on 500 — the caller owns dedup safety.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/transfers"))
        .respond_with(
            ResponseTemplate::new(500).set_body_json(error_envelope("UNEXPECTED_ERROR", "boom")),
        )
        .mount(&server)
        .await;

    let client = client_with(&server.uri(), fast_retry(3), None);
    let req = CreateTransferRequest {
        source_account_id: "a".into(),
        destination_account_id: "b".into(),
        amount_in_cents: 10000,
        description: None,
    };
    let err = client.create_transfer(&req, None).await.unwrap_err();
    assert!(matches!(err, ClientError::Api(_)));
    assert_eq!(server.received_requests().await.unwrap().len(), 3);
}

/// A caller-supplied key must ride every attempt unchanged — else a retry could
/// duplicate the transfer, which would be the client's fault.
#[tokio::test]
async fn create_transfer_resends_idempotency_key_on_every_retry() {
    let server = MockServer::start().await;
    // Fail twice, succeed on the third attempt → exercises two retries.
    Mock::given(method("POST"))
        .and(path("/transfers"))
        .respond_with(
            ResponseTemplate::new(500).set_body_json(error_envelope("UNEXPECTED_ERROR", "boom")),
        )
        .up_to_n_times(2)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/transfers"))
        .respond_with(ResponseTemplate::new(200).set_body_json(a_transfer()))
        .with_priority(2)
        .mount(&server)
        .await;

    let client = client_with(&server.uri(), fast_retry(3), None);
    let req = CreateTransferRequest {
        source_account_id: "a".into(),
        destination_account_id: "b".into(),
        amount_in_cents: 10000,
        description: None,
    };
    let transfer = client
        .create_transfer(&req, Some("stable-key"))
        .await
        .unwrap();
    assert_eq!(transfer.id, "t1");

    // All three attempts carry the same key, unchanged.
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 3);
    for r in &requests {
        assert_eq!(
            r.headers.get("idempotency-key").unwrap().to_str().unwrap(),
            "stable-key"
        );
    }
}

/// Same guarantee for the other key-bearing endpoint, `trigger_rule`.
#[tokio::test]
async fn trigger_rule_resends_idempotency_key_on_every_retry() {
    let server = MockServer::start().await;
    let rule_id = "551ff9b6-ddf1-4110-b611-1b11044b72d4";
    Mock::given(method("POST"))
        .and(path(format!("/rules/{rule_id}/trigger")))
        .respond_with(
            ResponseTemplate::new(503).set_body_json(error_envelope("UNEXPECTED_ERROR", "boom")),
        )
        .up_to_n_times(2)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/rules/{rule_id}/trigger")))
        .respond_with(
            ResponseTemplate::new(202).set_body_json(envelope(json!({ "executionId": "e1" }))),
        )
        .with_priority(2)
        .mount(&server)
        .await;

    let client = client_with(&server.uri(), fast_retry(3), None);
    let resp = client
        .trigger_rule(
            &rule_id.to_string(),
            &TriggerRuleRequest::default(),
            Some("trigger-key"),
        )
        .await
        .unwrap();
    assert_eq!(resp.execution_id, "e1");

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 3);
    for r in &requests {
        assert_eq!(
            r.headers.get("idempotency-key").unwrap().to_str().unwrap(),
            "trigger-key"
        );
    }
}

/// And the inverse: when no key is supplied, the client never invents one — not
/// on the first attempt, not on any retry.
#[tokio::test]
async fn no_idempotency_key_is_never_fabricated_across_retries() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/transfers"))
        .respond_with(
            ResponseTemplate::new(500).set_body_json(error_envelope("UNEXPECTED_ERROR", "boom")),
        )
        .mount(&server)
        .await;

    let client = client_with(&server.uri(), fast_retry(3), None);
    let req = CreateTransferRequest {
        source_account_id: "a".into(),
        destination_account_id: "b".into(),
        amount_in_cents: 10000,
        description: None,
    };
    let _ = client.create_transfer(&req, None).await;

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 3);
    for r in &requests {
        assert!(
            !r.headers.contains_key("idempotency-key"),
            "client must not fabricate an idempotency-key"
        );
    }
}

#[tokio::test]
async fn rate_limit_allows_burst_under_capacity() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(empty_accounts()))
        .mount(&server)
        .await;

    // Even at a low 60/min, a token bucket lets a short burst through with no
    // added delay (a per-call pacing limiter would have spread these over ~4s).
    let client = client_with(
        &server.uri(),
        RetryPolicy::none(),
        Some(RateLimit {
            requests_per_minute: 60,
        }),
    );
    let start = Instant::now();
    for _ in 0..5 {
        client
            .accounts(&ListAccountsParams::default())
            .await
            .unwrap();
    }
    assert!(
        start.elapsed() < Duration::from_millis(500),
        "burst of 5 under capacity should not be throttled, elapsed = {:?}",
        start.elapsed()
    );
}

#[tokio::test]
async fn retry_budget_is_a_hard_deadline() {
    let server = MockServer::start().await;
    // Always 429 — without a budget this would run all max_attempts.
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .respond_with(
            ResponseTemplate::new(429).set_body_json(error_envelope("RATE_LIMIT_EXCEEDED", "slow")),
        )
        .mount(&server)
        .await;

    // High attempt cap, tight overall budget: the deadline fires first.
    let policy = RetryPolicy {
        max_attempts: 100,
        initial_backoff: Duration::from_millis(40),
        max_backoff: Duration::from_secs(1),
        multiplier: 2.0,
        respect_retry_after: false,
        jitter: false,
        max_elapsed_time: Some(Duration::from_millis(120)),
    };
    let client = client_with(&server.uri(), policy, None);
    let err = client
        .accounts(&ListAccountsParams::default())
        .await
        .unwrap_err();
    assert!(matches!(err, ClientError::Timeout(_)), "got {err:?}");
    // The deadline cut it far short of max_attempts.
    let n = server.received_requests().await.unwrap().len();
    assert!(
        n < 10,
        "budget should stop retries early, made {n} requests"
    );
}

#[tokio::test]
async fn no_throttle_when_disabled() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(empty_accounts()))
        .mount(&server)
        .await;

    let client = client_with(&server.uri(), RetryPolicy::none(), None);
    let start = Instant::now();
    for _ in 0..5 {
        client
            .accounts(&ListAccountsParams::default())
            .await
            .unwrap();
    }
    // With throttling off, five local calls finish well under the 100ms a
    // single throttled gap would cost.
    assert!(start.elapsed() < Duration::from_millis(100));
}
