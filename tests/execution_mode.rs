mod helpers;

use helpers::{client, envelope};
use sequence_rs::model::rule::{ExecutionMode, RuleExecutionListMode, TriggerRuleRequest};
use sequence_rs::prelude::*;
use sequence_rs::ListRuleExecutionsParams;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn trigger_rule_sends_simulation_flag() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rules/rule-1/trigger"))
        .respond_with(
            ResponseTemplate::new(202).set_body_json(envelope(json!({ "executionId": "exec-1" }))),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let req = TriggerRuleRequest {
        simulation: Some(true),
        ..Default::default()
    };
    let res = client
        .trigger_rule(&"rule-1".to_string(), &req, None)
        .await
        .unwrap();
    assert_eq!(res.execution_id, "exec-1");

    let reqs = server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&reqs[0].body).unwrap();
    assert_eq!(body["simulation"], json!(true));
    // execute_amount is `skip_serializing_if = None` → omitted from the body.
    assert!(body.get("executeAmount").is_none());
}

#[tokio::test]
async fn list_executions_filters_by_mode_and_parses_it() {
    let server = MockServer::start().await;
    let exec = json!({
        "id": "e1", "ruleId": "rule-1", "status": "EXECUTED",
        "executionMode": "SIMULATION", "createdAt": "2026-06-10T00:00:00Z"
    });
    Mock::given(method("GET"))
        .and(path("/rules/rule-1/executions"))
        .and(query_param("executionMode", "ALL"))
        .respond_with(ResponseTemplate::new(200).set_body_json(envelope(json!({
            "items": [exec], "pagination": { "page": 1, "pageSize": 10, "hasNextPage": false }
        }))))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let params = ListRuleExecutionsParams {
        execution_mode: Some(RuleExecutionListMode::All),
        ..Default::default()
    };
    let res = client
        .rule_executions(&"rule-1".to_string(), &params)
        .await
        .unwrap();
    assert_eq!(res.items.len(), 1);
    assert!(matches!(
        res.items[0].execution_mode,
        ExecutionMode::Simulation
    ));
}

#[tokio::test]
async fn execution_mode_defaults_to_live_when_absent() {
    // A response without `executionMode` (older shape) must default to LIVE.
    let server = MockServer::start().await;
    let exec = json!({
        "id": "e1", "ruleId": "rule-1", "status": "EXECUTED", "createdAt": "2026-06-10T00:00:00Z"
    });
    Mock::given(method("GET"))
        .and(path("/rules/rule-1/executions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(envelope(json!({
            "items": [exec], "pagination": { "page": 1, "pageSize": 10 }
        }))))
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let res = client
        .rule_executions(&"rule-1".to_string(), &ListRuleExecutionsParams::default())
        .await
        .unwrap();
    assert!(matches!(res.items[0].execution_mode, ExecutionMode::Live));
}
