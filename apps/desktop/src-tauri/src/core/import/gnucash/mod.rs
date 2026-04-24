//! GnuCash SQLite book importer.
//!
//! Three phases: reader → mapper → committer, with a reconciler that runs
//! after commit to prove balances match the source book. See
//! `docs/superpowers/specs/2026-04-24-gnucash-import-design.md` for the
//! architectural rationale.

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod committer;
pub mod mapper;
pub mod reader;
pub mod reconcile;

// ── Reader output ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GnuCashBook {
    pub book_guid: String,
    pub commodities: Vec<GncCommodity>,
    pub accounts: Vec<GncAccount>,
    pub transactions: Vec<GncTransaction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GncCommodity {
    pub guid: String,
    pub namespace: String, // "CURRENCY" | other (stocks, funds)
    pub mnemonic: String,  // "USD" | "EUR" | ticker
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GncAccount {
    pub guid: String,
    pub parent_guid: Option<String>,
    pub name: String,
    pub full_name: String,
    pub gnc_type: GncAccountType,
    pub commodity_guid: String,
    pub placeholder: bool,
    pub hidden: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum GncAccountType {
    Bank,
    Cash,
    Asset,
    Stock,
    Mutual,
    Receivable,
    Credit,
    Liability,
    Payable,
    Income,
    Expense,
    Equity,
    Root,
    Trading,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GncTransaction {
    pub guid: String,
    pub post_date: i64,  // unix ms, UTC midnight of local date
    pub enter_date: i64,
    pub description: String,
    pub splits: Vec<GncSplit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GncSplit {
    pub guid: String,
    pub account_guid: String,
    pub amount_cents: i64, // signed; positive → debit, negative → credit
    pub memo: String,
    pub reconcile_state: char, // 'n' | 'c' | 'y'
}

// ── Preview returned by the read command ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GnuCashPreview {
    pub book_guid: String,
    pub account_count: u32,
    pub transaction_count: u32,
    pub non_usd_accounts: Vec<String>, // full_names; non-empty → abort
}

// ── Mapper output (committer input) ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPlan {
    pub household_id: String,
    pub import_id: String,
    pub account_mappings: Vec<AccountMapping>,
    pub transactions: Vec<PlannedTransaction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountMapping {
    pub gnc_guid: String,
    pub gnc_full_name: String,
    pub tally_account_id: String,
    pub tally_name: String,
    pub tally_parent_id: Option<String>,
    pub tally_type: AccountType,
    pub tally_normal_balance: NormalBalance,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AccountType {
    Asset,
    Liability,
    Income,
    Expense,
    Equity,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NormalBalance {
    Debit,
    Credit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedTransaction {
    pub gnc_guid: String,
    pub txn_date: i64,
    pub memo: Option<String>,
    pub lines: Vec<PlannedLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedLine {
    pub tally_account_id: String,
    pub amount_cents: i64, // always positive
    pub side: Side,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Debit,
    Credit,
}

// ── Commit receipt ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReceipt {
    pub import_id: String,
    pub accounts_created: u32,
    pub transactions_committed: u32,
    pub transactions_skipped: u32,
}

// ── Errors ──────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("Couldn't open GnuCash file: {0}")]
    FileUnreadable(String),
    #[error("Not a GnuCash SQLite book")]
    NotAGnuCashBook,
    #[error("Non-USD accounts present: {0:?}")]
    NonUsdAccounts(Vec<String>),
    #[error("Transaction {guid} splits don't sum to zero (sum={sum_cents})")]
    UnbalancedTransaction { guid: String, sum_cents: i64 },
    #[error("Duplicate Tally account name after mapping: {0}")]
    DuplicateAccountName(String),
    #[error("Import with id {0} already exists")]
    ImportAlreadyRan(String),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Validation failure for transaction on {date}: {reason}")]
    InvalidTransaction { date: i64, reason: String },
}

#[cfg(test)]
mod test_fixtures;
