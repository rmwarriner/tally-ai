//! Converts a `GnuCashBook` into an `ImportPlan` using default type mappings,
//! and applies user-supplied overrides. Pure logic — no DB, no IO.

use super::{
    AccountMapping, AccountType, GncAccountType, GncSplit,
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

/// Build the initial `ImportPlan` with the default type mapping and parent
/// hierarchy. Pure — caller provides the household_id, import_id, and a ULID
/// generator for account IDs.
pub fn build_default_plan<F>(
    household_id: String,
    import_id: String,
    book: &GnuCashBook,
    mut new_ulid: F,
) -> Result<ImportPlan, ImportError>
where
    F: FnMut() -> String,
{
    // Step 1: assign a Tally ULID to every importable account.
    let mut guid_to_ulid: HashMap<String, String> = HashMap::new();
    for a in &book.accounts {
        guid_to_ulid.insert(a.guid.clone(), new_ulid());
    }

    // Step 2: build AccountMapping per account.
    let mut account_mappings: Vec<AccountMapping> = Vec::with_capacity(book.accounts.len());
    for a in &book.accounts {
        let (ttype, nb) = default_tally_type(a.gnc_type);
        let tally_parent_id = a
            .parent_guid
            .as_ref()
            .and_then(|pg| guid_to_ulid.get(pg).cloned());
        account_mappings.push(AccountMapping {
            gnc_guid: a.guid.clone(),
            gnc_full_name: a.full_name.clone(),
            tally_account_id: guid_to_ulid.get(&a.guid).expect("pre-assigned").clone(),
            tally_name: a.name.clone(),
            tally_parent_id,
            tally_type: ttype,
            tally_normal_balance: nb,
        });
    }

    // Step 3: convert transactions.
    let mut transactions: Vec<PlannedTransaction> = Vec::with_capacity(book.transactions.len());
    for tx in &book.transactions {
        let mut lines = Vec::with_capacity(tx.splits.len());
        for sp in &tx.splits {
            let tally_id = match guid_to_ulid.get(&sp.account_guid) {
                Some(id) => id.clone(),
                None => continue,
            };
            lines.push(split_to_line(sp, tally_id));
        }
        let memo = if tx.description.is_empty() { None } else { Some(tx.description.clone()) };
        transactions.push(PlannedTransaction {
            gnc_guid: tx.guid.clone(),
            txn_date: tx.post_date,
            memo,
            lines,
        });
    }

    Ok(ImportPlan {
        household_id,
        import_id,
        account_mappings,
        transactions,
    })
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MappingEdit {
    ChangeType {
        gnc_full_name: String,
        new_type: AccountType,
        new_normal_balance: NormalBalance,
    },
    Rename {
        gnc_full_name: String,
        new_tally_name: String,
    },
}

pub fn apply_mapping_edit(plan: &mut ImportPlan, edit: &MappingEdit) -> Result<(), ImportError> {
    let target = match edit {
        MappingEdit::ChangeType { gnc_full_name, .. } => gnc_full_name,
        MappingEdit::Rename { gnc_full_name, .. } => gnc_full_name,
    };
    let m = plan
        .account_mappings
        .iter_mut()
        .find(|m| &m.gnc_full_name == target)
        .ok_or_else(|| ImportError::DuplicateAccountName(format!("unknown account: {target}")))?;

    match edit {
        MappingEdit::ChangeType { new_type, new_normal_balance, .. } => {
            m.tally_type = *new_type;
            m.tally_normal_balance = *new_normal_balance;
        }
        MappingEdit::Rename { new_tally_name, .. } => {
            m.tally_name = new_tally_name.clone();
        }
    }
    Ok(())
}

fn split_to_line(sp: &GncSplit, tally_account_id: String) -> PlannedLine {
    let (amount_cents, side) = if sp.amount_cents >= 0 {
        (sp.amount_cents, Side::Debit)
    } else {
        (-sp.amount_cents, Side::Credit)
    };
    PlannedLine {
        tally_account_id,
        amount_cents,
        side,
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

    use super::super::test_fixtures::{build_fixture, happy_spec};
    use super::super::reader::read;
    use tempfile::tempdir;

    #[tokio::test]
    async fn default_plan_maps_every_non_placeholder_account() {
        let dir = tempdir().unwrap();
        let path = build_fixture(dir.path(), &happy_spec()).await;
        let book = read(&path).await.unwrap();

        let plan = build_default_plan(
            "hh_test".into(),
            "imp_test".into(),
            &book,
            ulid_gen(),
        ).unwrap();

        assert_eq!(plan.account_mappings.len(), 3);
        assert_eq!(plan.transactions.len(), 2);
        assert_eq!(plan.household_id, "hh_test");
        assert_eq!(plan.import_id, "imp_test");
    }

    #[tokio::test]
    async fn default_plan_derives_side_from_signed_amount() {
        let dir = tempdir().unwrap();
        let path = build_fixture(dir.path(), &happy_spec()).await;
        let book = read(&path).await.unwrap();
        let plan = build_default_plan("hh".into(), "imp".into(), &book, ulid_gen()).unwrap();

        let opening = plan.transactions.iter().find(|t| t.gnc_guid == "tx_opening").unwrap();
        let debit_line = opening.lines.iter().find(|l| l.side == Side::Debit).unwrap();
        let credit_line = opening.lines.iter().find(|l| l.side == Side::Credit).unwrap();
        assert_eq!(debit_line.amount_cents, 100000);
        assert_eq!(credit_line.amount_cents, 100000);
    }

    #[tokio::test]
    async fn default_plan_preserves_parent_hierarchy() {
        let dir = tempdir().unwrap();
        let mut spec = happy_spec();
        spec.book_guid = "book_hier".into();
        spec.accounts.push(super::super::test_fixtures::FixtureAccount {
            guid: "acc_food".into(),
            name: "Food".into(),
            account_type: "EXPENSE",
            commodity_guid: "cmdty_usd".into(),
            parent_guid: None,
            placeholder: true,
            hidden: false,
        });
        spec.accounts.iter_mut().find(|a| a.guid == "acc_groceries").unwrap().parent_guid =
            Some("acc_food".into());
        let path = build_fixture(dir.path(), &spec).await;
        let book = read(&path).await.unwrap();

        let plan = build_default_plan("hh".into(), "imp".into(), &book, ulid_gen()).unwrap();
        let food = plan.account_mappings.iter().find(|m| m.gnc_full_name == "Food").unwrap();
        let groc = plan.account_mappings.iter().find(|m| m.gnc_full_name == "Food:Groceries").unwrap();
        assert_eq!(groc.tally_parent_id, Some(food.tally_account_id.clone()));
    }

    #[tokio::test]
    async fn apply_mapping_edit_changes_only_targeted_account() {
        let dir = tempdir().unwrap();
        let path = build_fixture(dir.path(), &happy_spec()).await;
        let book = read(&path).await.unwrap();
        let mut plan = build_default_plan("hh".into(), "imp".into(), &book, ulid_gen()).unwrap();

        let original_count = plan.account_mappings.len();
        let result = apply_mapping_edit(
            &mut plan,
            &MappingEdit::ChangeType {
                gnc_full_name: "Groceries".into(),
                new_type: AccountType::Liability,
                new_normal_balance: NormalBalance::Credit,
            },
        );
        assert!(result.is_ok());

        let groc = plan.account_mappings.iter().find(|m| m.gnc_full_name == "Groceries").unwrap();
        assert_eq!(groc.tally_type, AccountType::Liability);
        assert_eq!(groc.tally_normal_balance, NormalBalance::Credit);

        let chk = plan.account_mappings.iter().find(|m| m.gnc_full_name == "Checking").unwrap();
        assert_eq!(chk.tally_type, AccountType::Asset);
        assert_eq!(plan.account_mappings.len(), original_count);
    }

    #[tokio::test]
    async fn apply_mapping_edit_rename() {
        let dir = tempdir().unwrap();
        let path = build_fixture(dir.path(), &happy_spec()).await;
        let book = read(&path).await.unwrap();
        let mut plan = build_default_plan("hh".into(), "imp".into(), &book, ulid_gen()).unwrap();

        apply_mapping_edit(
            &mut plan,
            &MappingEdit::Rename {
                gnc_full_name: "Groceries".into(),
                new_tally_name: "Food & Household".into(),
            },
        ).unwrap();
        let m = plan.account_mappings.iter().find(|m| m.gnc_full_name == "Groceries").unwrap();
        assert_eq!(m.tally_name, "Food & Household");
    }

    #[tokio::test]
    async fn apply_mapping_edit_unknown_account_errors() {
        let dir = tempdir().unwrap();
        let path = build_fixture(dir.path(), &happy_spec()).await;
        let book = read(&path).await.unwrap();
        let mut plan = build_default_plan("hh".into(), "imp".into(), &book, ulid_gen()).unwrap();
        let err = apply_mapping_edit(
            &mut plan,
            &MappingEdit::ChangeType {
                gnc_full_name: "Nonexistent".into(),
                new_type: AccountType::Asset,
                new_normal_balance: NormalBalance::Debit,
            },
        ).unwrap_err();
        assert!(matches!(err, ImportError::DuplicateAccountName(ref s) if s.contains("Nonexistent")));
    }

    /// Deterministic pseudo-ULID for tests: atomic counter.
    fn ulid_gen() -> impl FnMut() -> String {
        let mut n: u64 = 0;
        move || {
            n += 1;
            format!("ULID_{n:0>20}")
        }
    }
}
