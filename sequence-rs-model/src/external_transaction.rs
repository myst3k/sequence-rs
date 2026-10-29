use crate::account::AccountId;
use crate::common::{Cents, Paginated};
use crate::transfer::{TransferDirection, TransferStatus};
use serde::{Deserialize, Serialize};

pub type ExternalTransactionId = String;

/// An external (bank/ACH) transaction (`GET /external-transactions`) on a linked
/// `EXTERNAL_ACCOUNT` — money moving in or out of an account outside Sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalTransaction {
    pub id: ExternalTransactionId,
    pub account_id: AccountId,
    /// Always a positive integer.
    pub amount_in_cents: Cents,
    pub direction: TransferDirection,
    pub status: TransferStatus,
    pub description: String,
    /// When the transaction occurred.
    pub transaction_date: String,
}

/// `status` filter for `GET /external-transactions`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExternalTransactionStatusFilter {
    Pending,
    Complete,
}

pub type ExternalTransactionsData = Paginated<ExternalTransaction>;
