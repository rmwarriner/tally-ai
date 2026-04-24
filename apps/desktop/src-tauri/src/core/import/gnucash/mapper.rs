//! Converts a `GnuCashBook` into an `ImportPlan` using default type mappings,
//! and applies user-supplied overrides. Pure logic — no DB, no IO.

use super::{
    AccountMapping, AccountType, GncAccount, GncAccountType, GncSplit, GncTransaction,
    GnuCashBook, ImportError, ImportPlan, NormalBalance, PlannedLine, PlannedTransaction, Side,
};
use std::collections::HashMap;

/// The default GnuCash-type → Tally (type, normal_balance) mapping.
pub fn default_tally_type(gnc: GncAccountType) -> (AccountType, NormalBalance) {
    use GncAccountType::*;
    match gnc {
        Bank | Cash | Asset | Stock | Mutual | Receivable => {
            (AccountType::Asset, NormalBalance::Debit)
        }
        Credit | Liability | Payable => (AccountType::Liability, NormalBalance::Credit),
        Income => (AccountType::Income, NormalBalance::Credit),
        Expense => (AccountType::Expense, NormalBalance::Debit),
        Equity => (AccountType::Equity, NormalBalance::Credit),
        Root | Trading => (AccountType::Equity, NormalBalance::Credit),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bank_maps_to_asset_debit() {
        assert_eq!(default_tally_type(GncAccountType::Bank), (AccountType::Asset, NormalBalance::Debit));
    }
    #[test]
    fn credit_maps_to_liability_credit() {
        assert_eq!(default_tally_type(GncAccountType::Credit), (AccountType::Liability, NormalBalance::Credit));
    }
    #[test]
    fn income_maps_to_income_credit() {
        assert_eq!(default_tally_type(GncAccountType::Income), (AccountType::Income, NormalBalance::Credit));
    }
    #[test]
    fn expense_maps_to_expense_debit() {
        assert_eq!(default_tally_type(GncAccountType::Expense), (AccountType::Expense, NormalBalance::Debit));
    }
    #[test]
    fn equity_maps_to_equity_credit() {
        assert_eq!(default_tally_type(GncAccountType::Equity), (AccountType::Equity, NormalBalance::Credit));
    }
    #[test]
    fn stock_maps_to_asset_debit() {
        assert_eq!(default_tally_type(GncAccountType::Stock), (AccountType::Asset, NormalBalance::Debit));
    }
}
