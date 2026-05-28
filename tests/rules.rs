mod helpers;

use helpers::{client, envelope};
use sequence_rs::model::rule::{Trigger, TriggerRuleRequest};
use sequence_rs::prelude::*;
use serde_json::json;
use wiremock::matchers::{body_json, header, header_exists, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const RULE_ID: &str = "551ff9b6-ddf1-4110-b611-1b11044b72d4";

fn full_rule() -> serde_json::Value {
    json!({
        "id": RULE_ID,
        "name": "Auto-save on deposit",
        "description": "Saves 20% of every incoming deposit",
        "status": "ENABLED",
        "trigger": { "type": "ON_FUNDS_TRANSFERRED", "accountId": "c7a7f26f-2ca5-4ae5-825a-70260591247c" },
        "steps": [{
            "conditions": null,
            "actions": [{
                "type": "FIXED", "amountInCents": 5000,
                "source": { "id": "c7a7f26f", "type": "INCOME_SOURCE", "name": null },
                "destination": { "id": "fae66a7b", "type": "POD", "name": null },
                "groupIndex": 0, "upToEnabled": true, "isDirectDeposit": false,
                "limit": null, "achDescription": null
            }]
        }],
        "createdAt": "2024-03-01T10:00:00Z",
        "updatedAt": "2024-03-15T14:30:00Z",
        "deletedAt": null
    })
}

#[tokio::test]
async fn get_rule_deserializes_full_shape() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/rules/{RULE_ID}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(envelope(full_rule())))
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let rule = client.rule(&RULE_ID.to_string()).await.unwrap();
    assert!(matches!(rule.trigger, Trigger::OnFundsTransferred { .. }));
    assert_eq!(rule.steps.len(), 1);
}

#[tokio::test]
async fn trigger_rule_without_key_auto_generates_idempotency_header() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/rules/{RULE_ID}/trigger")))
        // With no caller key, the client generates one, so the header is present.
        .and(header_exists("idempotency-key"))
        .respond_with(
            ResponseTemplate::new(202).set_body_json(envelope(json!({ "executionId": "exec-1" }))),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let resp = client
        .trigger_rule(&RULE_ID.to_string(), &TriggerRuleRequest::default(), None)
        .await
        .unwrap();
    assert_eq!(resp.execution_id, "exec-1");
}

#[tokio::test]
async fn trigger_rule_with_key_sends_it_verbatim() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/rules/{RULE_ID}/trigger")))
        .and(header("idempotency-key", "my-stable-key"))
        .and(header_exists("idempotency-key"))
        .respond_with(
            ResponseTemplate::new(202).set_body_json(envelope(json!({ "executionId": "exec-2" }))),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let resp = client
        .trigger_rule(
            &RULE_ID.to_string(),
            &TriggerRuleRequest {
                execute_amount: Some(150000),
            },
            Some("my-stable-key"),
        )
        .await
        .unwrap();
    assert_eq!(resp.execution_id, "exec-2");
}

#[tokio::test]
async fn trigger_rule_serializes_execute_amount() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/rules/{RULE_ID}/trigger")))
        .and(body_json(json!({ "executeAmount": 150000 })))
        .respond_with(
            ResponseTemplate::new(202).set_body_json(envelope(json!({ "executionId": "exec-3" }))),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    client
        .trigger_rule(
            &RULE_ID.to_string(),
            &TriggerRuleRequest {
                execute_amount: Some(150000),
            },
            None,
        )
        .await
        .unwrap();
}
