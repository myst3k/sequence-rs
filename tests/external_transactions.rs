mod helpers;

use helpers::{client, envelope};
use sequence_rs::model::external_transaction::ExternalTransactionStatusFilter;
use sequence_rs::model::transfer::{TransferDirection, TransferStatus};
use sequence_rs::prelude::*;
use sequence_rs::ListExternalTransactionsParams;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn page(items: serde_json::Value, p: i64, ps: i64) -> serde_json::Value {
    envelope(
        json!({ "items": items, "pagination": { "page": p, "pageSize": ps, "hasNextPage": false } }),
    )
}

fn one_ext(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "accountId": "ext-1",
        "amountInCents": 150000,
        "direction": "MONEY_IN",
        "status": "COMPLETE",
        "description": "Payroll",
        "transactionDate": "2026-06-15T00:00:00Z"
    })
}

#[tokio::test]
async fn list_external_transactions_happy_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/external-transactions"))
        .and(query_param("accountIds", "ext-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page(json!([one_ext("x1")]), 1, 10)))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let res = client
        .external_transactions(
            &["ext-1".to_string()],
            &ListExternalTransactionsParams::default(),
        )
        .await
        .unwrap();
    assert_eq!(res.items.len(), 1);
    let t = &res.items[0];
    assert_eq!(t.amount_in_cents, 150000);
    assert_eq!(t.description, "Payroll");
    assert!(matches!(t.direction, TransferDirection::MoneyIn));
    assert!(matches!(t.status, TransferStatus::Complete));
    assert_eq!(t.transaction_date, "2026-06-15T00:00:00Z");
}

#[tokio::test]
async fn list_external_transactions_sends_filters_and_multiple_accounts() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/external-transactions"))
        .and(query_param("accountIds", "ext-1"))
        .and(query_param("direction", "MONEY_OUT"))
        .and(query_param("status", "PENDING"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page(json!([]), 1, 10)))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let params = ListExternalTransactionsParams {
        direction: Some(TransferDirection::MoneyOut),
        status: Some(ExternalTransactionStatusFilter::Pending),
        ..Default::default()
    };
    client
        .external_transactions(&["ext-1".to_string(), "ext-2".to_string()], &params)
        .await
        .unwrap();

    // Both account ids are sent as repeated `accountIds` query params.
    let reqs = server.received_requests().await.unwrap();
    let q: Vec<_> = reqs[0]
        .url
        .query_pairs()
        .filter(|(k, _)| k == "accountIds")
        .map(|(_, v)| v.into_owned())
        .collect();
    assert_eq!(q, vec!["ext-1".to_string(), "ext-2".to_string()]);
}
