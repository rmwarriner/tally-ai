//! QIF (Quicken Interchange Format) importer.
//!
//! Four phases: reader → mapper → committer → reconciler. Mirrors the GnuCash
//! pipeline but targets Banktivity-flavored multi-account QIF exports (the
//! most common real-world source). See
//! `docs/superpowers/specs/2026-05-22-qif-import-design.md` for rationale.

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod committer;
pub mod mapper;
pub mod reader;
pub mod reconcile;

// ── Reader output ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QifBook {
    pub accounts: Vec<QifAccount>,
    pub categories: Vec<QifCategory>,
    pub transactions: Vec<QifTransaction>,
    pub skipped_security_trades: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QifAccount {
    pub name: String,
    pub qif_type: QifAccountType,
    pub declared_balance_cents: Option<i64>,
}

/// QIF account types observed in Banktivity exports plus the standard set.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum QifAccountType {
    Bank,
    CCard,
    Cash,
    Invst,
    OthA,
    OthL,
    Retirement,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QifCategory {
    pub full_name: String,
    pub is_income: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QifTransaction {
    pub source_account: String,
    pub date_ms: i64,
    pub amount_cents: i64,
    pub payee: Option<String>,
    pub memo: Option<String>,
    pub category: Option<QifCategoryRef>,
    pub cleared: char,
    pub splits: Vec<QifSplit>,
    pub source_ref: String,
}

/// What the QIF `L<value>` field pointed at.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum QifCategoryRef {
    Category(String),       // L<name>
    Transfer(String),       // L[AccountName]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QifSplit {
    pub memo: Option<String>,
    pub category: Option<QifCategoryRef>,
    pub amount_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QifPreview {
    pub account_count: u32,
    pub transaction_count: u32,
    pub split_count: u32,
    pub transfer_count: u32,
    pub skipped_security_trades: u32,
}

// ── Mapper output (committer input) ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QifImportPlan {
    pub household_id: String,
    pub import_id: String,
    pub account_mappings: Vec<QifAccountMapping>,
    pub transactions: Vec<QifPlannedTransaction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QifAccountMapping {
    pub qif_name: String,
    pub tally_account_id: String,
    pub tally_name: String,
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
pub struct QifPlannedTransaction {
    pub source_ref: String,
    pub txn_date: i64,
    pub memo: Option<String>,
    pub lines: Vec<QifPlannedLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QifPlannedLine {
    pub tally_account_id: String,
    pub amount_cents: i64,
    pub side: Side,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Debit,
    Credit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QifMappingEdit {
    ChangeType {
        qif_name: String,
        new_type: AccountType,
        new_normal_balance: NormalBalance,
    },
    Rename {
        qif_name: String,
        new_tally_name: String,
    },
}

// ── Receipts ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QifImportReceipt {
    pub import_id: String,
    pub accounts_created: u32,
    pub transactions_committed: u32,
    pub transactions_skipped: u32,
    pub skipped_security_trades: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QifBalanceReportArtifact {
    pub rows: Vec<QifBalanceRow>,
    pub total_mismatches: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QifBalanceRow {
    pub account_name: String,
    pub tally_cents: i64,
    pub declared_cents: i64,
    pub matches: bool,
}

// ── Errors ──────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum QifError {
    #[error("Couldn't read QIF file: {0}")]
    FileUnreadable(String),
    #[error("Not a recognizable QIF file")]
    NotAQifFile,
    #[error("Unbalanced split on {date_label}: lines sum to {split_sum_cents} but transaction is {txn_cents}")]
    UnbalancedSplit {
        date_label: String,
        txn_cents: i64,
        split_sum_cents: i64,
    },
    #[error("Transfer references unknown account: {0}")]
    UnknownTransferAccount(String),
    #[error("Unparseable date: {0}")]
    BadDate(String),
    #[error("Unparseable amount: {0}")]
    BadAmount(String),
    #[error("Duplicate Tally account name after mapping: {0}")]
    DuplicateAccountName(String),
    #[error("Unknown QIF account: {0}")]
    UnknownAccount(String),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

#[cfg(test)]
mod test_fixtures;
