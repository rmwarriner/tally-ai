//! QIF mapper — pure logic that converts a parsed [`QifBook`] plus user edits
//! into a [`QifImportPlan`] ready for atomic commit.
//!
//! Responsibilities:
//! - Assign ULIDs to every QIF account and to a small set of synthesized
//!   accounts (one Income/Expense account per QIF category that gets used,
//!   plus an Equity:Opening Balance counterpart for STARTING BALANCE
//!   transactions).
//! - Default-map QIF account types to Tally `AccountType` + `NormalBalance`.
//! - Translate each `QifTransaction` into a balanced set of `QifPlannedLine`s.
//!   Transfers (`L[Account]`) pair against the named account. Splits become
//!   one line per split on the category side. STARTING BALANCE payees pair
//!   against Equity:Opening Balance.
//! - Apply user edits (rename, change type) before commit.

use std::collections::HashMap;

use super::{
    AccountType, NormalBalance, QifAccountMapping, QifAccountType, QifBook, QifCategoryRef,
    QifError, QifImportPlan, QifMappingEdit, QifPlannedLine, QifPlannedTransaction,
    QifTransaction, Side,
};

const STARTING_BALANCE_PAYEE: &str = "STARTING BALANCE";
const OPENING_BALANCE_ACCOUNT: &str = "Equity:Opening Balance";
const UNCATEGORIZED_EXPENSE: &str = "Uncategorized Expense";
const UNCATEGORIZED_INCOME: &str = "Uncategorized Income";

/// Default Tally type + normal balance for a QIF account type.
pub fn default_tally_type(qt: QifAccountType) -> (AccountType, NormalBalance) {
    match qt {
        QifAccountType::Bank | QifAccountType::Cash | QifAccountType::OthA => {
            (AccountType::Asset, NormalBalance::Debit)
        }
        QifAccountType::Invst | QifAccountType::Retirement => {
            (AccountType::Asset, NormalBalance::Debit)
        }
        QifAccountType::CCard | QifAccountType::OthL => {
            (AccountType::Liability, NormalBalance::Credit)
        }
    }
}

/// Build the default import plan from a parsed book. Pure: no DB access; ULIDs
/// generated via the supplied closure so tests can stub them.
pub fn build_default_plan<F>(
    household_id: String,
    import_id: String,
    book: &QifBook,
    mut new_ulid: F,
) -> Result<QifImportPlan, QifError>
where
    F: FnMut() -> String,
{
    // 1. Assign ULIDs to each QIF account.
    let mut mappings: Vec<QifAccountMapping> = Vec::with_capacity(book.accounts.len());
    let mut account_id_by_name: HashMap<String, String> = HashMap::new();
    for acc in &book.accounts {
        let id = new_ulid();
        let (ty, nb) = default_tally_type(acc.qif_type);
        account_id_by_name.insert(acc.name.clone(), id.clone());
        mappings.push(QifAccountMapping {
            qif_name: acc.name.clone(),
            tally_account_id: id,
            tally_name: acc.name.clone(),
            tally_type: ty,
            tally_normal_balance: nb,
        });
    }

    // 2. Walk transactions, synthesizing category and opening-balance accounts
    //    on demand. A synthesized account gets one mapping just like a QIF
    //    account so the committer treats them uniformly.
    let mut synth_id_by_name: HashMap<String, String> = HashMap::new();
    let mut ensure_synth = |full_name: &str,
                            ty: AccountType,
                            nb: NormalBalance,
                            mappings: &mut Vec<QifAccountMapping>|
     -> String {
        if let Some(id) = synth_id_by_name.get(full_name) {
            return id.clone();
        }
        let id = new_ulid();
        synth_id_by_name.insert(full_name.to_string(), id.clone());
        mappings.push(QifAccountMapping {
            qif_name: full_name.to_string(),
            tally_account_id: id.clone(),
            tally_name: full_name.to_string(),
            tally_type: ty,
            tally_normal_balance: nb,
        });
        id
    };

    let income_lookup: HashMap<&str, bool> = book
        .categories
        .iter()
        .map(|c| (c.full_name.as_str(), c.is_income))
        .collect();

    let mut planned: Vec<QifPlannedTransaction> = Vec::with_capacity(book.transactions.len());

    for txn in &book.transactions {
        // Dedup transfer pairs. Banktivity exports the same transfer event on
        // both sides of the file (one txn in account A with L[B], a mirror
        // txn in account B with L[A]). We keep the alphabetically smaller
        // source account's leg and drop the other. Only applies to pure
        // transfer txns — txns with splits carry richer data and stay on
        // their source side.
        if txn.splits.is_empty() {
            if let Some(QifCategoryRef::Transfer(target)) = &txn.category {
                let target_canonical = target.split('/').next().unwrap_or(target);
                if account_id_by_name.contains_key(target_canonical)
                    && txn.source_account.as_str() > target_canonical
                {
                    continue;
                }
            }
        }

        let source_id = account_id_by_name
            .get(&txn.source_account)
            .cloned()
            .ok_or_else(|| QifError::UnknownAccount(txn.source_account.clone()))?;

        let lines = plan_lines(
            txn,
            &source_id,
            &account_id_by_name,
            &income_lookup,
            &mut mappings,
            &mut |full_name, ty, nb, m| ensure_synth(full_name, ty, nb, m),
        )?;

        planned.push(QifPlannedTransaction {
            source_ref: txn.source_ref.clone(),
            txn_date: txn.date_ms,
            memo: txn.memo.clone(),
            lines,
        });
    }

    Ok(QifImportPlan {
        household_id,
        import_id,
        account_mappings: mappings,
        transactions: planned,
    })
}

fn plan_lines<E>(
    txn: &QifTransaction,
    source_id: &str,
    account_id_by_name: &HashMap<String, String>,
    income_lookup: &HashMap<&str, bool>,
    mappings: &mut Vec<QifAccountMapping>,
    ensure_synth: &mut E,
) -> Result<Vec<QifPlannedLine>, QifError>
where
    E: FnMut(&str, AccountType, NormalBalance, &mut Vec<QifAccountMapping>) -> String,
{
    let amount_abs = txn.amount_cents.unsigned_abs() as i64;
    let source_side = if txn.amount_cents >= 0 {
        Side::Debit
    } else {
        Side::Credit
    };
    let other_side = invert(source_side);

    // Side line on the source account (always one).
    let mut lines: Vec<QifPlannedLine> = Vec::new();
    if amount_abs > 0 {
        lines.push(QifPlannedLine {
            tally_account_id: source_id.to_string(),
            amount_cents: amount_abs,
            side: source_side,
        });
    }

    // Other side(s): splits if present, otherwise the L field, otherwise
    // STARTING BALANCE / Uncategorized fallback.
    if !txn.splits.is_empty() {
        for split in &txn.splits {
            let (other_id, _) = resolve_counterparty(
                split.category.as_ref(),
                None,
                account_id_by_name,
                income_lookup,
                mappings,
                ensure_synth,
            )?;
            let split_abs = split.amount_cents.unsigned_abs() as i64;
            // A split that matches the txn's sign sits opposite the source
            // line; a split with opposite sign sits on the same side (it
            // contributed to bringing T toward zero).
            let same_sign = (split.amount_cents >= 0) == (txn.amount_cents >= 0);
            let split_side = if same_sign {
                invert(source_side)
            } else {
                source_side
            };
            if split_abs > 0 {
                lines.push(QifPlannedLine {
                    tally_account_id: other_id,
                    amount_cents: split_abs,
                    side: split_side,
                });
            }
        }
    } else if amount_abs > 0 {
        let payee = txn.payee.as_deref().unwrap_or("");
        let is_starting_balance = payee.eq_ignore_ascii_case(STARTING_BALANCE_PAYEE);
        let (other_id, _) = resolve_counterparty(
            txn.category.as_ref(),
            if is_starting_balance {
                Some(OPENING_BALANCE_ACCOUNT)
            } else {
                None
            },
            account_id_by_name,
            income_lookup,
            mappings,
            ensure_synth,
        )?;
        lines.push(QifPlannedLine {
            tally_account_id: other_id,
            amount_cents: amount_abs,
            side: other_side,
        });
    }

    // Final balance check: per CLAUDE.md core invariant, amounts are positive
    // and sides encode direction. Debits must equal credits.
    let debit_sum: i64 = lines
        .iter()
        .filter(|l| l.side == Side::Debit)
        .map(|l| l.amount_cents)
        .sum();
    let credit_sum: i64 = lines
        .iter()
        .filter(|l| l.side == Side::Credit)
        .map(|l| l.amount_cents)
        .sum();
    if debit_sum != credit_sum {
        return Err(QifError::UnbalancedSplit {
            date_label: format!("{}ms", txn.date_ms),
            txn_cents: txn.amount_cents,
            split_sum_cents: debit_sum - credit_sum,
        });
    }

    Ok(lines)
}

fn invert(s: Side) -> Side {
    match s {
        Side::Debit => Side::Credit,
        Side::Credit => Side::Debit,
    }
}

fn resolve_counterparty<E>(
    category: Option<&QifCategoryRef>,
    fallback_synth_name: Option<&str>,
    account_id_by_name: &HashMap<String, String>,
    income_lookup: &HashMap<&str, bool>,
    mappings: &mut Vec<QifAccountMapping>,
    ensure_synth: &mut E,
) -> Result<(String, AccountType), QifError>
where
    E: FnMut(&str, AccountType, NormalBalance, &mut Vec<QifAccountMapping>) -> String,
{
    match category {
        Some(QifCategoryRef::Transfer(target_name)) => {
            // Strip everything after `/` — Banktivity uses
            // `L[Account]/SubAccount` for some transfers; we only model the
            // top-level account.
            let canonical = target_name.split('/').next().unwrap_or(target_name);
            let id = account_id_by_name.get(canonical).cloned().ok_or_else(|| {
                QifError::UnknownTransferAccount(canonical.to_string())
            })?;
            Ok((id, AccountType::Asset)) // Type unused by caller currently.
        }
        Some(QifCategoryRef::Category(name)) => {
            // Strip class portion (`Category/Class`) — we don't model classes.
            let canonical = name.split('/').next().unwrap_or(name).to_string();
            // Banktivity quirk: in split lines, an `S<AccountName>` (no
            // brackets) is used when the split flows to a real account. If
            // the category name matches an account we already know about,
            // treat it as a transfer rather than synthesizing a duplicate
            // category account.
            if let Some(id) = account_id_by_name.get(canonical.as_str()) {
                return Ok((id.clone(), AccountType::Asset));
            }
            let is_income = income_lookup
                .get(canonical.as_str())
                .copied()
                .unwrap_or_else(|| {
                    canonical.to_ascii_lowercase().contains("income")
                });
            let (ty, nb) = if is_income {
                (AccountType::Income, NormalBalance::Credit)
            } else {
                (AccountType::Expense, NormalBalance::Debit)
            };
            let id = ensure_synth(&canonical, ty, nb, mappings);
            Ok((id, ty))
        }
        None => {
            let name = fallback_synth_name.unwrap_or(UNCATEGORIZED_EXPENSE);
            let (ty, nb) = if name == OPENING_BALANCE_ACCOUNT {
                (AccountType::Equity, NormalBalance::Credit)
            } else if name == UNCATEGORIZED_INCOME {
                (AccountType::Income, NormalBalance::Credit)
            } else {
                (AccountType::Expense, NormalBalance::Debit)
            };
            let id = ensure_synth(name, ty, nb, mappings);
            Ok((id, ty))
        }
    }
}

/// Apply a single user edit in place. Returns `UnknownAccount` if the target
/// QIF name doesn't exist in the plan.
pub fn apply_mapping_edit(plan: &mut QifImportPlan, edit: QifMappingEdit) -> Result<(), QifError> {
    match edit {
        QifMappingEdit::ChangeType {
            qif_name,
            new_type,
            new_normal_balance,
        } => {
            let m = plan
                .account_mappings
                .iter_mut()
                .find(|m| m.qif_name == qif_name)
                .ok_or_else(|| QifError::UnknownAccount(qif_name.clone()))?;
            m.tally_type = new_type;
            m.tally_normal_balance = new_normal_balance;
        }
        QifMappingEdit::Rename {
            qif_name,
            new_tally_name,
        } => {
            let m = plan
                .account_mappings
                .iter_mut()
                .find(|m| m.qif_name == qif_name)
                .ok_or_else(|| QifError::UnknownAccount(qif_name.clone()))?;
            m.tally_name = new_tally_name;
        }
    }
    Ok(())
}

/// Detect duplicate Tally names after mapping. Returns the duplicated names.
pub fn find_duplicate_names(plan: &QifImportPlan) -> Vec<String> {
    let mut counts: HashMap<&String, u32> = HashMap::new();
    for m in &plan.account_mappings {
        *counts.entry(&m.tally_name).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .filter(|(_, c)| *c > 1)
        .map(|(name, _)| name.clone())
        .collect()
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::import::qif::reader;
    use crate::core::import::qif::test_fixtures;

    fn counter_ulid_gen() -> impl FnMut() -> String {
        let mut n = 0u32;
        move || {
            n += 1;
            format!("ULID{:04}", n)
        }
    }

    #[test]
    fn default_type_mapping_covers_all_qif_types() {
        assert_eq!(
            default_tally_type(QifAccountType::Bank),
            (AccountType::Asset, NormalBalance::Debit)
        );
        assert_eq!(
            default_tally_type(QifAccountType::CCard),
            (AccountType::Liability, NormalBalance::Credit)
        );
        assert_eq!(
            default_tally_type(QifAccountType::OthL),
            (AccountType::Liability, NormalBalance::Credit)
        );
        assert_eq!(
            default_tally_type(QifAccountType::Retirement),
            (AccountType::Asset, NormalBalance::Debit)
        );
    }

    #[test]
    fn fixture_plan_balances_for_every_transaction() {
        let book = reader::parse(test_fixtures::banktivity_minimal()).unwrap();
        let plan = build_default_plan(
            "HH1".into(),
            "IMPORT1".into(),
            &book,
            counter_ulid_gen(),
        )
        .unwrap();
        // Reader has 8 txns; mapper drops the Savings-side leg of the
        // Checking↔Savings transfer pair, leaving 7.
        assert_eq!(plan.transactions.len(), 7);
        for t in &plan.transactions {
            let d: i64 = t
                .lines
                .iter()
                .filter(|l| l.side == Side::Debit)
                .map(|l| l.amount_cents)
                .sum();
            let c: i64 = t
                .lines
                .iter()
                .filter(|l| l.side == Side::Credit)
                .map(|l| l.amount_cents)
                .sum();
            assert_eq!(d, c, "txn {} unbalanced", t.source_ref);
        }
    }

    #[test]
    fn starting_balance_pairs_against_opening_balance_equity() {
        let q = concat!(
            "!Account\nNChecking\nTBank\n^\n",
            "!Type:Bank\n",
            "D1/1/26\nT1000.00\nCc\nPSTARTING BALANCE\nMBALANCE ADJUSTMENT\n^\n",
        );
        let book = reader::parse(q).unwrap();
        let plan = build_default_plan(
            "HH1".into(),
            "IMP1".into(),
            &book,
            counter_ulid_gen(),
        )
        .unwrap();
        // 1 account + 1 synthesized Equity:Opening Balance.
        assert_eq!(plan.account_mappings.len(), 2);
        assert!(plan
            .account_mappings
            .iter()
            .any(|m| m.qif_name == OPENING_BALANCE_ACCOUNT
                && m.tally_type == AccountType::Equity));
        let txn = &plan.transactions[0];
        assert_eq!(txn.lines.len(), 2);
    }

    #[test]
    fn transfer_resolves_to_target_account_id() {
        let q = concat!(
            "!Account\nNChecking\nTBank\n^\n!Type:Bank\n^\n",
            "!Account\nNSavings\nTBank\n^\n!Type:Bank\n^\n",
            "!Account\nNChecking\nTBank\n^\n",
            "!Type:Bank\n",
            "D1/5/26\nT-100.00\nPTransfer\nL[Savings]\n^\n",
        );
        let book = reader::parse(q).unwrap();
        let plan = build_default_plan(
            "HH1".into(),
            "IMP1".into(),
            &book,
            counter_ulid_gen(),
        )
        .unwrap();
        let txn = &plan.transactions[0];
        let checking_id = &plan
            .account_mappings
            .iter()
            .find(|m| m.qif_name == "Checking")
            .unwrap()
            .tally_account_id;
        let savings_id = &plan
            .account_mappings
            .iter()
            .find(|m| m.qif_name == "Savings")
            .unwrap()
            .tally_account_id;
        let ids: Vec<&str> = txn.lines.iter().map(|l| l.tally_account_id.as_str()).collect();
        assert!(ids.contains(&checking_id.as_str()));
        assert!(ids.contains(&savings_id.as_str()));
    }

    #[test]
    fn split_transaction_produces_one_line_per_split() {
        let q = concat!(
            "!Account\nNCard\nTCCard\n^\n",
            "!Type:CCard\n",
            "D1/20/26\nT-30.00\nPAmazon\n",
            "EItemA\nSNeeds:Groceries\n$-20.00\n",
            "EItemB\nSNeeds:Groceries\n$-10.00\n",
            "^\n",
        );
        let book = reader::parse(q).unwrap();
        let plan = build_default_plan(
            "HH1".into(),
            "IMP1".into(),
            &book,
            counter_ulid_gen(),
        )
        .unwrap();
        let txn = &plan.transactions[0];
        assert_eq!(txn.lines.len(), 3);
        let credit_total: i64 = txn
            .lines
            .iter()
            .filter(|l| l.side == Side::Credit)
            .map(|l| l.amount_cents)
            .sum();
        let debit_total: i64 = txn
            .lines
            .iter()
            .filter(|l| l.side == Side::Debit)
            .map(|l| l.amount_cents)
            .sum();
        assert_eq!(credit_total, 3000);
        assert_eq!(debit_total, 3000);
    }

    #[test]
    fn rename_edit_changes_only_target() {
        let book =
            reader::parse(test_fixtures::banktivity_minimal()).unwrap();
        let mut plan = build_default_plan(
            "HH1".into(),
            "IMP1".into(),
            &book,
            counter_ulid_gen(),
        )
        .unwrap();
        apply_mapping_edit(
            &mut plan,
            QifMappingEdit::Rename {
                qif_name: "TestChecking".into(),
                new_tally_name: "RenamedChecking".into(),
            },
        )
        .unwrap();
        let m = plan
            .account_mappings
            .iter()
            .find(|m| m.qif_name == "TestChecking")
            .unwrap();
        assert_eq!(m.tally_name, "RenamedChecking");
    }

    #[test]
    fn change_type_edit_updates_type_and_normal_balance() {
        let book =
            reader::parse(test_fixtures::banktivity_minimal()).unwrap();
        let mut plan = build_default_plan(
            "HH1".into(),
            "IMP1".into(),
            &book,
            counter_ulid_gen(),
        )
        .unwrap();
        apply_mapping_edit(
            &mut plan,
            QifMappingEdit::ChangeType {
                qif_name: "TestChecking".into(),
                new_type: AccountType::Liability,
                new_normal_balance: NormalBalance::Credit,
            },
        )
        .unwrap();
        let m = plan
            .account_mappings
            .iter()
            .find(|m| m.qif_name == "TestChecking")
            .unwrap();
        assert_eq!(m.tally_type, AccountType::Liability);
        assert_eq!(m.tally_normal_balance, NormalBalance::Credit);
    }

    #[test]
    fn unknown_account_edit_errors() {
        let book =
            reader::parse(test_fixtures::banktivity_minimal()).unwrap();
        let mut plan = build_default_plan(
            "HH1".into(),
            "IMP1".into(),
            &book,
            counter_ulid_gen(),
        )
        .unwrap();
        let err = apply_mapping_edit(
            &mut plan,
            QifMappingEdit::Rename {
                qif_name: "Nonexistent".into(),
                new_tally_name: "X".into(),
            },
        );
        assert!(matches!(err, Err(QifError::UnknownAccount(_))));
    }

    #[test]
    fn duplicate_names_detected_after_rename() {
        let book =
            reader::parse(test_fixtures::banktivity_minimal()).unwrap();
        let mut plan = build_default_plan(
            "HH1".into(),
            "IMP1".into(),
            &book,
            counter_ulid_gen(),
        )
        .unwrap();
        assert!(find_duplicate_names(&plan).is_empty());
        apply_mapping_edit(
            &mut plan,
            QifMappingEdit::Rename {
                qif_name: "TestCard".into(),
                new_tally_name: "TestChecking".into(),
            },
        )
        .unwrap();
        let dups = find_duplicate_names(&plan);
        assert_eq!(dups, vec!["TestChecking".to_string()]);
    }

    #[test]
    fn missing_transfer_target_errors() {
        let q = concat!(
            "!Account\nNChecking\nTBank\n^\n",
            "!Type:Bank\n",
            "D1/5/26\nT-100.00\nPTransfer\nL[Nonexistent]\n^\n",
        );
        let book = reader::parse(q).unwrap();
        let err = build_default_plan(
            "HH1".into(),
            "IMP1".into(),
            &book,
            counter_ulid_gen(),
        );
        assert!(matches!(err, Err(QifError::UnknownTransferAccount(_))));
    }

    /// Smoke test against the real Banktivity export. Run via:
    ///   QIF_SMOKE_FILE=~/Downloads/2026-05-20_Banktivity_Export.qif \
    ///     cargo test --lib core::import::qif::mapper::tests::smoke -- --ignored --nocapture
    #[test]
    #[ignore]
    fn smoke_real_banktivity_plan_balances() {
        let Ok(path) = std::env::var("QIF_SMOKE_FILE") else {
            eprintln!("set QIF_SMOKE_FILE");
            return;
        };
        let content = std::fs::read_to_string(&path).unwrap();
        let book = reader::parse(&content).unwrap();
        let plan = build_default_plan(
            "HH1".into(),
            "IMP1".into(),
            &book,
            counter_ulid_gen(),
        )
        .expect("plan should build");
        let dups = find_duplicate_names(&plan);
        if !dups.is_empty() {
            eprintln!("Duplicates found:");
            for d in &dups {
                let occurrences: Vec<_> =
                    plan.account_mappings.iter().filter(|m| m.tally_name == *d).collect();
                eprintln!("  {} appears {} times", d, occurrences.len());
                for m in occurrences {
                    eprintln!(
                        "    qif_name={:?} type={:?} id={}",
                        m.qif_name, m.tally_type, m.tally_account_id
                    );
                }
            }
            eprintln!("Total mappings: {}", plan.account_mappings.len());
            eprintln!("Book accounts: {}", book.accounts.len());
            panic!("duplicate names");
        }
        // Every txn must balance.
        let mut unbalanced = 0;
        for t in &plan.transactions {
            let d: i64 = t
                .lines
                .iter()
                .filter(|l| l.side == Side::Debit)
                .map(|l| l.amount_cents)
                .sum();
            let c: i64 = t
                .lines
                .iter()
                .filter(|l| l.side == Side::Credit)
                .map(|l| l.amount_cents)
                .sum();
            if d != c {
                eprintln!("UNBALANCED: {} d={} c={}", t.source_ref, d, c);
                unbalanced += 1;
            }
        }
        assert_eq!(unbalanced, 0);
        eprintln!(
            "Mapped {} txns into {} accounts ({} synthesized)",
            plan.transactions.len(),
            plan.account_mappings.len(),
            plan.account_mappings.len() - book.accounts.len()
        );
    }

    #[test]
    fn uncategorized_transaction_pairs_against_uncategorized_expense() {
        let q = concat!(
            "!Account\nNChecking\nTBank\n^\n",
            "!Type:Bank\n",
            "D1/5/26\nT-50.00\nPMystery\n^\n",
        );
        let book = reader::parse(q).unwrap();
        let plan = build_default_plan(
            "HH1".into(),
            "IMP1".into(),
            &book,
            counter_ulid_gen(),
        )
        .unwrap();
        assert!(plan
            .account_mappings
            .iter()
            .any(|m| m.qif_name == UNCATEGORIZED_EXPENSE));
    }
}
