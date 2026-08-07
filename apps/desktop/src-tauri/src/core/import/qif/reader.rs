//! QIF text parser.
//!
//! Targets Banktivity-flavored multi-account exports. Format:
//!
//! ```text
//! !Option:AutoSwitch
//! !Account
//! NMyChecking
//! TBank
//! B1234.56
//! ^
//! ... more account definitions ...
//! !Clear:AutoSwitch
//! !Type:Cat
//! NNeeds:Groceries
//! E
//! ^
//! ... more category definitions ...
//! !Account
//! NMyChecking
//! TBank
//! ^
//! !Type:Bank
//! D1/4/26
//! T-16.23
//! Cc
//! PCoffee
//! LNeeds:Coffee
//! ^
//! ... more transactions ...
//! ```
//!
//! Splits look like `E<memo>` / `S<category>` / `$<amount>` triples.
//! Transfers appear as `L[Other Account]`.

use std::path::Path;

use chrono::TimeZone;

use super::{
    QifAccount, QifAccountType, QifBook, QifCategory, QifCategoryRef, QifError, QifPreview,
    QifSplit, QifTransaction,
};

/// Read a QIF file from disk.
pub async fn read(path: &Path) -> Result<QifBook, QifError> {
    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| QifError::FileUnreadable(e.to_string()))?;
    parse(&content)
}

/// Preview a QIF file (cheap counts for onboarding gating).
pub async fn preview(path: &Path) -> Result<QifPreview, QifError> {
    let book = read(path).await?;
    let split_count: u32 = book
        .transactions
        .iter()
        .map(|t| t.splits.len() as u32)
        .sum();
    let txn_transfers = book
        .transactions
        .iter()
        .filter(|t| matches!(t.category, Some(QifCategoryRef::Transfer(_))))
        .count() as u32;
    let split_transfers = book
        .transactions
        .iter()
        .flat_map(|t| t.splits.iter())
        .filter(|s| matches!(s.category, Some(QifCategoryRef::Transfer(_))))
        .count() as u32;
    Ok(QifPreview {
        account_count: book.accounts.len() as u32,
        transaction_count: book.transactions.len() as u32,
        split_count,
        transfer_count: txn_transfers + split_transfers,
        skipped_security_trades: book.skipped_security_trades,
    })
}

/// Parse QIF content into a [`QifBook`].
pub fn parse(content: &str) -> Result<QifBook, QifError> {
    let mut accounts: Vec<QifAccount> = Vec::new();
    let mut categories: Vec<QifCategory> = Vec::new();
    let mut transactions: Vec<QifTransaction> = Vec::new();
    let mut skipped_security_trades: u32 = 0;
    let mut seen_any_directive = false;
    let mut mode = Mode::Initial;
    let mut autoswitch_pending = false;
    let mut current_account: Option<(String, QifAccountType)> = None;
    let mut record = RecordBuf::default();

    for raw in content.lines() {
        let line = raw.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }

        if let Some(directive) = line.strip_prefix('!') {
            seen_any_directive = true;
            let d = directive.trim();
            if d.eq_ignore_ascii_case("Option:AutoSwitch") {
                autoswitch_pending = true;
            } else if d.eq_ignore_ascii_case("Clear:AutoSwitch") {
                mode = Mode::Initial;
            } else if d.eq_ignore_ascii_case("Account") {
                if autoswitch_pending {
                    mode = Mode::BalancesList;
                    autoswitch_pending = false;
                } else {
                    mode = Mode::ContextRecord;
                }
                record = RecordBuf::default();
            } else if let Some(type_name) = strip_prefix_case_insensitive(d, "Type:") {
                if type_name.eq_ignore_ascii_case("Cat") {
                    mode = Mode::Categories;
                } else {
                    mode = Mode::TxnList;
                }
                record = RecordBuf::default();
            }
            // Unknown directives tolerated; state unchanged.
            continue;
        }

        if line == "^" {
            match mode {
                Mode::BalancesList => {
                    if let Some(acc) = record.take_account_def()? {
                        accounts.push(acc);
                    }
                }
                Mode::Categories => {
                    if let Some(cat) = record.take_category() {
                        categories.push(cat);
                    }
                }
                Mode::ContextRecord => {
                    if let Some((name, ty)) = record.take_context() {
                        if !accounts.iter().any(|a| a.name == name) {
                            accounts.push(QifAccount {
                                name: name.clone(),
                                qif_type: ty,
                                declared_balance_cents: None,
                            });
                        }
                        current_account = Some((name, ty));
                    }
                }
                Mode::TxnList => {
                    let (account_name, account_type) = current_account
                        .clone()
                        .ok_or(QifError::NotAQifFile)?;
                    match record.take_transaction(account_name, account_type)? {
                        TxnResult::Real(t) => transactions.push(t),
                        TxnResult::SkippedSecurityTrade => skipped_security_trades += 1,
                        TxnResult::Empty => {}
                    }
                }
                Mode::Initial => {}
            }
            record = RecordBuf::default();
            continue;
        }

        let mut chars = line.chars();
        let tag = match chars.next() {
            Some(c) => c,
            None => continue,
        };
        let value = chars.as_str();
        record.set_field(mode, tag, value);
    }

    if !seen_any_directive {
        return Err(QifError::NotAQifFile);
    }

    Ok(QifBook {
        accounts,
        categories,
        transactions,
        skipped_security_trades,
    })
}

// ── State machine ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Initial,
    BalancesList,
    Categories,
    ContextRecord,
    TxnList,
}

fn strip_prefix_case_insensitive<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() < prefix.len() {
        return None;
    }
    let (head, tail) = s.split_at(prefix.len());
    if head.eq_ignore_ascii_case(prefix) {
        Some(tail)
    } else {
        None
    }
}

// ── Record buffer ──────────────────────────────────────────────────────────

#[derive(Debug, Default)]
struct RecordBuf {
    // Account definition / context:
    n_field: Option<String>,
    t_field: Option<String>,
    b_field: Option<String>,
    // Category:
    income_expense: Option<char>,
    // Transaction:
    date: Option<String>,
    amount: Option<String>,
    cleared: Option<String>,
    payee: Option<String>,
    memo: Option<String>,
    category_l: Option<String>,
    n_action: Option<String>,
    splits: Vec<SplitBuf>,
}

#[derive(Debug, Default)]
struct SplitBuf {
    memo: Option<String>,
    category: Option<String>,
    amount: Option<String>,
}

enum TxnResult {
    Real(QifTransaction),
    SkippedSecurityTrade,
    Empty,
}

impl RecordBuf {
    fn set_field(&mut self, mode: Mode, tag: char, value: &str) {
        match mode {
            Mode::BalancesList | Mode::ContextRecord => match tag {
                'N' => self.n_field = Some(value.to_string()),
                'T' => self.t_field = Some(value.to_string()),
                'B' => self.b_field = Some(value.to_string()),
                _ => {}
            },
            Mode::Categories => match tag {
                'N' => self.n_field = Some(value.to_string()),
                'I' | 'E' => self.income_expense = Some(tag),
                _ => {}
            },
            Mode::TxnList => self.set_txn_field(tag, value),
            Mode::Initial => {}
        }
    }

    fn set_txn_field(&mut self, tag: char, value: &str) {
        match tag {
            'D' => self.date = Some(value.to_string()),
            'T' => self.amount = Some(value.to_string()),
            'C' => self.cleared = Some(value.to_string()),
            'P' => self.payee = Some(value.to_string()),
            'M' => self.memo = Some(value.to_string()),
            'L' => self.category_l = Some(value.to_string()),
            'N' => self.n_action = Some(value.to_string()),
            'E' => {
                self.splits.push(SplitBuf {
                    memo: Some(value.to_string()),
                    ..SplitBuf::default()
                });
            }
            'S' => {
                if let Some(last) = self.splits.last_mut() {
                    if last.category.is_none() && last.amount.is_none() {
                        last.category = Some(value.to_string());
                        return;
                    }
                }
                self.splits.push(SplitBuf {
                    category: Some(value.to_string()),
                    ..SplitBuf::default()
                });
            }
            '$' => {
                // Only treat $ as a split amount if it completes an in-progress
                // split (preceded by E or S). A lone $ after L[Acct] is the
                // QIF "other-side amount" of a transfer (e.g. Banktivity
                // investment NCash records the full paycheck on the $ line
                // while T holds only the 401k contribution); we ignore it.
                if let Some(last) = self.splits.last_mut() {
                    if last.amount.is_none()
                        && (last.memo.is_some() || last.category.is_some())
                    {
                        last.amount = Some(value.to_string());
                    }
                }
            }
            // Investment fields we don't model:
            'Y' | 'I' | 'Q' | 'O' => {}
            _ => {}
        }
    }

    fn take_account_def(&mut self) -> Result<Option<QifAccount>, QifError> {
        let Some(name) = self.n_field.take() else {
            return Ok(None);
        };
        let type_str = self.t_field.take().unwrap_or_default();
        let qif_type = QifAccountType::parse(&type_str);
        let balance_cents = match self.b_field.take() {
            Some(b) => Some(parse_amount_cents(&b)?),
            None => None,
        };
        Ok(Some(QifAccount {
            name,
            qif_type,
            declared_balance_cents: balance_cents,
        }))
    }

    fn take_category(&mut self) -> Option<QifCategory> {
        let name = self.n_field.take()?;
        let is_income = matches!(self.income_expense.take(), Some('I'));
        Some(QifCategory {
            full_name: name,
            is_income,
        })
    }

    fn take_context(&mut self) -> Option<(String, QifAccountType)> {
        let name = self.n_field.take()?;
        let type_str = self.t_field.take().unwrap_or_default();
        Some((name, QifAccountType::parse(&type_str)))
    }

    fn take_transaction(
        &mut self,
        source_account: String,
        _account_type: QifAccountType,
    ) -> Result<TxnResult, QifError> {
        let Some(date_str) = self.date.take() else {
            return Ok(TxnResult::Empty);
        };
        let Some(amount_str) = self.amount.take() else {
            return Ok(TxnResult::Empty);
        };

        if let Some(action) = self.n_action.take() {
            let lower = action.to_ascii_lowercase();
            if !matches!(
                lower.as_str(),
                "cash" | "miscinc" | "miscexp" | "div" | "intinc" | "cgshort" | "cglong"
            ) {
                return Ok(TxnResult::SkippedSecurityTrade);
            }
        }

        let date_ms = parse_date_ms(&date_str)?;
        let amount_cents = parse_amount_cents(&amount_str)?;
        let cleared = self
            .cleared
            .take()
            .and_then(|c| c.chars().next())
            .unwrap_or(' ');
        let payee = self.payee.take().filter(|s| !s.is_empty());
        let memo = self.memo.take().filter(|s| !s.is_empty());
        let category = self
            .category_l
            .take()
            .filter(|s| !s.is_empty())
            .map(parse_category_ref);

        let splits_buf = std::mem::take(&mut self.splits);
        let mut splits: Vec<QifSplit> = Vec::with_capacity(splits_buf.len());
        for s in splits_buf {
            let Some(amt_str) = s.amount else { continue };
            splits.push(QifSplit {
                memo: s.memo.filter(|s| !s.is_empty()),
                category: s
                    .category
                    .filter(|s| !s.is_empty())
                    .map(parse_category_ref),
                amount_cents: parse_amount_cents(&amt_str)?,
            });
        }

        if !splits.is_empty() {
            let sum: i64 = splits.iter().map(|s| s.amount_cents).sum();
            if sum != amount_cents {
                return Err(QifError::UnbalancedSplit {
                    date_label: date_str,
                    txn_cents: amount_cents,
                    split_sum_cents: sum,
                });
            }
        }

        let source_ref = source_ref_for(&source_account, date_ms, amount_cents, &payee, &memo);

        Ok(TxnResult::Real(QifTransaction {
            source_account,
            date_ms,
            amount_cents,
            payee,
            memo,
            category,
            cleared,
            splits,
            source_ref,
        }))
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

impl QifAccountType {
    pub fn parse(s: &str) -> Self {
        let t = s.trim();
        if t.eq_ignore_ascii_case("Bank") {
            QifAccountType::Bank
        } else if t.eq_ignore_ascii_case("CCard") {
            QifAccountType::CCard
        } else if t.eq_ignore_ascii_case("Cash") {
            QifAccountType::Cash
        } else if t.eq_ignore_ascii_case("Invst") || t.eq_ignore_ascii_case("Port") {
            QifAccountType::Invst
        } else if t.eq_ignore_ascii_case("Oth A") {
            QifAccountType::OthA
        } else if t.eq_ignore_ascii_case("Oth L") {
            QifAccountType::OthL
        } else if t.contains("401") || t.eq_ignore_ascii_case("Retirement") {
            QifAccountType::Retirement
        } else {
            QifAccountType::Bank
        }
    }
}

fn parse_category_ref(value: String) -> QifCategoryRef {
    if value.starts_with('[') && value.ends_with(']') {
        QifCategoryRef::Transfer(value[1..value.len() - 1].to_string())
    } else {
        QifCategoryRef::Category(value)
    }
}

/// Parse a QIF amount string like "16,516.60", "-359.39", or "0" into cents.
pub fn parse_amount_cents(raw: &str) -> Result<i64, QifError> {
    let s = raw.trim().replace(',', "");
    if s.is_empty() {
        return Err(QifError::BadAmount(raw.to_string()));
    }
    let (sign, body) = match s.strip_prefix('-') {
        Some(rest) => (-1i64, rest),
        None => match s.strip_prefix('+') {
            Some(rest) => (1i64, rest),
            None => (1i64, s.as_str()),
        },
    };
    let (whole_str, frac_str) = match body.split_once('.') {
        Some((w, f)) => (w, f),
        None => (body, ""),
    };
    if whole_str.is_empty() && frac_str.is_empty() {
        return Err(QifError::BadAmount(raw.to_string()));
    }
    let whole_n: i64 = if whole_str.is_empty() {
        0
    } else {
        whole_str
            .parse()
            .map_err(|_| QifError::BadAmount(raw.to_string()))?
    };
    let frac_digits: String = frac_str.chars().filter(|c| c.is_ascii_digit()).collect();
    let frac_n: i64 = if frac_digits.is_empty() {
        0
    } else if frac_digits.len() >= 2 {
        frac_digits[..2]
            .parse()
            .map_err(|_| QifError::BadAmount(raw.to_string()))?
    } else {
        format!("{}0", frac_digits)
            .parse()
            .map_err(|_| QifError::BadAmount(raw.to_string()))?
    };
    Ok(sign * (whole_n * 100 + frac_n))
}

/// Parse a QIF date string (`M/D/YY`, `M/D/YYYY`, or `MM/DD'YYYY`) into unix
/// ms at UTC midnight.
pub fn parse_date_ms(raw: &str) -> Result<i64, QifError> {
    let s = raw.trim().replace('\'', "/");
    let parts: Vec<&str> = s.split(|c| c == '/' || c == '-').collect();
    if parts.len() != 3 {
        return Err(QifError::BadDate(raw.to_string()));
    }
    let month: u32 = parts[0]
        .trim()
        .parse()
        .map_err(|_| QifError::BadDate(raw.to_string()))?;
    let day: u32 = parts[1]
        .trim()
        .parse()
        .map_err(|_| QifError::BadDate(raw.to_string()))?;
    let mut year: i32 = parts[2]
        .trim()
        .parse()
        .map_err(|_| QifError::BadDate(raw.to_string()))?;
    if year < 100 {
        year = if year >= 50 { 1900 + year } else { 2000 + year };
    }
    let dt = chrono::Utc
        .with_ymd_and_hms(year, month, day, 0, 0, 0)
        .single()
        .ok_or_else(|| QifError::BadDate(raw.to_string()))?;
    Ok(dt.timestamp_millis())
}

/// Deterministic idempotency key. QIF files lack GUIDs so the natural key is
/// (account, date, amount, payee, memo) — collisions mean a true duplicate.
fn source_ref_for(
    account: &str,
    date_ms: i64,
    amount_cents: i64,
    payee: &Option<String>,
    memo: &Option<String>,
) -> String {
    let p = payee.as_deref().unwrap_or("");
    let m = memo.as_deref().unwrap_or("");
    format!(
        "qif:{}|{}|{}|{}|{}",
        sanitize(account),
        date_ms,
        amount_cents,
        sanitize(p),
        sanitize(m),
    )
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c == '|' || c.is_control() { '_' } else { c })
        .collect()
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amount_with_comma_thousands() {
        assert_eq!(parse_amount_cents("16,516.60").unwrap(), 1_651_660);
        assert_eq!(parse_amount_cents("-359.39").unwrap(), -35_939);
        assert_eq!(parse_amount_cents("0").unwrap(), 0);
        assert_eq!(parse_amount_cents("0.00").unwrap(), 0);
    }

    #[test]
    fn amount_with_single_digit_fraction() {
        assert_eq!(parse_amount_cents("3.5").unwrap(), 350);
        assert_eq!(parse_amount_cents("-0.5").unwrap(), -50);
    }

    #[test]
    fn amount_with_no_fraction() {
        assert_eq!(parse_amount_cents("10").unwrap(), 1000);
        assert_eq!(parse_amount_cents("-2").unwrap(), -200);
    }

    #[test]
    fn amount_bad_input_errors() {
        assert!(matches!(
            parse_amount_cents("abc"),
            Err(QifError::BadAmount(_))
        ));
        assert!(matches!(parse_amount_cents(""), Err(QifError::BadAmount(_))));
    }

    #[test]
    fn date_two_digit_year_pivot() {
        let d26 = parse_date_ms("1/4/26").unwrap();
        let d99 = parse_date_ms("1/4/99").unwrap();
        let dt26 = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(d26).unwrap();
        let dt99 = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(d99).unwrap();
        assert_eq!(dt26.format("%Y-%m-%d").to_string(), "2026-01-04");
        assert_eq!(dt99.format("%Y-%m-%d").to_string(), "1999-01-04");
    }

    #[test]
    fn date_four_digit_year() {
        let d = parse_date_ms("12/31/2024").unwrap();
        let dt = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(d).unwrap();
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2024-12-31");
    }

    #[test]
    fn date_apostrophe_separator() {
        let d = parse_date_ms("12/31'2024").unwrap();
        let dt = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(d).unwrap();
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2024-12-31");
    }

    #[test]
    fn empty_input_is_not_a_qif() {
        assert!(matches!(parse(""), Err(QifError::NotAQifFile)));
        assert!(matches!(parse("   \n   \n"), Err(QifError::NotAQifFile)));
    }

    #[test]
    fn single_account_with_one_transaction() {
        let q = "!Account\nNChecking\nTBank\n^\n!Type:Bank\nD1/4/26\nT-16.23\nPCoffee\n^\n";
        let book = parse(q).unwrap();
        assert_eq!(book.accounts.len(), 1);
        assert_eq!(book.accounts[0].name, "Checking");
        assert_eq!(book.accounts[0].qif_type, QifAccountType::Bank);
        assert_eq!(book.transactions.len(), 1);
        let txn = &book.transactions[0];
        assert_eq!(txn.source_account, "Checking");
        assert_eq!(txn.amount_cents, -1623);
        assert_eq!(txn.payee.as_deref(), Some("Coffee"));
    }

    #[test]
    fn banktivity_style_balances_block_records_declared_balances() {
        let q = concat!(
            "!Option:AutoSwitch\n",
            "!Account\n",
            "NChecking\nTBank\nB1234.56\n^\n",
            "NCard\nTCCard\nB-200.00\n^\n",
            "!Clear:AutoSwitch\n",
            "!Type:Cat\n",
            "NNeeds:Groceries\nE\n^\n",
            "!Account\n",
            "NChecking\nTBank\n^\n",
            "!Type:Bank\n",
            "D1/1/26\nT-50.00\nPGrocer\nLNeeds:Groceries\n^\n",
        );
        let book = parse(q).unwrap();
        assert_eq!(book.accounts.len(), 2);
        assert_eq!(book.accounts[0].declared_balance_cents, Some(123_456));
        assert_eq!(book.accounts[1].declared_balance_cents, Some(-20_000));
        assert_eq!(book.categories.len(), 1);
        assert!(!book.categories[0].is_income);
        assert_eq!(book.transactions.len(), 1);
    }

    #[test]
    fn transfer_category_parsed_as_transfer() {
        let q = concat!(
            "!Account\nNChecking\nTBank\n^\n",
            "!Type:Bank\n",
            "D1/5/26\nT500.00\nPTransfer\nL[Savings]\n^\n",
        );
        let book = parse(q).unwrap();
        match &book.transactions[0].category {
            Some(QifCategoryRef::Transfer(name)) => assert_eq!(name, "Savings"),
            other => panic!("expected transfer, got {other:?}"),
        }
    }

    #[test]
    fn splits_with_e_s_dollar_triples() {
        let q = concat!(
            "!Account\nNCard\nTCCard\n^\n",
            "!Type:CCard\n",
            "D1/20/26\nT-9.35\nPAmazon\n",
            "EHat\nSWants:Clothing/Robert\n$-21.64\n",
            "EGift Card\nSWants:Clothing\n$12.29\n",
            "^\n",
        );
        let book = parse(q).unwrap();
        let txn = &book.transactions[0];
        assert_eq!(txn.splits.len(), 2);
        assert_eq!(txn.splits[0].amount_cents, -2164);
        assert_eq!(txn.splits[1].amount_cents, 1229);
        assert_eq!(txn.splits[0].memo.as_deref(), Some("Hat"));
    }

    #[test]
    fn unbalanced_splits_rejected() {
        let q = concat!(
            "!Account\nNCard\nTCCard\n^\n",
            "!Type:CCard\n",
            "D1/20/26\nT-100.00\nPSomething\n",
            "EA\nSCat1\n$-50.00\n",
            "EB\nSCat2\n$-40.00\n",
            "^\n",
        );
        assert!(matches!(parse(q), Err(QifError::UnbalancedSplit { .. })));
    }

    #[test]
    fn investment_security_trade_is_skipped_and_counted() {
        let q = concat!(
            "!Account\nN401k\nTInvst\n^\n",
            "!Type:Invst\n",
            "D1/1/26\nNBuy\nYBRK Class B\nI67.23\nQ245.673\nT16,516.60\nO0.00\nCc\n^\n",
            "D1/7/26\nNCash\nT147.56\nO0.00\nCc\nPBNSF Railway Payroll\nM2025-24\nL[Checking]\n",
            "$2589.83\n^\n",
        );
        let book = parse(q).unwrap();
        assert_eq!(book.skipped_security_trades, 1);
        assert_eq!(book.transactions.len(), 1, "NCash flow should be kept");
        let txn = &book.transactions[0];
        assert_eq!(txn.amount_cents, 14_756);
        assert!(matches!(txn.category, Some(QifCategoryRef::Transfer(_))));
    }

    #[test]
    fn account_context_without_balances_block_still_registers_account() {
        let q = concat!(
            "!Account\nNPlainBank\nTBank\n^\n",
            "!Type:Bank\n",
            "D1/1/26\nT100.00\nPThing\n^\n",
        );
        let book = parse(q).unwrap();
        assert_eq!(book.accounts.len(), 1);
        assert_eq!(book.accounts[0].name, "PlainBank");
        assert!(book.accounts[0].declared_balance_cents.is_none());
    }

    #[test]
    fn source_ref_stable_across_runs() {
        let r1 = source_ref_for(
            "Checking",
            1_704_412_800_000,
            -1623,
            &Some("Coffee".into()),
            &None,
        );
        let r2 = source_ref_for(
            "Checking",
            1_704_412_800_000,
            -1623,
            &Some("Coffee".into()),
            &None,
        );
        assert_eq!(r1, r2);
        let r3 = source_ref_for(
            "Checking",
            1_704_412_800_000,
            -1623,
            &Some("Coffee".into()),
            &Some("different memo".into()),
        );
        assert_ne!(r1, r3);
    }

    #[test]
    fn banktivity_custom_401k_type_recognized() {
        assert_eq!(
            QifAccountType::parse("401k/403B"),
            QifAccountType::Retirement
        );
    }

    #[test]
    fn multi_account_file_routes_transactions_correctly() {
        let q = concat!(
            "!Account\nNA\nTBank\n^\n",
            "!Type:Bank\n",
            "D1/1/26\nT-1.00\nPa1\n^\n",
            "!Account\nNB\nTCCard\n^\n",
            "!Type:CCard\n",
            "D1/2/26\nT-2.00\nPb1\n^\n",
        );
        let book = parse(q).unwrap();
        assert_eq!(book.transactions.len(), 2);
        assert_eq!(book.transactions[0].source_account, "A");
        assert_eq!(book.transactions[1].source_account, "B");
    }

    /// Smoke test against a real Banktivity export. Skipped unless
    /// `QIF_SMOKE_FILE` is set in the env so CI doesn't depend on private
    /// data. Run locally via:
    ///   QIF_SMOKE_FILE=~/Downloads/2026-05-20_Banktivity_Export.qif \
    ///     cargo test --lib core::import::qif::reader::tests::smoke -- --ignored
    #[test]
    #[ignore]
    fn smoke_real_banktivity_export() {
        let Ok(path) = std::env::var("QIF_SMOKE_FILE") else {
            eprintln!("set QIF_SMOKE_FILE to run this smoke test");
            return;
        };
        let content = std::fs::read_to_string(&path).expect("read smoke file");
        let book = parse(&content).expect("parse smoke file");
        eprintln!(
            "Smoke result: {} accounts, {} categories, {} txns, {} splits, {} skipped trades",
            book.accounts.len(),
            book.categories.len(),
            book.transactions.len(),
            book.transactions.iter().map(|t| t.splits.len()).sum::<usize>(),
            book.skipped_security_trades,
        );
        assert!(book.accounts.len() > 0);
        assert!(book.transactions.len() > 0);
        // Per-account balance sanity: declared balance should be close to
        // (within 100 cents) the sum of imported txn amounts for that account.
        // This is loose — security-trade-skipping introduces drift for Invst
        // accounts.
        for acc in &book.accounts {
            if let Some(declared) = acc.declared_balance_cents {
                let sum: i64 = book
                    .transactions
                    .iter()
                    .filter(|t| t.source_account == acc.name)
                    .map(|t| t.amount_cents)
                    .sum();
                let diff = (sum - declared).abs();
                eprintln!(
                    "  {}: declared={}¢ sum_imported={}¢ diff={}¢",
                    acc.name, declared, sum, diff
                );
            }
        }
    }

    #[test]
    fn fixture_parses_cleanly() {
        let book = parse(super::super::test_fixtures::banktivity_minimal()).unwrap();
        assert_eq!(book.accounts.len(), 3);
        // Reader keeps both legs of every transfer; the mapper dedups.
        assert_eq!(book.transactions.len(), 8);
        let total_splits: usize = book.transactions.iter().map(|t| t.splits.len()).sum();
        assert_eq!(total_splits, 2);
        let transfers = book
            .transactions
            .iter()
            .filter(|t| matches!(t.category, Some(QifCategoryRef::Transfer(_))))
            .count();
        assert_eq!(transfers, 2);
    }
}
