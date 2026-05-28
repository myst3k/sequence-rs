//! Auto-paginating `*_stream` helpers: yield every item across pages, fetched
//! lazily. The API exposes no total, so a stream stops once a page comes back
//! shorter than the requested size.

use futures::stream::{self, Stream, TryStreamExt};

use sequence_rs_model::account::{AccountId, AccountSummary};
use sequence_rs_model::rule::{RuleExecutionSummary, RuleId, RuleSummary};
use sequence_rs_model::transfer::Transfer;

use crate::clients::base::{
    ListAccountTransfersParams, ListAccountsParams, ListRuleExecutionsParams, ListRulesParams,
};
use crate::clients::BaseClient;
use crate::{ClientError, ClientResult, Sequence};

impl Sequence {
    /// Stream every account across all pages (`GET /accounts`).
    pub fn accounts_stream<'a>(
        &'a self,
        params: &'a ListAccountsParams,
    ) -> impl Stream<Item = ClientResult<AccountSummary>> + 'a {
        let page_size = resolve_page_size(params.page_size, self.config.default_page_size);
        stream::try_unfold(Some(1u32), move |state| async move {
            let Some(page) = state else { return Ok(None) };
            let mut p = params.clone();
            p.page = Some(page);
            p.page_size = Some(page_size);
            let resp = self.accounts(&p).await?;
            let next = next_page(resp.items.len(), page, page_size);
            Ok::<_, ClientError>(Some((stream::iter(resp.items.into_iter().map(Ok)), next)))
        })
        .try_flatten()
    }

    /// Stream every transfer for an account across all pages.
    pub fn account_transfers_stream<'a>(
        &'a self,
        account_id: &'a AccountId,
        params: &'a ListAccountTransfersParams,
    ) -> impl Stream<Item = ClientResult<Transfer>> + 'a {
        let page_size = resolve_page_size(params.page_size, self.config.default_page_size);
        stream::try_unfold(Some(1u32), move |state| async move {
            let Some(page) = state else { return Ok(None) };
            let mut p = params.clone();
            p.page = Some(page);
            p.page_size = Some(page_size);
            let resp = self.account_transfers(account_id, &p).await?;
            let next = next_page(resp.items.len(), page, page_size);
            Ok::<_, ClientError>(Some((stream::iter(resp.items.into_iter().map(Ok)), next)))
        })
        .try_flatten()
    }

    /// Stream every rule across all pages (`GET /rules`).
    pub fn rules_stream<'a>(
        &'a self,
        params: &'a ListRulesParams,
    ) -> impl Stream<Item = ClientResult<RuleSummary>> + 'a {
        let page_size = resolve_page_size(params.page_size, self.config.default_page_size);
        stream::try_unfold(Some(1u32), move |state| async move {
            let Some(page) = state else { return Ok(None) };
            let mut p = params.clone();
            p.page = Some(page);
            p.page_size = Some(page_size);
            let resp = self.rules(&p).await?;
            let next = next_page(resp.items.len(), page, page_size);
            Ok::<_, ClientError>(Some((stream::iter(resp.items.into_iter().map(Ok)), next)))
        })
        .try_flatten()
    }

    /// Stream every execution for a rule across all pages.
    pub fn rule_executions_stream<'a>(
        &'a self,
        rule_id: &'a RuleId,
        params: &'a ListRuleExecutionsParams,
    ) -> impl Stream<Item = ClientResult<RuleExecutionSummary>> + 'a {
        let page_size = resolve_page_size(params.page_size, self.config.default_page_size);
        stream::try_unfold(Some(1u32), move |state| async move {
            let Some(page) = state else { return Ok(None) };
            let mut p = params.clone();
            p.page = Some(page);
            p.page_size = Some(page_size);
            let resp = self.rule_executions(rule_id, &p).await?;
            let next = next_page(resp.items.len(), page, page_size);
            Ok::<_, ClientError>(Some((stream::iter(resp.items.into_iter().map(Ok)), next)))
        })
        .try_flatten()
    }
}

/// The next page to request, or `None` to stop. A short page (fewer items than
/// requested) is the last one, since the API exposes no total count.
fn next_page(items: usize, page: u32, page_size: u32) -> Option<u32> {
    if (items as u32) < page_size {
        None
    } else {
        Some(page + 1)
    }
}

/// Clamped to >=1: a page size of 0 would never satisfy the short-page
/// terminator and would loop forever.
fn resolve_page_size(requested: Option<u32>, default: u32) -> u32 {
    requested.unwrap_or(default).max(1)
}
