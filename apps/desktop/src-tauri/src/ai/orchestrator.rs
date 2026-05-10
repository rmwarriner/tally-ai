// Chat orchestrator (T-046)
// Given a raw user message, decides whether to:
//   - serve a balance query directly from the financial snapshot (no AI call),
//   - route an entry intent through the Claude adapter as a TransactionProposal,
//   - return a placeholder text response for intents we don't cover yet.
//
// The orchestrator is AI-adapter-agnostic — tests inject a mock adapter.
// Prompt assembly order follows CLAUDE.md: BASE > SNAPSHOT > INTENT > HISTORY > MEMORY.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use thiserror::Error;

use crate::ai::adapter::{AdapterError, AiAdapter, AiUsage, ProposeResult};
use crate::ai::classifier::{classify, IntentKind};
use crate::ai::payee_memory::PayeeMemory;
use crate::ai::prompt::PromptBuilder;
use crate::ai::snapshot::{build_snapshot_with_scope, SnapshotScope};
use crate::ai::{Message, Role};
use crate::chat::ChatRepo;
use crate::core::insight::{log_if_new, should_emit, ProactiveInsight, Sensitivity};
use crate::core::proposal::TransactionProposal;
use crate::core::triggers::duplicate as duplicate_trigger;
use crate::core::validation::{validate_proposal, AIAdvisory, ValidationResult};

const BASE_SYSTEM_PROMPT: &str = "\
You are Tally, a household finance assistant. The user chats with you naturally about their money; \
you log transactions, answer balance questions, and keep their books. All money is stored as integer cents. \
Every transaction is double-entry: debits and credits must balance. Account IDs are 26-character ULIDs — \
always pick IDs from the financial snapshot below; never invent one. When the user describes a transaction, \
call the submit_transaction_proposal tool with the structured proposal. Keep text replies short and direct — \
no hedging, no recap, no markdown.";

const HISTORY_FETCH_LIMIT: i64 = 40;
/// T-067: cap raw history at 10 messages. Anything older folds into the
/// rolling summary so HISTORY stays under its ~800-token budget.
const PROMPT_HISTORY_LIMIT: usize = 10;
const PAYEE_MEMORY_HINTS: usize = 10;

#[derive(Debug, Error)]
pub enum OrchestratorError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("AI adapter error: {0}")]
    Adapter(#[from] AdapterError),
    #[error("chat error: {0}")]
    Chat(#[from] crate::chat::ChatError),
}

/// What the orchestrator hands back to the Tauri layer. Variant `Proposal`
/// is rendered as a pending transaction card; `Text` as a plain AI message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MessageResponse {
    Text {
        text: String,
    },
    Proposal {
        proposal: TransactionProposal,
        /// Pre-commit validation result — lets the UI show warnings before
        /// the user confirms. The commit path re-validates server-side.
        validation: ValidationResult,
        advisories: Vec<AIAdvisory>,
        /// account_id → human-readable account name, for rendering the card
        /// without a second round-trip. Keys cover every line in the proposal.
        account_names: HashMap<String, String>,
        /// Proactive engine insights raised by pre-commit triggers (e.g.
        /// possible duplicate). Sensitivity-gated and deduped via
        /// `core::insight::log_if_new`. Empty when nothing fires.
        #[serde(default)]
        proactive_insights: Vec<ProactiveInsight>,
        /// Token usage for the propose call (T-069). The frontend echoes
        /// this back on `commit_proposal` so the resulting transaction row
        /// captures `ai_input_tokens` / `ai_output_tokens` / `ai_cache_hit`.
        #[serde(default)]
        ai_usage: AiUsage,
    },
}

pub struct Orchestrator {
    pool: SqlitePool,
    adapter: Arc<dyn AiAdapter>,
    chat: ChatRepo,
    payee_memory: PayeeMemory,
}

impl Orchestrator {
    pub fn new(pool: SqlitePool, adapter: Arc<dyn AiAdapter>) -> Self {
        Self {
            chat: ChatRepo::new(pool.clone()),
            payee_memory: PayeeMemory::new(pool.clone()),
            pool,
            adapter,
        }
    }

    pub async fn handle(
        &self,
        household_id: &str,
        user_text: &str,
    ) -> Result<MessageResponse, OrchestratorError> {
        let intent = classify(user_text);
        let now = now_ms();
        // T-068: load only what each intent needs. QueryBalance skips
        // envelope rows (it doesn't render them), and non-Record intents
        // also skip payee memory hints.
        let scope = scope_for(intent.kind);
        let snapshot = build_snapshot_with_scope(&self.pool, household_id, now, scope).await?;

        match intent.kind {
            IntentKind::QueryBalance => Ok(MessageResponse::Text { text: snapshot.to_prompt_text() }),

            IntentKind::RecordExpense | IntentKind::RecordIncome => {
                let HistoryWithSummary { history, summary } =
                    self.load_prompt_history(household_id, now).await?;
                // Memory hints are intent-scoped: only Record intents
                // (T-068) — they're the only path that uses payee memory.
                let hints = self.payee_memory.top_hints(household_id, PAYEE_MEMORY_HINTS).await;

                let mut prompt = PromptBuilder::new(
                    BASE_SYSTEM_PROMPT,
                    snapshot.to_prompt_text_with_ids(),
                )
                .with_intent(intent)
                .with_history(history)
                .with_memory(hints)
                .build();
                // T-067: prepend the rolling summary so older context isn't lost.
                if let Some(summary) = summary {
                    prompt.messages.insert(
                        0,
                        Message::user(format!("[Earlier conversation summary: {summary}]")),
                    );
                }
                prompt.messages.push(Message::user(user_text.to_string()));

                let ProposeResult { proposal, usage } = self.adapter.propose(&prompt).await?;
                let validation = validate_proposal(&self.pool, &proposal).await;
                let account_names = lookup_account_names(&self.pool, &proposal).await?;
                let proactive_insights =
                    run_pre_commit_triggers(&self.pool, household_id, &proposal, now).await;
                Ok(MessageResponse::Proposal {
                    proposal,
                    validation,
                    advisories: Vec::new(),
                    account_names,
                    proactive_insights,
                    ai_usage: usage,
                })
            }

            IntentKind::QueryHistory
            | IntentKind::BudgetManagement
            | IntentKind::CorrectTransaction
            | IntentKind::AccountManagement
            | IntentKind::GeneralQuestion => Ok(MessageResponse::Text {
                text: "That type of request isn't wired up yet — for now I can log transactions and show account balances. Try \"I spent $10 on coffee\" or \"what's my balance?\".".to_string(),
            }),
        }
    }

    /// T-067: returns the last 10 conversational turns plus, if older
    /// turns exist beyond the cap, a deterministic rolling summary of the
    /// dropped pairs. Persists the summary asynchronously so subsequent
    /// requests can read it without recomputing.
    async fn load_prompt_history(
        &self,
        household_id: &str,
        now_ms: i64,
    ) -> Result<HistoryWithSummary, OrchestratorError> {
        let rows = self.chat.list_before(household_id, i64::MAX, HISTORY_FETCH_LIMIT).await?;
        let mut all: Vec<Message> = rows
            .into_iter()
            .filter_map(|row| match row.kind.as_str() {
                "user" => extract_text(&row.payload).map(|t| Message { role: Role::User, content: t }),
                "ai" => extract_text(&row.payload).map(|t| Message { role: Role::Assistant, content: t }),
                _ => None,
            })
            .collect();
        all.reverse(); // chronological

        if all.len() <= PROMPT_HISTORY_LIMIT {
            return Ok(HistoryWithSummary {
                history: all,
                summary: None,
            });
        }

        let split = all.len() - PROMPT_HISTORY_LIMIT;
        let older = &all[..split];
        let summary = compress_history_pairs(older);
        let recent = all[split..].to_vec();

        // Async persistence: best-effort, never blocks the response path.
        crate::ai::session_summary::store_summary_async(
            self.pool.clone(),
            household_id.to_string(),
            household_id.to_string(), // session_id stand-in (Phase 1 single session per household)
            summary.clone(),
            now_ms,
        );

        Ok(HistoryWithSummary {
            history: recent,
            summary: Some(summary),
        })
    }
}

struct HistoryWithSummary {
    history: Vec<Message>,
    summary: Option<String>,
}

/// Deterministic local compressor (T-067). Walks the older messages in
/// chronological order, pairing user/AI turns into one-line summaries:
///   "User asked: <first 80 chars>. AI answered: <first 80 chars>."
/// An odd trailing message (no AI reply yet) gets summarized solo.
fn compress_history_pairs(older: &[Message]) -> String {
    const SNIPPET: usize = 80;
    let mut lines: Vec<String> = Vec::new();
    let mut i = 0;
    while i < older.len() {
        match (&older[i], older.get(i + 1)) {
            (m, _) if matches!(m.role, Role::User) => {
                let user_snip = snippet(&m.content, SNIPPET);
                if let Some(next) = older.get(i + 1) {
                    if matches!(next.role, Role::Assistant) {
                        let ai_snip = snippet(&next.content, SNIPPET);
                        lines.push(format!("User asked: {user_snip}. AI answered: {ai_snip}."));
                        i += 2;
                        continue;
                    }
                }
                lines.push(format!("User asked: {user_snip}."));
                i += 1;
            }
            (m, _) => {
                // Bare assistant message (lost preceding user turn) — keep it.
                let ai_snip = snippet(&m.content, SNIPPET);
                lines.push(format!("AI said: {ai_snip}."));
                i += 1;
            }
        }
    }
    lines.join(" ")
}

/// T-068 intent → snapshot scope. Record intents need full envelope detail
/// because the user may target an envelope by name; everything else can
/// drop envelopes entirely. Memory hints are gated separately at the call
/// site (only Record intents pull them).
fn scope_for(intent: IntentKind) -> SnapshotScope {
    match intent {
        IntentKind::RecordExpense | IntentKind::RecordIncome => SnapshotScope::Full,
        _ => SnapshotScope::QueryBalance,
    }
}

fn snippet(text: &str, max: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max {
        return trimmed.to_string();
    }
    let cut: String = trimmed.chars().take(max).collect();
    format!("{cut}…")
}

async fn lookup_account_names(
    pool: &SqlitePool,
    proposal: &TransactionProposal,
) -> Result<HashMap<String, String>, sqlx::Error> {
    let mut out = HashMap::new();
    for line in &proposal.lines {
        if out.contains_key(&line.account_id) {
            continue;
        }
        let row: Option<(String,)> = sqlx::query_as("SELECT name FROM accounts WHERE id = ?")
            .bind(&line.account_id)
            .fetch_optional(pool)
            .await?;
        // Unknown accounts land in the map as the raw ID so the UI still has
        // something to render; validation surfaces the underlying issue.
        let name = row.map(|(n,)| n).unwrap_or_else(|| line.account_id.clone());
        out.insert(line.account_id.clone(), name);
    }
    Ok(out)
}

fn extract_text(payload: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(payload).ok()?;
    v.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// Best-effort pre-commit trigger pass. Failures are logged and swallowed —
/// the proposal must still reach the user even if a trigger query errors.
async fn run_pre_commit_triggers(
    pool: &SqlitePool,
    household_id: &str,
    proposal: &TransactionProposal,
    now_ms: i64,
) -> Vec<ProactiveInsight> {
    let (sensitivity, tz) = match load_sensitivity_and_tz(pool, household_id).await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[orchestrator] failed to load sensitivity/tz: {e}");
            return Vec::new();
        }
    };

    let mut emitted = Vec::new();

    match duplicate_trigger::check(pool, household_id, &tz, proposal).await {
        Ok(Some(draft)) if should_emit(draft.kind, sensitivity) => {
            match log_if_new(pool, household_id, &tz, now_ms, draft).await {
                Ok(Some(insight)) => emitted.push(insight),
                Ok(None) => {} // dedup'd
                Err(e) => eprintln!("[orchestrator] insight log failed: {e}"),
            }
        }
        Ok(_) => {}
        Err(e) => eprintln!("[orchestrator] duplicate trigger failed: {e}"),
    }

    emitted
}

async fn load_sensitivity_and_tz(
    pool: &SqlitePool,
    household_id: &str,
) -> Result<(Sensitivity, String), sqlx::Error> {
    let (raw_sensitivity, tz): (String, String) =
        sqlx::query_as("SELECT sensitivity, timezone FROM households WHERE id = ?")
            .bind(household_id)
            .fetch_one(pool)
            .await?;
    let sensitivity = Sensitivity::parse(&raw_sensitivity).unwrap_or(Sensitivity::Normal);
    Ok((sensitivity, tz))
}

#[cfg(test)]
mod compressor_tests {
    use super::*;

    fn user(s: &str) -> Message { Message::user(s.to_string()) }
    fn ai(s: &str) -> Message { Message::assistant(s.to_string()) }

    #[test]
    fn pairs_user_and_ai_into_one_line() {
        let msgs = vec![user("balance?"), ai("$1,500.00")];
        let s = compress_history_pairs(&msgs);
        assert_eq!(s, "User asked: balance?. AI answered: $1,500.00.");
    }

    #[test]
    fn truncates_long_messages_with_ellipsis() {
        let long = "x".repeat(200);
        let msgs = vec![user(&long), ai("ok")];
        let s = compress_history_pairs(&msgs);
        // 80 char snippet + ellipsis + suffix
        assert!(s.contains(&"x".repeat(80)));
        assert!(s.contains('…'));
        assert!(s.contains("AI answered: ok"));
    }

    #[test]
    fn handles_trailing_unanswered_user_turn() {
        let msgs = vec![user("hi"), ai("hello"), user("balance?")];
        let s = compress_history_pairs(&msgs);
        assert!(s.contains("User asked: hi. AI answered: hello."));
        assert!(s.ends_with("User asked: balance?."));
    }

    #[test]
    fn handles_bare_assistant_message() {
        // Edge case: an AI turn with no preceding user turn (e.g. proactive).
        let msgs = vec![ai("Heads up!"), user("ok")];
        let s = compress_history_pairs(&msgs);
        assert!(s.starts_with("AI said: Heads up!."));
        assert!(s.contains("User asked: ok."));
    }

    #[test]
    fn empty_input_yields_empty_string() {
        assert_eq!(compress_history_pairs(&[]), "");
    }

    /// T-068: Record intents must keep envelope detail (the prompt
    /// references envelopes by name); everything else drops it.
    #[test]
    fn scope_for_record_intents_is_full() {
        assert_eq!(scope_for(IntentKind::RecordExpense), SnapshotScope::Full);
        assert_eq!(scope_for(IntentKind::RecordIncome), SnapshotScope::Full);
    }

    #[test]
    fn scope_for_non_record_intents_skips_envelopes() {
        for kind in [
            IntentKind::QueryBalance,
            IntentKind::QueryHistory,
            IntentKind::BudgetManagement,
            IntentKind::CorrectTransaction,
            IntentKind::AccountManagement,
            IntentKind::GeneralQuestion,
        ] {
            assert_eq!(scope_for(kind), SnapshotScope::QueryBalance, "{kind:?}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::BuiltPrompt;
    use crate::core::proposal::{ProposedLine, Side, TransactionProposal};
    use crate::db::connection::create_encrypted_db;
    use crate::id::new_ulid;
    use async_trait::async_trait;
    use std::sync::Mutex;
    use tempfile::tempdir;

    struct MockAdapter {
        proposal: Mutex<TransactionProposal>,
        captured: Mutex<Option<BuiltPrompt>>,
    }

    impl MockAdapter {
        fn new(proposal: TransactionProposal) -> Self {
            Self { proposal: Mutex::new(proposal), captured: Mutex::new(None) }
        }
    }

    #[async_trait]
    impl AiAdapter for MockAdapter {
        async fn propose(&self, prompt: &BuiltPrompt) -> Result<ProposeResult, AdapterError> {
            *self.captured.lock().unwrap() = Some(prompt.clone());
            Ok(ProposeResult {
                proposal: self.proposal.lock().unwrap().clone(),
                usage: AiUsage::default(),
            })
        }
    }

    async fn test_pool_with_household() -> (SqlitePool, String) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("orch.db");
        let pool = create_encrypted_db(&path, "pw", &[0u8; 16]).await.unwrap();
        let hid = new_ulid();
        sqlx::query(
            "INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'H', 'UTC', 0)",
        )
        .bind(&hid)
        .execute(&pool)
        .await
        .unwrap();
        // Seed two accounts + an equity counterpart so balance queries produce
        // non-empty output and asset-balance validation passes.
        sqlx::query(
            "INSERT INTO accounts (id, household_id, name, type, normal_balance, is_placeholder, created_at)
             VALUES ('acc_chk', ?, 'Checking', 'asset', 'debit', 0, 0),
                    ('acc_grc', ?, 'Groceries', 'expense', 'debit', 0, 0),
                    ('acc_eq',  ?, 'Equity',   'equity', 'credit', 0, 0)",
        )
        .bind(&hid)
        .bind(&hid)
        .bind(&hid)
        .execute(&pool)
        .await
        .unwrap();
        // Opening balance so Checking has funds to spend against.
        sqlx::query(
            "INSERT INTO transactions (id, household_id, txn_date, entry_date, status, source, created_at)
             VALUES ('txn_ob', ?, 0, 0, 'posted', 'opening_balance', 0)",
        )
        .bind(&hid)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO journal_lines (id, transaction_id, account_id, amount, side, created_at)
             VALUES ('jl_ob_1', 'txn_ob', 'acc_chk', 100000, 'debit',  0),
                    ('jl_ob_2', 'txn_ob', 'acc_eq',  100000, 'credit', 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        std::mem::forget(dir);
        (pool, hid)
    }

    fn groceries_proposal() -> TransactionProposal {
        TransactionProposal {
            memo: Some("Coffee".to_string()),
            txn_date_ms: now_ms(),
            lines: vec![
                ProposedLine {
                    account_id: "acc_grc".to_string(),
                    envelope_id: None,
                    amount_cents: 450,
                    side: Side::Debit,
                },
                ProposedLine {
                    account_id: "acc_chk".to_string(),
                    envelope_id: None,
                    amount_cents: 450,
                    side: Side::Credit,
                },
            ],
        }
    }

    #[tokio::test]
    async fn query_balance_returns_snapshot_text_without_calling_adapter() {
        let (pool, hid) = test_pool_with_household().await;
        let adapter = Arc::new(MockAdapter::new(groceries_proposal()));
        let orch = Orchestrator::new(pool, adapter.clone());

        let resp = orch.handle(&hid, "what's my balance?").await.unwrap();
        match resp {
            MessageResponse::Text { text } => {
                assert!(text.contains("Financial Snapshot"), "expected snapshot text, got: {text}");
            }
            _ => panic!("expected text response"),
        }
        assert!(adapter.captured.lock().unwrap().is_none(), "adapter should not be called");
    }

    #[tokio::test]
    async fn record_expense_routes_through_adapter_and_returns_proposal() {
        let (pool, hid) = test_pool_with_household().await;
        let adapter = Arc::new(MockAdapter::new(groceries_proposal()));
        let orch = Orchestrator::new(pool, adapter.clone());

        let resp = orch.handle(&hid, "I spent $4.50 on coffee").await.unwrap();
        match resp {
            MessageResponse::Proposal { proposal, account_names, .. } => {
                assert_eq!(proposal.lines.len(), 2);
                assert_eq!(proposal.memo.as_deref(), Some("Coffee"));
                assert_eq!(account_names.get("acc_chk").map(|s| s.as_str()), Some("Checking"));
                assert_eq!(account_names.get("acc_grc").map(|s| s.as_str()), Some("Groceries"));
            }
            _ => panic!("expected proposal response"),
        }
        let captured = adapter.captured.lock().unwrap();
        let prompt = captured.as_ref().expect("adapter should be called");
        assert!(prompt.system.contains("Tally"), "base prompt in system");
        assert!(prompt.system.contains("Financial Snapshot"), "snapshot in system");
        // The user's current turn is the last message in the prompt.
        let last = prompt.messages.last().expect("at least one message");
        assert_eq!(last.role, Role::User);
        assert!(last.content.contains("coffee"));
    }

    #[tokio::test]
    async fn record_expense_includes_validation_result_in_response() {
        let (pool, hid) = test_pool_with_household().await;
        let adapter = Arc::new(MockAdapter::new(groceries_proposal()));
        let orch = Orchestrator::new(pool, adapter);

        let resp = orch.handle(&hid, "I spent $4.50 on coffee").await.unwrap();
        match resp {
            MessageResponse::Proposal { validation, .. } => {
                // The mock proposal references real seeded accounts and balances,
                // so validation should accept it.
                assert!(validation.is_accepted(), "expected ACCEPTED, got {:?}", validation);
            }
            _ => panic!("expected proposal"),
        }
    }

    #[tokio::test]
    async fn unsupported_intents_return_a_placeholder_text() {
        let (pool, hid) = test_pool_with_household().await;
        let adapter = Arc::new(MockAdapter::new(groceries_proposal()));
        let orch = Orchestrator::new(pool, adapter.clone());

        for input in [
            "show me my recent transactions",
            "how much is left in my grocery envelope",
            "fix that last transaction",
            "tell me a joke",
        ] {
            let resp = orch.handle(&hid, input).await.unwrap();
            match resp {
                MessageResponse::Text { text } => {
                    assert!(text.contains("isn't wired up yet"), "input: {input} got: {text}");
                }
                _ => panic!("expected text for: {input}"),
            }
        }
        assert!(adapter.captured.lock().unwrap().is_none());
    }

    #[tokio::test]
    async fn prior_chat_history_is_fed_into_the_prompt() {
        let (pool, hid) = test_pool_with_household().await;
        let repo = ChatRepo::new(pool.clone());
        // Seed two prior turns.
        repo.append(&hid, &new_ulid(), "user", r#"{"text":"earlier question"}"#, 1000, 1000)
            .await
            .unwrap();
        repo.append(&hid, &new_ulid(), "ai", r#"{"text":"earlier answer"}"#, 2000, 2000)
            .await
            .unwrap();

        let adapter = Arc::new(MockAdapter::new(groceries_proposal()));
        let orch = Orchestrator::new(pool, adapter.clone());
        orch.handle(&hid, "I spent $4.50 on coffee").await.unwrap();

        let captured = adapter.captured.lock().unwrap();
        let prompt = captured.as_ref().unwrap();
        let contents: Vec<String> = prompt.messages.iter().map(|m| m.content.clone()).collect();
        let joined = contents.join(" | ");
        assert!(joined.contains("earlier question"), "history missing: {joined}");
        assert!(joined.contains("earlier answer"), "history missing: {joined}");
        // The current turn still comes last.
        let last = prompt.messages.last().unwrap();
        assert_eq!(last.role, Role::User);
        assert!(last.content.contains("coffee"));
    }
}
