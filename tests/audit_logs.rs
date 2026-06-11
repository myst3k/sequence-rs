mod helpers;

use helpers::{client, envelope};
use sequence_rs::model::audit_log::{AuditOutcome, SequenceApiAction};
use sequence_rs::prelude::*;
use sequence_rs::ListAuditLogParams;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn page(items: serde_json::Value, p: i64, ps: i64) -> serde_json::Value {
    envelope(
        json!({ "items": items, "pagination": { "page": p, "pageSize": ps, "hasNextPage": false } }),
    )
}

fn success_entry(id: &str) -> serde_json::Value {
    json!({
        "id": id, "createdAt": "2026-06-10T00:00:00Z",
        "apiKeyId": "key-1", "apiKeyName": "maestro",
        "path": "/platform/v1/accounts", "action": "LIST_ACCOUNTS",
        "requestId": "req-1", "outcome": "SUCCESS", "errorCode": null
    })
}

fn failure_entry(id: &str) -> serde_json::Value {
    json!({
        "id": id, "createdAt": "2026-06-10T00:01:00Z",
        "apiKeyId": "key-1", "apiKeyName": "maestro",
        "path": "/platform/v1/accounts/x", "action": null,
        "requestId": "req-2", "outcome": "FAILURE", "errorCode": "ACCESS_DENIED"
    })
}

#[tokio::test]
async fn list_audit_logs_parses_success_and_failure() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/audit-logs"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page(
            json!([success_entry("a1"), failure_entry("a2")]),
            1,
            10,
        )))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let res = client
        .audit_logs(&ListAuditLogParams::default())
        .await
        .unwrap();
    assert_eq!(res.items.len(), 2);

    let ok = &res.items[0];
    assert!(matches!(ok.outcome, AuditOutcome::Success));
    assert!(matches!(ok.action, Some(SequenceApiAction::ListAccounts)));
    assert!(ok.error_code.is_none());

    let bad = &res.items[1];
    assert!(matches!(bad.outcome, AuditOutcome::Failure));
    assert!(bad.action.is_none());
    assert_eq!(bad.error_code.as_deref(), Some("ACCESS_DENIED"));
}

#[tokio::test]
async fn list_audit_logs_sends_filters() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/audit-logs"))
        .and(query_param("apiKeyId", "key-9"))
        .and(query_param("action", "CREATE_TRANSFER"))
        .and(query_param("from", "2026-06-01T00:00:00Z"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page(json!([]), 1, 10)))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let params = ListAuditLogParams {
        api_key_id: Some("key-9".to_string()),
        action: Some(SequenceApiAction::CreateTransfer),
        from: Some("2026-06-01T00:00:00Z".to_string()),
        ..Default::default()
    };
    client.audit_logs(&params).await.unwrap();
}
