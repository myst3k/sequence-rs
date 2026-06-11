use crate::common::Paginated;
use serde::{Deserialize, Serialize};

pub type AuditLogId = String;

/// One API audit-log entry (`GET /audit-logs`) — a record of a single platform
/// API call made with your key, including its outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogEntry {
    pub id: AuditLogId,
    pub created_at: String,
    pub api_key_id: String,
    pub api_key_name: String,
    /// HTTP request path, e.g. `/platform/v1/accounts`.
    pub path: String,
    /// Semantic action; `None` when the request didn't map to a known action.
    pub action: Option<SequenceApiAction>,
    pub request_id: String,
    pub outcome: AuditOutcome,
    /// Sequence error code on failure, e.g. `NOT_FOUND`, `ACCESS_DENIED`.
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuditOutcome {
    Success,
    Failure,
}

/// The semantic action recorded for an audited API call; also the `action`
/// filter for `GET /audit-logs`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SequenceApiAction {
    GetAccount,
    ListAccounts,
    ListTransfers,
    GetTransfer,
    CreateTransfer,
    TriggerRule,
    ListRules,
    GetRule,
    ListRuleExecutions,
    GetRuleExecution,
    ListAuditLogs,
}

pub type AuditLogData = Paginated<AuditLogEntry>;
