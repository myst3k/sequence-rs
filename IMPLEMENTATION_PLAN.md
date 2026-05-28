# sequence-rs — status & design notes

This was a scaffold; it is now a working client. This doc records what's
implemented, the design decisions behind it, and the handful of nice-to-haves
that remain. Shapes follow the spec, fetched into `docs/openapi.yaml` (gitignored)
via `scripts/fetch_openapi.py`.

## Crates

| Crate | Purpose |
|---|---|
| `sequence-rs` (root) | `Sequence` client, `Credentials`, `Config`, `ClientError`, `prelude::BaseClient`, per-endpoint `List*Params`, and the `*_stream` auto-paginators. |
| `sequence-rs-http` | `reqwest` wrapper behind `BaseHttpClient`, plus the retry loop, client-side throttle (`RetryPolicy`/`RateLimit`), and request tracing. Non-2xx → typed `HttpError::Status`. |
| `sequence-rs-model` | Typed request/response types per resource, `common::{ApiResponse<T>, Paginated<T>, Pagination, Cents}`, `error::{ApiError, ApiErrorEnvelope}`. |

## What's implemented

- **All 10 endpoints**, each returning fully-typed models that match the spec's
  schemas (list endpoints return the `*Summary` shape; detail endpoints return the
  full shape).
- **Discriminated unions** modelled with serde: `Trigger`, `RuleAction`
  (8 variants over a shared `RuleActionBase`, via flattened `{ base, kind }`),
  `TriggerDetails`, and the recursive `ChainableRuleCondition` (`condition`/`any`/`all`).
- **Bearer auth** via `Credentials::from_env()` (`SEQUENCE_API_KEY`) or the builder.
- **Response/error envelopes** unwrapped automatically; error bodies lifted to `ClientError::Api`.
- **Idempotency**: caller-supplied keys on `trigger_rule`/`create_transfer`; nothing fabricated.
- **`x-called-reason`** sent from `Config.called_reason`.
- **Client-side rate limiting** (default 100/min, `None` to disable) — token-bucket throttle:
  bursts up to a minute's allowance, then holds the sustained rate.
- **Client-side validation** of `CreateTransferRequest` (min amount, ACH description rules),
  failing fast before a request is sent.
- **Retries** with full-jitter exponential backoff on transient failures (`429` honouring
  `Retry-After`, `500`, `502/503/504`, network timeout/connect errors). The retry decision is
  independent of the idempotency key: a supplied key is re-sent unchanged on every attempt,
  and making a mutating call retry-safe is the caller's choice (pass a key), not something the
  client decides by refusing to retry. Bounded by `max_attempts` and an optional
  `max_elapsed_time` hard deadline (cancels the in-flight request on expiry);
  `Config::no_retries()` turns retries off entirely.
- **Auto-pagination** via `*_stream` helpers; terminator is a short page (no `total` in the spec).
- **Owned, ordered query params** (`Vec<(String, String)>`) — no `Box::leak`.
- **Request tracing** (`tracing` debug spans) in the HTTP layer.

## Tests

- `sequence-rs-model/tests/roundtrip.rs` — parses spec examples into the unions and
  asserts the flattened `RuleAction` re-serializes to a flat object.
- `sequence-rs-http` unit tests — backoff math (exponential, capped, `Retry-After`, jitter).
- `tests/{accounts,rules,errors,retry}.rs` — wiremock matrix: happy paths, query params,
  `x-called-reason`, idempotency-header presence/absence, the `[401,403,404,422,429,500]`
  error matrix, non-envelope fallback, retry counts, and throttle pacing.

Gates: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`,
`cargo test --all` — all clean.

## Open questions — resolved by the spec

Every question the original scaffold flagged turned out to be answered by the vendored spec:

- **Trigger 202 shape** → `{ executionId: uuid }` (now `TriggerRuleResponse`).
- **`429` `Retry-After`** → always present, integer seconds (parsed and honoured).
- **`idempotency-key`** → optional everywhere; 24h dedup window.
- **`MANUAL_TRANSFER` scope** → per `{source, target}` pair.
- **`listRuleExecutions.triggerType`** → same value set as `TriggerDetails.type`
  (now the typed `TriggerType` enum).

## Remaining nice-to-haves

- **Publishing to crates.io.** Deferred by choice. To do it later: add `repository`
  (and optionally `homepage`/`keywords`/`categories`) to each `Cargo.toml`, write a
  CHANGELOG, then `cargo publish` in order: `sequence-rs-http`, `sequence-rs-model`,
  `sequence-rs`. Consume via git/path dependency until then.
- **CI**: a fmt/clippy/test workflow once the repo is on GitHub.

## Design notes

- **Workspace, not one crate** — consumers of just the types (a webhook ingester, a
  server emitting Sequence-shaped JSON) can depend on `sequence-rs-model` without
  `reqwest`/`tokio`.
- **`BaseClient` is a trait** so tests can supply a mock with the same method surface;
  only `get_config`/`get_http`/`get_creds` are implemented per backend.
- **Retry/rate-limit types live in `sequence-rs-http`** (re-exported from the root) so
  the HTTP client can own and apply them without a dependency cycle.
- **`RuleAction` is `{ base, kind }`** where `base` is a plain struct and `kind` a
  standalone internally-tagged enum. Serialization is derived (the two `#[serde(flatten)]`
  attributes merge them into one flat object); deserialization is **hand-written** as a
  two-pass split over a buffered `serde_json::Value`, so parsing never relies on `flatten`'s
  buffering. Both halves are serde-safe constructs on their own. All 8 variants, the nullable
  `TOP_UP` targets, integer→float coercion, and the flat re-serialization are covered by
  `roundtrip.rs`.
- **Bearer token**: the spec's `ApiKeyAuth` scheme is literally `{ type: http, scheme: bearer }`.
- **`Cargo.lock` is gitignored** — this is a library; apps resolve their own.

## Pointers

- OpenAPI spec: `scripts/fetch_openapi.py` → `docs/openapi.yaml` (gitignored)
- Online docs: <https://app.getsequence.io/api/platform/>
- Permission catalogue: `docs/openapi.yaml` → `components.securitySchemes.ApiKeyAuth.description`
