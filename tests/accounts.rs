mod helpers;

use futures::TryStreamExt;
use helpers::{client, envelope};
use sequence_rs::model::account::AccountType;
use sequence_rs::prelude::*;
use sequence_rs::{Config, Credentials, ListAccountsParams, Sequence};
use serde_json::json;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn accounts_page(items: serde_json::Value, page: i64, page_size: i64) -> serde_json::Value {
    envelope(json!({ "items": items, "pagination": { "page": page, "pageSize": page_size } }))
}

fn one_account(id: &str) -> serde_json::Value {
    json!({
        "id": id, "name": "Emergency Fund", "type": "POD",
        "description": null, "externalAccountType": null, "beneficiaryName": "John Smith",
        "institutionName": null, "canBeSource": true, "deletedAt": null,
        "createdAt": "2024-02-01T09:00:00Z", "updatedAt": "2024-03-10T14:30:00Z"
    })
}

#[tokio::test]
async fn list_accounts_happy_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .and(header("authorization", "Bearer seq_test_key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(accounts_page(
            json!([one_account("c2cb3499-2491-4185-a6f5-1a3d281b875a")]),
            1,
            10,
        )))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let page = client
        .accounts(&ListAccountsParams::default())
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].name, "Emergency Fund");
    assert!(matches!(page.items[0].account_type, AccountType::Pod));
}

#[tokio::test]
async fn list_accounts_sends_filter_query_params() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .and(query_param("type", "POD"))
        .and(query_param("state", "ALL"))
        .and(query_param("pageSize", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(accounts_page(json!([]), 1, 50)))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let params = ListAccountsParams {
        account_type: Some(AccountType::Pod),
        state: Some(sequence_rs::model::account::AccountState::All),
        page_size: Some(50),
        ..Default::default()
    };
    client.accounts(&params).await.unwrap();
}

#[tokio::test]
async fn get_single_account_returns_full_shape() {
    let server = MockServer::start().await;
    let mut full = one_account("c2cb3499-2491-4185-a6f5-1a3d281b875a");
    full["routingNumber"] = json!("011401533");
    full["bankAccountNumber"] = json!("1111222233334892");
    full["savingsTargetInCents"] = json!(500000);
    full["balance"] = json!({
        "balanceInCents": 250000, "availableBalanceInCents": 250000,
        "lastStatementBalanceInCents": null, "lastStatementDate": null,
        "nextPaymentMinimumInCents": null, "nextPaymentDueDate": null,
        "balanceLastUpdatedAt": "2024-03-10T14:30:00Z", "error": null,
        "interestRatePercentage": null, "originalLoanAmountInCents": null
    });
    Mock::given(method("GET"))
        .and(path("/accounts/c2cb3499-2491-4185-a6f5-1a3d281b875a"))
        .respond_with(ResponseTemplate::new(200).set_body_json(envelope(full)))
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let account = client
        .account(&"c2cb3499-2491-4185-a6f5-1a3d281b875a".to_string())
        .await
        .unwrap();
    assert_eq!(account.summary.name, "Emergency Fund");
    assert_eq!(account.routing_number.as_deref(), Some("011401533"));
    assert_eq!(account.savings_target_in_cents, Some(500000));
    assert_eq!(account.balance.unwrap().balance_in_cents, Some(250000));
}

#[tokio::test]
async fn path_segments_are_percent_encoded() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(envelope(one_account("x"))))
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let _ = client.account(&"a b/c".to_string()).await;

    let req = &server.received_requests().await.unwrap()[0];
    assert_eq!(req.url.path(), "/accounts/a%20b%2Fc");
}

#[tokio::test]
async fn called_reason_header_is_sent() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .and(header("x-called-reason", "test/agent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(accounts_page(json!([]), 1, 10)))
        .expect(1)
        .mount(&server)
        .await;

    let client = Sequence::with_config(
        Credentials::new("seq_test_key"),
        Config {
            api_base_url: format!("{}/", server.uri()),
            rate_limit: None,
            called_reason: Some("test/agent".to_string()),
            ..Default::default()
        },
    );
    client
        .accounts(&ListAccountsParams::default())
        .await
        .unwrap();
}

#[tokio::test]
async fn accounts_stream_stops_on_short_page() {
    let server = MockServer::start().await;
    // Page 1: a full page (pageSize=2) → the stream must request page 2.
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .and(query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(accounts_page(
            json!([one_account("a1"), one_account("a2")]),
            1,
            2,
        )))
        .expect(1)
        .mount(&server)
        .await;
    // Page 2: a short page (1 < 2) → terminator; no page 3 is requested.
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .and(query_param("page", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(accounts_page(
            json!([one_account("a3")]),
            2,
            2,
        )))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let params = ListAccountsParams {
        page_size: Some(2),
        ..Default::default()
    };
    let all: Vec<_> = client
        .accounts_stream(&params)
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    assert_eq!(all.len(), 3);
    // Exactly two requests (page 1 + page 2), proving the terminator fired.
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn accounts_stream_clamps_zero_page_size() {
    // A page size of 0 would otherwise never satisfy `items < page_size` and
    // loop forever. The stream must clamp it and terminate; the timeout guards
    // against a regression by failing instead of hanging.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(accounts_page(json!([]), 1, 1)))
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let params = ListAccountsParams {
        page_size: Some(0),
        ..Default::default()
    };
    let collected = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.accounts_stream(&params).try_collect::<Vec<_>>(),
    )
    .await
    .expect("stream must terminate when page_size is clamped to >= 1")
    .unwrap();
    assert!(collected.is_empty());
}
