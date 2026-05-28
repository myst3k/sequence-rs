use crate::account::AccountNode;
use crate::common::{Cents, Paginated};
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};

pub type RuleId = String;
pub type RuleExecutionId = String;

/// Compact rule shape from `GET /rules` (list); see [`Rule`] for the full shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleSummary {
    pub id: RuleId,
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: RuleStatus,
    /// `false` for rules this API version can't represent; `GET /rules/{id}`
    /// then returns `INVALID_RULE`.
    pub is_supported: bool,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// Full `Rule` (`GET /rules/{id}`), including `trigger` and `steps`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: RuleId,
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: RuleStatus,
    pub trigger: Trigger,
    pub steps: Vec<RuleStep>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RuleStatus {
    Enabled,
    Disabled,
}

// ----- Trigger -------------------------------------------------------------

/// How a rule fires, discriminated on `type`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum Trigger {
    OnFundsTransferred {
        account_id: String,
    },
    Scheduled {
        schedule_type: ScheduleType,
        start_date: String,
        account_id: Option<String>,
    },
    Manual {
        account_id: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ScheduleType {
    OneTime,
    Daily,
    Weekly,
    BiWeekly,
    Monthly,
    EveryOtherWeek,
}

// ----- Steps, actions, conditions ------------------------------------------

/// Steps run in order; the first whose conditions pass executes its actions and
/// the rule stops (first-match-wins). `conditions: None` is a catch-all.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleStep {
    pub conditions: Option<ChainableRuleCondition>,
    pub actions: Vec<RuleAction>,
}

/// A rule action: shared [`RuleActionBase`] plus a type-specific
/// [`RuleActionKind`]. On the wire both flatten into one object with the `type`
/// tag, e.g. `{ "type": "FIXED", "source": {…}, "amountInCents": 5000 }`.
///
/// `Deserialize` is hand-written so parsing doesn't rely on `flatten` (the two
/// halves are each serde-safe alone); serialization stays derived.
#[derive(Debug, Clone, Serialize)]
pub struct RuleAction {
    #[serde(flatten)]
    pub base: RuleActionBase,
    #[serde(flatten)]
    pub kind: RuleActionKind,
}

impl<'de> Deserialize<'de> for RuleAction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Parse the flat object two ways; each half ignores the other's fields.
        let value = serde_json::Value::deserialize(deserializer)?;
        let kind = RuleActionKind::deserialize(&value).map_err(de::Error::custom)?;
        let base = RuleActionBase::deserialize(value).map_err(de::Error::custom)?;
        Ok(RuleAction { base, kind })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleActionBase {
    pub source: AccountNode,
    pub destination: AccountNode,
    /// Actions sharing a `group_index` run as a unit and share one `limit`.
    pub group_index: i32,
    /// Transfer up to the available balance instead of failing on shortfall.
    pub up_to_enabled: bool,
    /// Send as a direct deposit (ACH classified as payroll).
    pub is_direct_deposit: bool,
    pub limit: Option<TransferCap>,
    pub ach_description: Option<String>,
}

/// Type-specific payload of a [`RuleAction`], discriminated on `type`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum RuleActionKind {
    Fixed {
        amount_in_cents: Cents,
    },
    Percentage {
        percentage_value: f32,
        percentage_target: PercentageTarget,
    },
    /// Top the destination up to a target; exactly one target field is set.
    TopUp {
        amount_in_cents: Option<Cents>,
        next_payment_minimum_account: Option<AccountNode>,
        current_balance_account: Option<AccountNode>,
        last_statement_balance_account: Option<AccountNode>,
    },
    RoundDown {
        amount_in_cents: Cents,
    },
    NextPaymentMinimum,
    TotalAmountDue,
    LastStatementBalance,
    PercentageLiabilityBalance {
        percentage_value: f32,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PercentageTarget {
    IncomingAmount,
    SourceAccount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferCap {
    pub period: TransferCapPeriod,
    pub amount_in_cents: Cents,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransferCapPeriod {
    PerTransfer,
    PerWeek,
    PerMonth,
    PerYear,
}

/// A condition tree: exactly one of a leaf `condition`, `any` (OR), or `all`
/// (AND); nesting is supported.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChainableRuleCondition {
    Condition { condition: RuleCondition },
    Any { any: Vec<ChainableRuleCondition> },
    All { all: Vec<ChainableRuleCondition> },
}

/// Compares `fact` against a literal `value` or another `value_fact`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleCondition {
    pub fact: RuleConditionFact,
    pub operator: RuleConditionOperator,
    pub value: Option<f64>,
    pub value_fact: Option<RuleConditionValueFact>,
    pub params: Option<RuleConditionParams>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleConditionParams {
    pub account_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RuleConditionFact {
    TransferAmount,
    Balance,
    Date,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RuleConditionValueFact {
    TransferAmount,
    Balance,
    Date,
    LastDayOfMonth,
    NextPaymentMinimumAmount,
    LastStatementBalance,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RuleConditionOperator {
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
}

// ----- Executions ----------------------------------------------------------

/// Execution shape from `GET /rules/{id}/executions`; see [`RuleExecution`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleExecutionSummary {
    pub id: RuleExecutionId,
    pub rule_id: RuleId,
    pub status: RuleExecutionStatus,
    pub created_at: String,
}

/// Full execution (`GET /rules/{id}/executions/{id}`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleExecution {
    #[serde(flatten)]
    pub summary: RuleExecutionSummary,
    pub trigger_details: TriggerDetails,
    pub step_index_matched: Option<i32>,
    pub conditions_not_met: bool,
    pub transfers_attempted: i32,
    pub transfers_completed: i32,
    pub transfers_failed: i32,
    pub transfers_pending: i32,
    pub transfer_ids: Vec<String>,
    pub error_message: Option<String>,
    pub next_attempt_at: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RuleExecutionStatus {
    #[default]
    Executed,
    Partial,
    InProgress,
    Failed,
}

/// What triggered an execution, discriminated on `type`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum TriggerDetails {
    Manual { amount_in_cents: Option<Cents> },
    SequenceApi { amount_in_cents: Option<Cents> },
    Scheduled { scheduled_time: Option<String> },
    OnFundsTransferred { amount_in_cents: Option<Cents> },
    RemoteApi,
}

/// `triggerType` filter values for `listRuleExecutions` (mirrors
/// [`TriggerDetails`]'s `type`).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TriggerType {
    #[default]
    Manual,
    SequenceApi,
    Scheduled,
    OnFundsTransferred,
    RemoteApi,
}

// ----- Trigger request/response --------------------------------------------

/// Body for `POST /rules/{id}/trigger`. `execute_amount` injects a synthetic
/// `TRANSFER_AMOUNT` fact; it does not override fixed-amount transfers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerRuleRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execute_amount: Option<i64>,
}

/// 202 `data` of `POST /rules/{id}/trigger`; poll `execution_id` for the outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerRuleResponse {
    pub execution_id: String,
}

pub type RulesData = Paginated<RuleSummary>;
pub type RuleExecutionsData = Paginated<RuleExecutionSummary>;
