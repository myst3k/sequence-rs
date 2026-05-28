# sequence-rs

[![CI](https://github.com/myst3k/sequence-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/myst3k/sequence-rs/actions/workflows/ci.yml)

Rust client for the [Sequence Platform API](https://app.getsequence.io/api/platform/) —
typed models, auto-pagination, client-side rate limiting, and retries with `Retry-After`.

> **Unofficial.** A community project, not affiliated with or endorsed by Sequence.

## Install

```toml
[dependencies]
sequence-rs = { git = "https://github.com/myst3k/sequence-rs" }
```

Requires a Tokio runtime and a recent stable Rust (uses async fn in traits, Rust ≥ 1.75).

## Quick start

```bash
cp .env.example .env  # add your SEQUENCE_API_KEY
cargo run --example playground --features env-file
```

```rust
use sequence_rs::prelude::*;
use sequence_rs::{Credentials, ListAccountsParams, Sequence};
use futures::TryStreamExt;

let client = Sequence::new(Credentials::from_env().expect("SEQUENCE_API_KEY"));

let page = client.accounts(&ListAccountsParams::default()).await?;        // one page
let all: Vec<_> = client.accounts_stream(&ListAccountsParams::default())  // every page, lazily
    .try_collect().await?;
```

Auth is a bearer token from `SEQUENCE_API_KEY`; permissions are the `ApiKeyAuth` scheme in
the [API docs](https://app.getsequence.io/api/platform/).

## Endpoints

| Method | Endpoint | Returns |
|---|---|---|
| `accounts` / `accounts_stream` | `GET /accounts` | `AccountSummary` |
| `account` | `GET /accounts/{id}` | full `Account` |
| `account_transfers` / `account_transfers_stream` | `GET /accounts/{id}/transfers` | `Transfer` |
| `rules` / `rules_stream` | `GET /rules` | `RuleSummary` |
| `rule` | `GET /rules/{id}` | full `Rule` |
| `trigger_rule` | `POST /rules/{id}/trigger` | `TriggerRuleResponse` |
| `rule_executions` / `rule_executions_stream` | `GET /rules/{id}/executions` | `RuleExecutionSummary` |
| `rule_execution` | `GET /rules/{id}/executions/{id}` | full `RuleExecution` |
| `create_transfer` | `POST /transfers` | `Transfer` |
| `transfer` | `GET /transfers/{id}` | `Transfer` |

## Behavior

- **Envelopes** are unwrapped automatically; error bodies become `ClientError::Api`, anything else `ClientError::Http`.
- **Idempotency** — `trigger_rule`/`create_transfer` take `Option<&str>`; a supplied key rides every retry (24h server dedup), `None` sends none. Pass one to make a mutating call retry-safe.
- **Rate limiting** — token bucket, default 100 req/min (the server ceiling), burstable; `Config.rate_limit = None` disables.
- **Retries** — `429`/`5xx`/network blips with full-jitter backoff honouring `Retry-After`, bounded by `max_attempts` and an optional `max_elapsed_time` deadline; `Config::default().no_retries()` turns them off.
- **Validation** — `create_transfer` enforces the spec's input rules before sending.
- **Pagination** — the API has no `total`, so `*_stream` stops on a short page.

## Configuration

```rust
use sequence_rs::{Config, Credentials, RateLimit, Sequence};

let client = Sequence::with_config(
    Credentials::from_env().unwrap(),
    Config {
        called_reason: Some("my-service/nightly-sweep".into()), // x-called-reason
        rate_limit: Some(RateLimit { requests_per_minute: 60 }),
        ..Default::default()
    },
);
```

Point `api_base_url` at the spec's `dev`/`staging` servers for non-prod testing.

## Development

Crates: `sequence-rs` (client, config, streams) · `sequence-rs-http` (reqwest wrapper, retries,
rate limiting) · `sequence-rs-model` (request/response types).

Fetch the spec (gitignored) with `python3 scripts/fetch_openapi.py`. To add an endpoint: extend
the model, add the `BaseClient` method, and cover it with a roundtrip + wiremock test. See
`IMPLEMENTATION_PLAN.md` for design notes.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
