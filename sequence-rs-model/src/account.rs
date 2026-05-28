use crate::common::{Cents, Paginated};
use serde::{Deserialize, Serialize};

pub type AccountId = String;

/// Lightweight account shape from `GET /accounts` (list); see [`Account`] for
/// the full detail shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummary {
    pub id: AccountId,
    pub name: String,
    #[serde(rename = "type")]
    pub account_type: AccountType,
    pub description: Option<String>,
    pub external_account_type: Option<ExternalAccountType>,
    pub beneficiary_name: Option<String>,
    pub institution_name: Option<String>,
    pub can_be_source: bool,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// Full `Account` (`GET /accounts/{id}`): [`AccountSummary`] plus account
/// numbers, balance, and savings target.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    #[serde(flatten)]
    pub summary: AccountSummary,
    pub routing_number: Option<String>,
    pub bank_account_number: Option<String>,
    pub balance: Option<Balance>,
    /// Cents; set only for `POD` accounts.
    pub savings_target_in_cents: Option<Cents>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AccountType {
    #[default]
    IncomeSource,
    Pod,
    ExternalAccount,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExternalAccountType {
    Depository,
    Investment,
    Liability,
}

/// State filter for `GET /accounts`; defaults to `ACTIVE` (excludes deleted).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AccountState {
    #[default]
    Active,
    All,
}

/// A reference to an account, used inside rule actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountNode {
    pub id: String,
    #[serde(rename = "type")]
    pub account_type: AccountType,
    pub name: Option<String>,
}

/// Account balance. When it can't be fetched, `error` holds a reason code and
/// the numeric fields are `None`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
    pub balance_in_cents: Option<Cents>,
    pub available_balance_in_cents: Option<Cents>,
    pub last_statement_balance_in_cents: Option<Cents>,
    pub last_statement_date: Option<String>,
    pub next_payment_minimum_in_cents: Option<Cents>,
    pub next_payment_due_date: Option<String>,
    pub balance_last_updated_at: Option<String>,
    pub error: Option<String>,
    pub interest_rate_percentage: Option<f64>,
    pub original_loan_amount_in_cents: Option<Cents>,
}

pub type AccountsData = Paginated<AccountSummary>;
