//! Values the live API added after v0.2: approval statuses, card payouts,
//! optional schedule start, and `hasNextPage`-driven pagination.

mod helpers;

use futures::TryStreamExt;
use helpers::{client, envelope};
use sequence_rs::model::rule::{RuleExecutionStatus, Trigger, TriggerRuleRequest};
use sequence_rs::model::transaction::CardTransactionSubtype;
use sequence_rs::model::transfer::{
    CreateTransferRequest, TransferOrigin, TransferStatus, TransferStatusFilter,
};
use sequence_rs::prelude::*;
use sequence_rs::{ListAccountTransfersParams, ListRuleExecutionsParams, ListRulesParams};
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn transfer(id: &str, status: &str, origin: &str) -> serde_json::Value {
    json!({
        "id": id, "amountInCents": 2500, "direction": "INTERNAL", "origin": origin,
        "source": null, "destination": null, "status": status,
        "ruleId": null, "ruleExecutionId": null, "errorCode": null,
        "createdAt": "2026-01-05T09:00:00Z", "completedAt": null
    })
}

fn page(items: serde_json::Value, p: u32, ps: u32, has_next: bool) -> serde_json::Value {
    envelope(json!({
        "items": items,
        "pagination": { "page": p, "pageSize": ps, "hasNextPage": has_next }
    }))
}

fn request() -> CreateTransferRequest {
    CreateTransferRequest {
        source_account_id: "pod-a".into(),
        destination_account_id: "pod-b".into(),
        amount_in_cents: 2500,
        description: None,
    }
}

#[tokio::test]
async fn create_transfer_parses_approval_pending() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/transfers"))
        .respond_with(ResponseTemplate::new(201).set_body_json(envelope(transfer(
            "t1",
            "APPROVAL_PENDING",
            "USER",
        ))))
        .mount(&server)
        .await;

    let t = client(&server.uri())
        .create_transfer(&request(), None)
        .await
        .unwrap();
    assert_eq!(t.status, TransferStatus::ApprovalPending);
}

#[tokio::test]
async fn transfer_parses_denied_and_unknown_values() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/transfers/t2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(envelope(transfer(
            "t2",
            "APPROVAL_DENIED",
            "UNKNOWN",
        ))))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/transfers/t3"))
        .respond_with(ResponseTemplate::new(200).set_body_json(envelope(transfer(
            "t3",
            "SOME_FUTURE_STATUS",
            "SOME_FUTURE_ORIGIN",
        ))))
        .mount(&server)
        .await;

    let c = client(&server.uri());
    let denied = c.transfer(&"t2".to_string()).await.unwrap();
    assert_eq!(denied.status, TransferStatus::ApprovalDenied);
    assert_eq!(denied.origin, TransferOrigin::Unknown);
    let future = c.transfer(&"t3".to_string()).await.unwrap();
    assert_eq!(future.status, TransferStatus::Unknown);
    assert_eq!(future.origin, TransferOrigin::Unknown);
}

#[tokio::test]
async fn transfer_status_filter_sends_approval_pending() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/accounts/pod-a/transfers"))
        .and(query_param("status", "APPROVAL_PENDING"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page(json!([]), 1, 10, false)))
        .expect(1)
        .mount(&server)
        .await;

    let params = ListAccountTransfersParams {
        status: Some(TransferStatusFilter::ApprovalPending),
        ..Default::default()
    };
    client(&server.uri())
        .account_transfers(&"pod-a".to_string(), &params)
        .await
        .unwrap();
}

#[tokio::test]
async fn rule_execution_status_filter_sends_approval_denied() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rules/r1/executions"))
        .and(query_param("status", "APPROVAL_DENIED"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page(json!([]), 1, 10, false)))
        .expect(1)
        .mount(&server)
        .await;

    let params = ListRuleExecutionsParams {
        status: Some(RuleExecutionStatus::ApprovalDenied),
        ..Default::default()
    };
    client(&server.uri())
        .rule_executions(&"r1".to_string(), &params)
        .await
        .unwrap();
}

#[tokio::test]
async fn card_transaction_parses_payout() {
    let server = MockServer::start().await;
    let payout = json!({
        "id": "c1", "cardId": "card-1", "cardType": "DEBIT_CARD",
        "account": { "id": "pod-a", "name": "Spending", "type": "POD", "isDeleted": false },
        "direction": "MONEY_IN", "subtype": "PAYOUT", "status": "COMPLETE",
        "amountInCents": 1200, "description": "Payout",
        "createdAt": "2026-01-05T09:00:00Z", "completedAt": "2026-01-05T09:00:01Z"
    });
    Mock::given(method("GET"))
        .and(path("/card-transactions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page(json!([payout]), 1, 10, false)))
        .mount(&server)
        .await;

    let res = client(&server.uri())
        .card_transactions(&"pod-a".to_string(), &Default::default())
        .await
        .unwrap();
    assert_eq!(res.items[0].subtype, CardTransactionSubtype::Payout);
}

#[tokio::test]
async fn scheduled_rule_without_start_date_parses() {
    let server = MockServer::start().await;
    let rule = json!({
        "id": "r1", "name": "Weekly sweep", "description": null, "status": "ENABLED",
        "trigger": { "type": "SCHEDULED", "scheduleType": "WEEKLY", "accountId": "pod-a" },
        "steps": [],
        "createdAt": "2026-01-01T00:00:00Z", "updatedAt": "2026-01-01T00:00:00Z", "deletedAt": null
    });
    Mock::given(method("GET"))
        .and(path("/rules/r1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(envelope(rule)))
        .mount(&server)
        .await;

    let r = client(&server.uri()).rule(&"r1".to_string()).await.unwrap();
    assert!(matches!(
        r.trigger,
        Trigger::Scheduled {
            start_date: None,
            ..
        }
    ));
}

#[tokio::test]
async fn trigger_rule_reports_approval_status() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rules/r1/trigger"))
        .respond_with(ResponseTemplate::new(202).set_body_json(envelope(json!({
            "executionId": "e1",
            "status": "APPROVAL_PENDING",
            "reasonForApproval": "large move",
            "approvalUrl": "https://example.test/approve/e1"
        }))))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rules/r2/trigger"))
        .respond_with(
            ResponseTemplate::new(202).set_body_json(envelope(json!({ "executionId": "e2" }))),
        )
        .mount(&server)
        .await;

    let c = client(&server.uri());
    let req = TriggerRuleRequest::default();
    let pending = c.trigger_rule(&"r1".to_string(), &req, None).await.unwrap();
    assert_eq!(pending.status, Some(RuleExecutionStatus::ApprovalPending));
    assert_eq!(
        pending.approval_url.as_deref(),
        Some("https://example.test/approve/e1")
    );
    let bare = c.trigger_rule(&"r2".to_string(), &req, None).await.unwrap();
    assert_eq!(bare.status, None);
}

#[tokio::test]
async fn stream_follows_has_next_page_past_a_short_page() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rules"))
        .and(query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page(
            json!([rule_summary("r1")]),
            1,
            10,
            true,
        )))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rules"))
        .and(query_param("page", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page(
            json!([rule_summary("r2")]),
            2,
            10,
            false,
        )))
        .expect(1)
        .mount(&server)
        .await;

    let params = ListRulesParams::default();
    let all: Vec<_> = client(&server.uri())
        .rules_stream(&params)
        .try_collect()
        .await
        .unwrap();
    assert_eq!(all.len(), 2);
}

#[tokio::test]
async fn stream_starts_at_caller_page_and_clamps_page_size() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rules"))
        .and(query_param("page", "3"))
        .and(query_param("pageSize", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page(
            json!([rule_summary("r3")]),
            3,
            100,
            false,
        )))
        .expect(1)
        .mount(&server)
        .await;

    let params = ListRulesParams {
        page: Some(3),
        page_size: Some(500),
        ..Default::default()
    };
    let all: Vec<_> = client(&server.uri())
        .rules_stream(&params)
        .try_collect()
        .await
        .unwrap();
    assert_eq!(all.len(), 1);
}

fn rule_summary(id: &str) -> serde_json::Value {
    json!({
        "id": id, "name": null, "description": null, "status": "ENABLED", "isSupported": true,
        "createdAt": "2026-01-01T00:00:00Z", "updatedAt": "2026-01-01T00:00:00Z", "deletedAt": null
    })
}
