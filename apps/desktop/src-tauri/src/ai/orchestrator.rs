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

use crate::ai::adapter::{AdapterError, AiAdapter};
use crate::ai::classifier::{classify, IntentKind};
use crate::ai::payee_memory::PayeeMemory;
use crate::ai::prompt::PromptBuilder;
use crate::ai::snapshot::build_snapshot;
use crate::ai::{Message, Role};
use crate::chat::ChatRepo;
use crate::core::proposal::TransactionProposal;
use crate::core::validation::{validate_proposal, AIAdvisory, ValidationResult};

const BASE_SYSTEM_PROMPT: &str = "\
You are Tally, a household finance assistant. The user chats with you naturally about their money; \
you log transactions, answer balance questions, and keep their books. All money is stored as integer cents. \
Every transaction is double-entry: debits and credits must balance. Account IDs are 26-character ULIDs — \
always pick IDs from the financial snapshot below; never invent one. When the user describes a transaction, \
call the submit_transaction_proposal tool with the structured proposal. Keep text replies short and direct — \
no hedging, no recap, no markdown.";

const HISTORY_FETCH_LIMIT: i64 = 40;
const PROMPT_HISTORY_LIMIT: usize = 20;
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
        let snapshot = build_snapshot(&self.pool, household_id, now).await?;

        match intent.kind {
            IntentKind::QueryBalance => Ok(MessageResponse::Text { text: snapshot.to_prompt_text() }),

            IntentKind::RecordExpense | IntentKind::RecordIncome => {
                let history = self.load_prompt_history(household_id).await?;
                let hints = self.payee_memory.top_hints(household_id, PAYEE_MEMORY_HINTS).await;

                let mut prompt = PromptBuilder::new(
                    BASE_SYSTEM_PROMPT,
                    snapshot.to_prompt_text_with_ids(),
                )
                .with_intent(intent)
                .with_history(history)
                .with_memory(hints)
                .build();
                prompt.messages.push(Message::user(user_text.to_string()));

                let proposal = self.adapter.propose(&prompt).await?;
                let validation = validate_proposal(&self.pool, &proposal).await;
                let account_names = lookup_account_names(&self.pool, &proposal).await?;
                Ok(MessageResponse::Proposal {
                    proposal,
                    validation,
                    advisories: Vec::new(),
                    account_names,
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

    async fn load_prompt_history(&self, household_id: &str) -> Result<Vec<Message>, OrchestratorError> {
        let rows = self.chat.list_before(household_id, i64::MAX, HISTORY_FETCH_LIMIT).await?;
        // Rows are newest-first; filter to conversational turns, then reverse for chronological.
        let mut history: Vec<Message> = rows
            .into_iter()
            .filter_map(|row| match row.kind.as_str() {
                "user" => extract_text(&row.payload).map(|t| Message { role: Role::User, content: t }),
                "ai" => extract_text(&row.payload).map(|t| Message { role: Role::Assistant, content: t }),
                _ => None,
            })
            .collect();
        history.reverse();
        // Keep only the most recent N turns so the prompt doesn't balloon.
        let start = history.len().saturating_sub(PROMPT_HISTORY_LIMIT);
        Ok(history[start..].to_vec())
    }
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
        async fn propose(&self, prompt: &BuiltPrompt) -> Result<TransactionProposal, AdapterError> {
            *self.captured.lock().unwrap() = Some(prompt.clone());
            Ok(self.proposal.lock().unwrap().clone())
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
