mod helpers;

use helpers::{client, error_envelope};
use rstest::rstest;
use sequence_rs::prelude::*;
use sequence_rs::{ClientError, ListAccountsParams};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[rstest]
#[case(401, "UNAUTHORIZED")]
#[case(403, "ACCESS_DENIED")]
#[case(404, "ACCOUNT_NOT_FOUND")]
#[case(422, "VALIDATION_ERROR")]
#[case(429, "RATE_LIMIT_EXCEEDED")]
#[case(500, "UNEXPECTED_ERROR")]
#[tokio::test]
async fn error_envelope_maps_to_typed_api_error(#[case] status: u16, #[case] code: &str) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .respond_with(ResponseTemplate::new(status).set_body_json(error_envelope(code, "boom")))
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let err = client
        .accounts(&ListAccountsParams::default())
        .await
        .unwrap_err();
    match err {
        ClientError::Api(api) => {
            assert_eq!(api.code, code);
            assert_eq!(api.message, "boom");
        }
        other => panic!("expected ClientError::Api, got {other:?}"),
    }
}

#[tokio::test]
async fn non_envelope_body_falls_back_to_http_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .respond_with(ResponseTemplate::new(502).set_body_string("<html>bad gateway</html>"))
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let err = client
        .accounts(&ListAccountsParams::default())
        .await
        .unwrap_err();
    assert!(
        matches!(err, ClientError::Http(_)),
        "expected ClientError::Http for non-envelope body, got {err:?}"
    );
}
