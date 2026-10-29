//! Auto-paginating `*_stream` helpers: yield every item across pages, fetched
//! lazily, starting at `params.page` (default 1) and following `hasNextPage`.

use futures::stream::{self, Stream, TryStreamExt};

use sequence_rs_model::account::{AccountId, AccountSummary};
use sequence_rs_model::audit_log::AuditLogEntry;
use sequence_rs_model::common::Pagination;
use sequence_rs_model::external_transaction::ExternalTransaction;
use sequence_rs_model::rule::{RuleExecutionSummary, RuleId, RuleSummary};
use sequence_rs_model::transaction::Transaction;
use sequence_rs_model::transfer::Transfer;

use crate::clients::base::{
    ListAccountTransfersParams, ListAccountsParams, ListAuditLogParams, ListCardTransactionsParams,
    ListExternalTransactionsParams, ListRuleExecutionsParams, ListRulesParams,
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
        stream::try_unfold(Some(first_page(params.page)), move |state| async move {
            let Some(page) = state else { return Ok(None) };
            let mut p = params.clone();
            p.page = Some(page);
            p.page_size = Some(page_size);
            let resp = self.accounts(&p).await?;
            let next = next_page(&resp.pagination, resp.items.len(), page, page_size);
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
        stream::try_unfold(Some(first_page(params.page)), move |state| async move {
            let Some(page) = state else { return Ok(None) };
            let mut p = params.clone();
            p.page = Some(page);
            p.page_size = Some(page_size);
            let resp = self.account_transfers(account_id, &p).await?;
            let next = next_page(&resp.pagination, resp.items.len(), page, page_size);
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
        stream::try_unfold(Some(first_page(params.page)), move |state| async move {
            let Some(page) = state else { return Ok(None) };
            let mut p = params.clone();
            p.page = Some(page);
            p.page_size = Some(page_size);
            let resp = self.rules(&p).await?;
            let next = next_page(&resp.pagination, resp.items.len(), page, page_size);
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
        stream::try_unfold(Some(first_page(params.page)), move |state| async move {
            let Some(page) = state else { return Ok(None) };
            let mut p = params.clone();
            p.page = Some(page);
            p.page_size = Some(page_size);
            let resp = self.rule_executions(rule_id, &p).await?;
            let next = next_page(&resp.pagination, resp.items.len(), page, page_size);
            Ok::<_, ClientError>(Some((stream::iter(resp.items.into_iter().map(Ok)), next)))
        })
        .try_flatten()
    }

    /// Stream every card transaction for an account across all pages.
    pub fn card_transactions_stream<'a>(
        &'a self,
        account_id: &'a AccountId,
        params: &'a ListCardTransactionsParams,
    ) -> impl Stream<Item = ClientResult<Transaction>> + 'a {
        let page_size = resolve_page_size(params.page_size, self.config.default_page_size);
        stream::try_unfold(Some(first_page(params.page)), move |state| async move {
            let Some(page) = state else { return Ok(None) };
            let mut p = params.clone();
            p.page = Some(page);
            p.page_size = Some(page_size);
            let resp = self.card_transactions(account_id, &p).await?;
            let next = next_page(&resp.pagination, resp.items.len(), page, page_size);
            Ok::<_, ClientError>(Some((stream::iter(resp.items.into_iter().map(Ok)), next)))
        })
        .try_flatten()
    }

    /// Stream every external transaction for the given accounts across all pages.
    pub fn external_transactions_stream<'a>(
        &'a self,
        account_ids: &'a [AccountId],
        params: &'a ListExternalTransactionsParams,
    ) -> impl Stream<Item = ClientResult<ExternalTransaction>> + 'a {
        let page_size = resolve_page_size(params.page_size, self.config.default_page_size);
        stream::try_unfold(Some(first_page(params.page)), move |state| async move {
            let Some(page) = state else { return Ok(None) };
            let mut p = params.clone();
            p.page = Some(page);
            p.page_size = Some(page_size);
            let resp = self.external_transactions(account_ids, &p).await?;
            let next = next_page(&resp.pagination, resp.items.len(), page, page_size);
            Ok::<_, ClientError>(Some((stream::iter(resp.items.into_iter().map(Ok)), next)))
        })
        .try_flatten()
    }

    /// Stream every audit-log entry across all pages (`GET /audit-logs`).
    pub fn audit_logs_stream<'a>(
        &'a self,
        params: &'a ListAuditLogParams,
    ) -> impl Stream<Item = ClientResult<AuditLogEntry>> + 'a {
        let page_size = resolve_page_size(params.page_size, self.config.default_page_size);
        stream::try_unfold(Some(first_page(params.page)), move |state| async move {
            let Some(page) = state else { return Ok(None) };
            let mut p = params.clone();
            p.page = Some(page);
            p.page_size = Some(page_size);
            let resp = self.audit_logs(&p).await?;
            let next = next_page(&resp.pagination, resp.items.len(), page, page_size);
            Ok::<_, ClientError>(Some((stream::iter(resp.items.into_iter().map(Ok)), next)))
        })
        .try_flatten()
    }
}

/// The next page to request, or `None` to stop. `hasNextPage` is authoritative;
/// a full page also continues, for responses that omit the flag. An empty page
/// always stops, so a stuck flag can't loop forever.
fn next_page(meta: &Pagination, items: usize, page: u32, page_size: u32) -> Option<u32> {
    if items > 0 && (meta.has_next_page || items as u32 >= page_size) {
        Some(page + 1)
    } else {
        None
    }
}

fn first_page(requested: Option<u32>) -> u32 {
    requested.unwrap_or(1).max(1)
}

/// The API caps `pageSize` at 100. Asking for more would return 100 items, which
/// would look like a short page.
const MAX_PAGE_SIZE: u32 = 100;

fn resolve_page_size(requested: Option<u32>, default: u32) -> u32 {
    requested.unwrap_or(default).clamp(1, MAX_PAGE_SIZE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(has_next_page: bool) -> Pagination {
        Pagination {
            page: 1,
            page_size: 10,
            has_next_page,
        }
    }

    #[test]
    fn next_page_trusts_has_next_page_on_a_short_page() {
        assert_eq!(next_page(&meta(true), 3, 2, 10), Some(3));
    }

    #[test]
    fn next_page_stops_on_short_page_without_flag() {
        assert_eq!(next_page(&meta(false), 3, 2, 10), None);
    }

    #[test]
    fn next_page_continues_on_full_page_without_flag() {
        assert_eq!(next_page(&meta(false), 10, 1, 10), Some(2));
    }

    #[test]
    fn next_page_stops_on_empty_page_even_if_flagged() {
        assert_eq!(next_page(&meta(true), 0, 4, 10), None);
    }

    #[test]
    fn page_size_is_clamped_to_api_range() {
        assert_eq!(resolve_page_size(Some(500), 10), 100);
        assert_eq!(resolve_page_size(Some(0), 10), 1);
        assert_eq!(resolve_page_size(None, 25), 25);
    }
}
