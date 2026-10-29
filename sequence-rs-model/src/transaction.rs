use crate::common::{Cents, Paginated};
use crate::transfer::{TransferAccountRef, TransferDirection, TransferStatus};
use serde::{Deserialize, Serialize};

pub type TransactionId = String;

/// A card transaction (`GET /card-transactions`) — a debit/omni-card purchase or
/// refund. These are NOT in [`crate::transfer::Transfer`], which excludes card
/// activity; this is the only way to see card spend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub id: TransactionId,
    pub card_id: String,
    pub card_type: CardType,
    pub account: TransferAccountRef,
    pub direction: TransferDirection,
    pub subtype: CardTransactionSubtype,
    pub status: TransferStatus,
    /// Always a positive integer.
    pub amount_in_cents: Cents,
    /// Human-readable, typically the merchant name; may be empty.
    pub description: String,
    pub created_at: String,
    pub completed_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CardType {
    DebitCard,
    OmniCard,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CardTransactionSubtype {
    Purchase,
    Refund,
    /// Incoming push-to-card payout (`MONEY_IN`); income, not a refund.
    Payout,
    /// A subtype added after this crate version.
    #[serde(other)]
    Unknown,
}

pub type TransactionsData = Paginated<Transaction>;
