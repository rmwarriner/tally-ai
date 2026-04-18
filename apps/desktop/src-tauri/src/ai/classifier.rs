// Intent pre-classifier — T-021
// 8 intent types, pattern matching, no ML required.
// Correct/undo patterns are checked first to avoid false matches on other intents.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentKind {
    RecordExpense,
    RecordIncome,
    QueryBalance,
    QueryHistory,
    BudgetManagement,
    CorrectTransaction,
    AccountManagement,
    GeneralQuestion,
}

#[derive(Debug, Clone)]
pub struct ClassifiedIntent {
    pub kind: IntentKind,
}

pub fn classify(input: &str) -> ClassifiedIntent {
    let lower = input.to_lowercase();
    ClassifiedIntent { kind: detect(&lower) }
}

fn detect(lower: &str) -> IntentKind {
    // Correction/undo checked first — "fix my balance" → Correct, not QueryBalance.
    if any(lower, &["undo", "fix ", "fix that", "correct", "mistake", "wrong amount",
                    "wrong account", "change that", "edit that", "delete that",
                    "void", "reverse that", "remove that transaction"]) {
        return IntentKind::CorrectTransaction;
    }
    if any(lower, &["add account", "new account", "create account", "delete account",
                    "remove account", "list accounts", "what accounts", "my accounts",
                    "show accounts", "rename account", "savings account",
                    "checking account", "investment account"]) {
        return IntentKind::AccountManagement;
    }
    if any(lower, &["received", "got paid", "paycheck", "direct deposit", "salary",
                    "wage", "earned", "income", "refund", "reimbursement",
                    "transfer in", "deposited"]) {
        return IntentKind::RecordIncome;
    }
    if any(lower, &["spent", "paid ", "bought", "charged", "cost", "purchase",
                    "expense", "bill ", "paid for", "picked up", "grabbed"]) {
        return IntentKind::RecordExpense;
    }
    if any(lower, &["balance", "how much do i have", "what's in my", "total in",
                    "what do i have", "current balance", "available"]) {
        return IntentKind::QueryBalance;
    }
    if any(lower, &["history", "transactions", "show me", "list my", "recent",
                    "last month", "this month", "this week", "last week",
                    "spending", "what did i spend", "what have i spent"]) {
        return IntentKind::QueryHistory;
    }
    if any(lower, &["budget", "envelope", "allocate", "remaining", "how much left",
                    "over budget", "under budget", "budget status", "set budget",
                    "create envelope", "delete envelope"]) {
        return IntentKind::BudgetManagement;
    }
    IntentKind::GeneralQuestion
}

fn any(lower: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| lower.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(input: &str) -> IntentKind {
        classify(input).kind
    }

    // --- RecordExpense ---

    #[test]
    fn spent_triggers_record_expense() {
        assert_eq!(kind("I spent $42 at Whole Foods"), IntentKind::RecordExpense);
    }

    #[test]
    fn paid_triggers_record_expense() {
        assert_eq!(kind("Paid for dinner last night"), IntentKind::RecordExpense);
    }

    #[test]
    fn bought_triggers_record_expense() {
        assert_eq!(kind("bought groceries today"), IntentKind::RecordExpense);
    }

    // --- RecordIncome ---

    #[test]
    fn got_paid_triggers_record_income() {
        assert_eq!(kind("got paid $3200 today"), IntentKind::RecordIncome);
    }

    #[test]
    fn paycheck_triggers_record_income() {
        assert_eq!(kind("my paycheck hit the account"), IntentKind::RecordIncome);
    }

    #[test]
    fn refund_triggers_record_income() {
        assert_eq!(kind("got a refund from Amazon"), IntentKind::RecordIncome);
    }

    // --- QueryBalance ---

    #[test]
    fn balance_triggers_query_balance() {
        assert_eq!(kind("What's my checking balance?"), IntentKind::QueryBalance);
    }

    #[test]
    fn how_much_do_i_have_triggers_query_balance() {
        assert_eq!(kind("how much do i have in savings"), IntentKind::QueryBalance);
    }

    // --- QueryHistory ---

    #[test]
    fn history_triggers_query_history() {
        assert_eq!(kind("show me my transaction history"), IntentKind::QueryHistory);
    }

    #[test]
    fn this_month_spending_triggers_query_history() {
        assert_eq!(kind("what did i spend this month"), IntentKind::QueryHistory);
    }

    // --- BudgetManagement ---

    #[test]
    fn budget_triggers_budget_management() {
        assert_eq!(kind("how's my grocery budget?"), IntentKind::BudgetManagement);
    }

    #[test]
    fn envelope_remaining_triggers_budget_management() {
        assert_eq!(kind("how much left in my dining envelope"), IntentKind::BudgetManagement);
    }

    // --- CorrectTransaction ---

    #[test]
    fn undo_triggers_correct_transaction() {
        assert_eq!(kind("undo that last transaction"), IntentKind::CorrectTransaction);
    }

    #[test]
    fn fix_triggers_correct_transaction() {
        assert_eq!(kind("fix that, I entered the wrong amount"), IntentKind::CorrectTransaction);
    }

    #[test]
    fn void_triggers_correct_transaction() {
        assert_eq!(kind("void the transaction from yesterday"), IntentKind::CorrectTransaction);
    }

    // --- AccountManagement ---

    #[test]
    fn add_account_triggers_account_management() {
        assert_eq!(kind("add a new savings account"), IntentKind::AccountManagement);
    }

    #[test]
    fn list_accounts_triggers_account_management() {
        assert_eq!(kind("what accounts do I have?"), IntentKind::AccountManagement);
    }

    // --- GeneralQuestion ---

    #[test]
    fn hello_falls_through_to_general() {
        assert_eq!(kind("hello"), IntentKind::GeneralQuestion);
    }

    #[test]
    fn help_request_falls_through_to_general() {
        assert_eq!(kind("can you help me understand double-entry bookkeeping?"), IntentKind::GeneralQuestion);
    }

    // --- Priority ---

    #[test]
    fn correct_takes_priority_over_query_balance() {
        // "fix" should win over "balance"
        assert_eq!(kind("fix my balance entry"), IntentKind::CorrectTransaction);
    }

    #[test]
    fn correct_takes_priority_over_record_expense() {
        assert_eq!(kind("fix that — I didn't actually spent that"), IntentKind::CorrectTransaction);
    }

    // --- Case insensitivity ---

    #[test]
    fn classification_is_case_insensitive() {
        assert_eq!(kind("SPENT $50 AT TARGET"), IntentKind::RecordExpense);
        assert_eq!(kind("GOT PAID TODAY"), IntentKind::RecordIncome);
        assert_eq!(kind("UNDO THAT"), IntentKind::CorrectTransaction);
    }
}
