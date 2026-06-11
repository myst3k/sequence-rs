mod helpers;

use futures::TryStreamExt;
use helpers::{client, envelope};
use sequence_rs::model::transaction::{CardTransactionSubtype, CardType};
use sequence_rs::model::transfer::{TransferDirection, TransferStatus};
use sequence_rs::prelude::*;
use sequence_rs::ListCardTransactionsParams;
use serde_json::json;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn page(items: serde_json::Value, p: i64, ps: i64) -> serde_json::Value {
    envelope(
        json!({ "items": items, "pagination": { "page": p, "pageSize": ps, "hasNextPage": false } }),
    )
}

fn one_txn(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "cardId": "card-1",
        "cardType": "DEBIT_CARD",
        "account": { "id": "pod-1", "name": "Spending Account", "type": "POD", "isDeleted": false },
        "direction": "MONEY_OUT",
        "subtype": "PURCHASE",
        "status": "COMPLETE",
        "amountInCents": 4299,
        "description": "Coffee Shop",
        "createdAt": "2026-06-01T12:00:00Z",
        "completedAt": "2026-06-01T12:00:05Z"
    })
}

#[tokio::test]
async fn list_card_transactions_happy_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/card-transactions"))
        .and(query_param("accountId", "pod-1"))
        .and(header("authorization", "Bearer seq_test_key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page(json!([one_txn("t1")]), 1, 10)))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let res = client
        .card_transactions(&"pod-1".to_string(), &ListCardTransactionsParams::default())
        .await
        .unwrap();
    assert_eq!(res.items.len(), 1);
    let t = &res.items[0];
    assert_eq!(t.description, "Coffee Shop");
    assert_eq!(t.amount_in_cents, 4299);
    assert!(matches!(t.card_type, CardType::DebitCard));
    assert!(matches!(t.subtype, CardTransactionSubtype::Purchase));
    assert!(matches!(t.direction, TransferDirection::MoneyOut));
    assert!(matches!(t.status, TransferStatus::Complete));
    assert_eq!(t.account.name, "Spending Account");
}

#[tokio::test]
async fn list_card_transactions_sends_filters() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/card-transactions"))
        .and(query_param("accountId", "pod-1"))
        .and(query_param("cardId", "card-9"))
        .and(query_param("from", "2026-06-01T00:00:00Z"))
        .and(query_param("pageSize", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page(json!([]), 1, 50)))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let params = ListCardTransactionsParams {
        card_id: Some("card-9".to_string()),
        from: Some("2026-06-01T00:00:00Z".to_string()),
        page_size: Some(50),
        ..Default::default()
    };
    client
        .card_transactions(&"pod-1".to_string(), &params)
        .await
        .unwrap();
}

#[tokio::test]
async fn card_transactions_stream_paginates() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/card-transactions"))
        .and(query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page(
            json!([one_txn("a"), one_txn("b")]),
            1,
            2,
        )))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/card-transactions"))
        .and(query_param("page", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page(json!([one_txn("c")]), 2, 2)))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let acct = "pod-1".to_string();
    let params = ListCardTransactionsParams {
        page_size: Some(2),
        ..Default::default()
    };
    let all: Vec<_> = client
        .card_transactions_stream(&acct, &params)
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}
