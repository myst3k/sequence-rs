use crate::account::AccountId;
use crate::common::{Cents, Paginated};
use crate::error::{ModelError, ModelResult};
use crate::rule::{RuleExecutionId, RuleId};
use serde::{Deserialize, Serialize};

pub type TransferId = String;

/// A single money movement (rule-triggered, user-initiated, incoming, or
/// external). Card transactions are excluded.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transfer {
    pub id: TransferId,
    pub amount_in_cents: Cents,
    pub direction: TransferDirection,
    pub origin: TransferOrigin,
    pub source: Option<TransferAccountRef>,
    pub destination: Option<TransferAccountRef>,
    pub status: TransferStatus,
    pub rule_id: Option<RuleId>,
    pub rule_execution_id: Option<RuleExecutionId>,
    pub error_code: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

/// A transfer participant. `id` and `is_deleted` are `None` for `EXTERNAL_ENTITY`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferAccountRef {
    pub id: Option<String>,
    pub name: String,
    #[serde(rename = "type")]
    pub participant_type: TransferParticipantType,
    pub is_deleted: Option<bool>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransferParticipantType {
    IncomeSource,
    Pod,
    ExternalAccount,
    /// External participant with no Sequence record (ATM, external pull).
    ExternalEntity,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransferDirection {
    #[default]
    MoneyIn,
    MoneyOut,
    Internal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransferOrigin {
    DirectDeposit,
    CheckDeposit,
    Cashback,
    UserPull,
    Rule,
    User,
    ExternalPull,
    Unknown,
}

/// `origin` filter values — [`TransferOrigin`] minus the response-only `UNKNOWN`.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransferOriginFilter {
    #[default]
    DirectDeposit,
    CheckDeposit,
    Cashback,
    UserPull,
    Rule,
    User,
    ExternalPull,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransferStatus {
    #[default]
    PendingApproval,
    Processing,
    Pending,
    Complete,
    Incomplete,
    Error,
    Cancelled,
}

/// Body for `POST /transfers`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTransferRequest {
    pub source_account_id: AccountId,
    pub destination_account_id: AccountId,
    pub amount_in_cents: Cents,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl CreateTransferRequest {
    /// Check the API's input rules: `amount_in_cents >= 100`, and an optional
    /// `description` of at most 10 letters/digits/spaces.
    pub fn validate(&self) -> ModelResult<()> {
        if self.amount_in_cents < 100 {
            return Err(ModelError::Validation(format!(
                "amount_in_cents must be at least 100 ($1.00); got {}",
                self.amount_in_cents
            )));
        }
        if let Some(desc) = &self.description {
            let len = desc.chars().count();
            if len > 10 {
                return Err(ModelError::Validation(format!(
                    "description must be at most 10 characters; got {len}"
                )));
            }
            if !desc.chars().all(|c| c.is_ascii_alphanumeric() || c == ' ') {
                return Err(ModelError::Validation(
                    "description may contain only letters, digits, and spaces".to_string(),
                ));
            }
        }
        Ok(())
    }
}

pub type TransfersData = Paginated<Transfer>;
