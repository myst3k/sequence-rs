//! Smoke-test playground. Requires a real API key.
//!
//! Run with:
//!
//! ```bash
//! cp .env.example .env  # then edit
//! cargo run --example playground --features env-file
//! ```

use futures::TryStreamExt;
use sequence_rs::prelude::*;
use sequence_rs::{Config, Credentials, ListAccountsParams, Sequence};
use tracing::info;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();

    let creds = Credentials::from_env().expect("SEQUENCE_API_KEY missing");
    // Default config: production base URL, client-side throttling at 100 req/min,
    // and retries on 429 / 5xx with `Retry-After` honoured.
    let client = Sequence::with_config(
        creds,
        Config {
            called_reason: Some("playground/smoke-test".to_string()),
            ..Default::default()
        },
    );

    info!("Listing accounts (single page)…");
    let page = client.accounts(&ListAccountsParams::default()).await?;
    for a in &page.items {
        println!("{}  {}  ({:?})", a.id, a.name, a.account_type);
    }

    info!("Streaming every account across all pages…");
    let all: Vec<_> = client
        .accounts_stream(&ListAccountsParams::default())
        .try_collect()
        .await?;
    println!("{} accounts total", all.len());

    // Fetch the full detail (balance, account numbers) for the first account.
    if let Some(first) = all.first() {
        let full = client.account(&first.id).await?;
        if let Some(balance) = full.balance {
            println!("balance: {:?} cents", balance.balance_in_cents);
        }
    }

    // Other endpoints — uncomment as you exercise them.
    //
    // let rules = client.rules(&Default::default()).await?;
    // for r in &rules.items { println!("rule {} ({:?})", r.id, r.status); }
    //
    // use sequence_rs::model::rule::TriggerRuleRequest;
    // let resp = client
    //     .trigger_rule(&rules.items[0].id, &TriggerRuleRequest::default(), None)
    //     .await?;
    // println!("triggered execution {}", resp.execution_id);

    Ok(())
}
