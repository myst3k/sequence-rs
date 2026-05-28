//! `BaseClient` — one async method per endpoint in `docs/openapi.yaml`.
//! Helpers (`api_get`/`api_post`) handle URL building, auth, the
//! `idempotency-key` / `x-called-reason` headers, and unwrapping the
//! `{ data, requestId }` response envelope.

use std::fmt;

use serde::Serialize;
use serde_json::Value;
use url::Url;

use sequence_rs_http::{BaseHttpClient, Headers, HttpClient, Query};
use sequence_rs_model::account::{Account, AccountId, AccountState, AccountType, AccountsData};
use sequence_rs_model::common::ApiResponse;
use sequence_rs_model::rule::{
    Rule, RuleExecution, RuleExecutionId, RuleExecutionStatus, RuleExecutionsData, RuleId,
    RulesData, TriggerRuleRequest, TriggerRuleResponse, TriggerType,
};
use sequence_rs_model::transfer::{
    CreateTransferRequest, Transfer, TransferDirection, TransferId, TransferOriginFilter,
    TransferStatus, TransfersData,
};

use crate::clients::convert_result;
use crate::{ClientResult, Config, Credentials};

/// Render an enum as its `SCREAMING_SNAKE_CASE` wire string for use in a query
/// parameter (serde emits it quoted; strip the quotes).
fn scream<T: Serialize>(value: &T) -> ClientResult<String> {
    Ok(serde_json::to_string(value)?.trim_matches('"').to_owned())
}

/// Controls how the `accountId` is matched in
/// `GET /accounts/{accountId}/transfers`.
#[derive(Debug, Clone, Copy, Default)]
pub enum AccountRole {
    Source,
    Destination,
    /// The spec's default when the filter is omitted.
    #[default]
    Either,
}

impl AccountRole {
    fn as_query(&self) -> &'static str {
        match self {
            AccountRole::Source => "source",
            AccountRole::Destination => "destination",
            AccountRole::Either => "either",
        }
    }
}

/// Query parameters for `GET /accounts`.
#[derive(Debug, Default, Clone)]
pub struct ListAccountsParams {
    pub account_type: Option<AccountType>,
    pub state: Option<AccountState>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

/// Query parameters for `GET /accounts/{accountId}/transfers`.
#[derive(Debug, Default, Clone)]
pub struct ListAccountTransfersParams {
    pub account_role: Option<AccountRole>,
    pub direction: Option<TransferDirection>,
    pub status: Option<TransferStatus>,
    /// Filter by origin. Uses [`TransferOriginFilter`] (no response-only
    /// `UNKNOWN`), so an invalid filter value can't be constructed.
    pub origin: Option<TransferOriginFilter>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub rule_execution_id: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

/// Query parameters for `GET /rules`.
#[derive(Debug, Default, Clone)]
pub struct ListRulesParams {
    pub source_id: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

/// Query parameters for `GET /rules/{ruleId}/executions`.
#[derive(Debug, Default, Clone)]
pub struct ListRuleExecutionsParams {
    pub status: Option<RuleExecutionStatus>,
    pub trigger_type: Option<TriggerType>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

#[allow(async_fn_in_trait)]
pub trait BaseClient
where
    Self: Send + Sync + Default + Clone + fmt::Debug,
{
    fn get_config(&self) -> &Config;
    fn get_http(&self) -> &HttpClient;
    fn get_creds(&self) -> &Credentials;

    /// Absolute URL for an endpoint, percent-encoding each path segment.
    fn api_url(&self, segments: &[&str]) -> ClientResult<String> {
        let mut url = Url::parse(&self.get_config().api_base_url)?;
        url.path_segments_mut()
            .expect("api_base_url must be a base URL (http/https)")
            .pop_if_empty()
            .extend(segments);
        Ok(url.into())
    }

    /// Auth headers, plus the idempotency key (when given; reused on every retry
    /// by `request()`) and `x-called-reason` (when configured).
    fn build_headers(&self, idempotency_key: Option<&str>) -> Headers {
        let mut headers = self.get_creds().auth_headers();
        if let Some(key) = idempotency_key {
            headers.insert("idempotency-key".to_owned(), key.to_owned());
        }
        if let Some(reason) = &self.get_config().called_reason {
            headers.insert("x-called-reason".to_owned(), reason.clone());
        }
        headers
    }

    #[doc(hidden)]
    #[inline]
    async fn api_get(&self, segments: &[&str], query: &Query) -> ClientResult<String> {
        let url = self.api_url(segments)?;
        let headers = self.build_headers(None);
        Ok(self.get_http().get(&url, Some(&headers), query).await?)
    }

    #[doc(hidden)]
    #[inline]
    async fn api_post(
        &self,
        segments: &[&str],
        payload: &Value,
        idempotency_key: Option<&str>,
    ) -> ClientResult<String> {
        let url = self.api_url(segments)?;
        let headers = self.build_headers(idempotency_key);
        Ok(self.get_http().post(&url, Some(&headers), payload).await?)
    }

    // ----- Accounts -------------------------------------------------------

    /// `GET /accounts` — list accounts. Requires `READ_ACCOUNTS`.
    async fn accounts(&self, params: &ListAccountsParams) -> ClientResult<AccountsData> {
        let mut query: Query = Query::new();
        if let Some(t) = params.account_type {
            query.push(("type".into(), scream(&t)?));
        }
        if let Some(s) = params.state {
            query.push(("state".into(), scream(&s)?));
        }
        if let Some(p) = params.page {
            query.push(("page".into(), p.to_string()));
        }
        if let Some(ps) = params.page_size {
            query.push(("pageSize".into(), ps.to_string()));
        }
        let body = self.api_get(&["accounts"], &query).await?;
        convert_result::<ApiResponse<AccountsData>>(&body).map(|r| r.data)
    }

    /// `GET /accounts/{id}` — fetch a single account.
    async fn account(&self, id: &AccountId) -> ClientResult<Account> {
        let body = self
            .api_get(&["accounts", id.as_str()], &Query::new())
            .await?;
        convert_result::<ApiResponse<Account>>(&body).map(|r| r.data)
    }

    /// `GET /accounts/{accountId}/transfers` — transfer history for an account.
    async fn account_transfers(
        &self,
        account_id: &AccountId,
        params: &ListAccountTransfersParams,
    ) -> ClientResult<TransfersData> {
        let mut query: Query = Query::new();
        if let Some(r) = params.account_role {
            query.push(("accountRole".into(), r.as_query().to_owned()));
        }
        if let Some(d) = params.direction {
            query.push(("direction".into(), scream(&d)?));
        }
        if let Some(s) = params.status {
            query.push(("status".into(), scream(&s)?));
        }
        if let Some(o) = params.origin {
            query.push(("origin".into(), scream(&o)?));
        }
        if let Some(f) = &params.from {
            query.push(("from".into(), f.clone()));
        }
        if let Some(t) = &params.to {
            query.push(("to".into(), t.clone()));
        }
        if let Some(reid) = &params.rule_execution_id {
            query.push(("rule_execution_id".into(), reid.clone()));
        }
        if let Some(p) = params.page {
            query.push(("page".into(), p.to_string()));
        }
        if let Some(ps) = params.page_size {
            query.push(("pageSize".into(), ps.to_string()));
        }
        let body = self
            .api_get(&["accounts", account_id.as_str(), "transfers"], &query)
            .await?;
        convert_result::<ApiResponse<TransfersData>>(&body).map(|r| r.data)
    }

    // ----- Rules ----------------------------------------------------------

    /// `GET /rules` — list rules. Requires `READ_RULES`.
    async fn rules(&self, params: &ListRulesParams) -> ClientResult<RulesData> {
        let mut query: Query = Query::new();
        if let Some(s) = &params.source_id {
            query.push(("sourceId".into(), s.clone()));
        }
        if let Some(p) = params.page {
            query.push(("page".into(), p.to_string()));
        }
        if let Some(ps) = params.page_size {
            query.push(("pageSize".into(), ps.to_string()));
        }
        let body = self.api_get(&["rules"], &query).await?;
        convert_result::<ApiResponse<RulesData>>(&body).map(|r| r.data)
    }

    /// `GET /rules/{id}` — fetch a single rule.
    async fn rule(&self, id: &RuleId) -> ClientResult<Rule> {
        let body = self.api_get(&["rules", id.as_str()], &Query::new()).await?;
        convert_result::<ApiResponse<Rule>>(&body).map(|r| r.data)
    }

    /// `POST /rules/{id}/trigger` — trigger a rule on demand. Requires
    /// `TRIGGER_RULES` on the rule. `idempotency_key` is optional; pass a
    /// stable key (e.g. a deterministic event id) if you want retries of the
    /// *same* logical trigger to be deduplicated server-side for 24h. Passing
    /// `None` sends no key, so each call executes independently.
    async fn trigger_rule(
        &self,
        id: &RuleId,
        request: &TriggerRuleRequest,
        idempotency_key: Option<&str>,
    ) -> ClientResult<TriggerRuleResponse> {
        let payload = serde_json::to_value(request)?;
        let body = self
            .api_post(
                &["rules", id.as_str(), "trigger"],
                &payload,
                idempotency_key,
            )
            .await?;
        convert_result::<ApiResponse<TriggerRuleResponse>>(&body).map(|r| r.data)
    }

    /// `GET /rules/{ruleId}/executions` — execution history for a rule.
    async fn rule_executions(
        &self,
        rule_id: &RuleId,
        params: &ListRuleExecutionsParams,
    ) -> ClientResult<RuleExecutionsData> {
        let mut query: Query = Query::new();
        if let Some(s) = params.status {
            query.push(("status".into(), scream(&s)?));
        }
        if let Some(t) = params.trigger_type {
            query.push(("triggerType".into(), scream(&t)?));
        }
        if let Some(f) = &params.from {
            query.push(("from".into(), f.clone()));
        }
        if let Some(t) = &params.to {
            query.push(("to".into(), t.clone()));
        }
        if let Some(p) = params.page {
            query.push(("page".into(), p.to_string()));
        }
        if let Some(ps) = params.page_size {
            query.push(("pageSize".into(), ps.to_string()));
        }
        let body = self
            .api_get(&["rules", rule_id.as_str(), "executions"], &query)
            .await?;
        convert_result::<ApiResponse<RuleExecutionsData>>(&body).map(|r| r.data)
    }

    /// `GET /rules/{ruleId}/executions/{id}` — single execution.
    async fn rule_execution(
        &self,
        rule_id: &RuleId,
        execution_id: &RuleExecutionId,
    ) -> ClientResult<RuleExecution> {
        let body = self
            .api_get(
                &[
                    "rules",
                    rule_id.as_str(),
                    "executions",
                    execution_id.as_str(),
                ],
                &Query::new(),
            )
            .await?;
        convert_result::<ApiResponse<RuleExecution>>(&body).map(|r| r.data)
    }

    // ----- Transfers ------------------------------------------------------

    /// `POST /transfers` — create a manual transfer. Requires `MANUAL_TRANSFER`
    /// for the {source, target} pair. Idempotency-key is recommended.
    async fn create_transfer(
        &self,
        request: &CreateTransferRequest,
        idempotency_key: Option<&str>,
    ) -> ClientResult<Transfer> {
        // Fail fast on the API's documented input constraints before sending.
        request.validate()?;
        let payload = serde_json::to_value(request)?;
        let body = self
            .api_post(&["transfers"], &payload, idempotency_key)
            .await?;
        convert_result::<ApiResponse<Transfer>>(&body).map(|r| r.data)
    }

    /// `GET /transfers/{id}` — fetch a single transfer by id.
    async fn transfer(&self, id: &TransferId) -> ClientResult<Transfer> {
        let body = self
            .api_get(&["transfers", id.as_str()], &Query::new())
            .await?;
        convert_result::<ApiResponse<Transfer>>(&body).map(|r| r.data)
    }
}
