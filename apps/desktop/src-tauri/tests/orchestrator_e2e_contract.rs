//! Contract test for the request/response shapes the Playwright E2E harness
//! mocks. Exercises the real Rust pipeline end-to-end:
//!
//!   submit_message ─→ orchestrator ─→ proposal
//!     ↓
//!   commit_proposal (ledger) ─→ posted txn
//!     ↓
//!   undo_last_transaction (correction) ─→ reversal
//!
//! If a refactor changes the JSON shape produced by `MessageResponse::Proposal`
//! or the validation/commit/undo flow, this test fails — flagging that
//! `apps/desktop/e2e/fixtures/responses.ts` needs to follow.

use std::sync::Arc;

use async_trait::async_trait;
use tally_desktop_lib::ai::adapter::{AdapterError, AiAdapter, AiUsage, ProposeResult};
use tally_desktop_lib::ai::orchestrator::{MessageResponse, Orchestrator};
use tally_desktop_lib::ai::BuiltPrompt;
use tally_desktop_lib::commands::stamp_ai_usage;
use tally_desktop_lib::core::correction::undo_last_transaction;
use tally_desktop_lib::core::ledger::commit_proposal;
use tally_desktop_lib::core::proposal::{ProposedLine, Side, TransactionProposal};
use tally_desktop_lib::db::create_encrypted_db;
use tally_desktop_lib::id::new_ulid;
use tempfile::tempdir;

struct FixedProposalAdapter {
    proposal: TransactionProposal,
}

#[async_trait]
impl AiAdapter for FixedProposalAdapter {
    async fn propose(&self, _prompt: &BuiltPrompt) -> Result<ProposeResult, AdapterError> {
        Ok(ProposeResult {
            proposal: self.proposal.clone(),
            usage: AiUsage::default(),
        })
    }
}

async fn seed_household() -> (sqlx::SqlitePool, String) {
    let dir = tempdir().unwrap();
    let path = dir.path().join("contract.db");
    let pool = create_encrypted_db(&path, "pp", &[0u8; 16]).await.unwrap();
    let hh_id = new_ulid();

    sqlx::query(
        "INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'H', 'UTC', 0)",
    )
    .bind(&hh_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO accounts (id, household_id, name, type, normal_balance, is_placeholder, created_at)
         VALUES ('acc_chk', ?, 'Checking',  'asset',   'debit',  0, 0),
                ('acc_grc', ?, 'Groceries', 'expense', 'debit',  0, 0),
                ('acc_eq',  ?, 'Equity',    'equity',  'credit', 0, 0)",
    )
    .bind(&hh_id).bind(&hh_id).bind(&hh_id)
    .execute(&pool).await.unwrap();

    // Opening balance for Checking so the snapshot reports non-zero funds.
    sqlx::query(
        "INSERT INTO transactions (id, household_id, txn_date, entry_date, status, source, created_at)
         VALUES ('txn_ob', ?, 0, 0, 'posted', 'opening_balance', 0)",
    )
    .bind(&hh_id).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO journal_lines (id, transaction_id, account_id, amount, side, created_at)
         VALUES ('jl_ob_1', 'txn_ob', 'acc_chk', 100000, 'debit',  0),
                ('jl_ob_2', 'txn_ob', 'acc_eq',  100000, 'credit', 0)",
    )
    .execute(&pool).await.unwrap();

    std::mem::forget(dir);
    (pool, hh_id)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn coffee_proposal() -> TransactionProposal {
    TransactionProposal {
        memo: Some("Coffee".to_string()),
        txn_date_ms: now_ms(),
        lines: vec![
            ProposedLine {
                account_id: "acc_grc".to_string(),
                envelope_id: None,
                amount_cents: 500,
                side: Side::Debit,
            },
            ProposedLine {
                account_id: "acc_chk".to_string(),
                envelope_id: None,
                amount_cents: 500,
                side: Side::Credit,
            },
        ],
    }
}

#[tokio::test]
async fn submit_commit_undo_round_trip_matches_e2e_mock_shape() {
    let (pool, hh_id) = seed_household().await;
    let adapter = Arc::new(FixedProposalAdapter { proposal: coffee_proposal() });
    let orchestrator = Orchestrator::new(pool.clone(), adapter);

    // 1) submit_message — orchestrator returns Proposal with account_names map.
    let resp = orchestrator
        .handle(&hh_id, "I spent $5 on coffee at Blue Bottle")
        .await
        .expect("orchestrator handle");

    let (proposal, account_names, validation, proactive_insights) = match resp {
        MessageResponse::Proposal {
            proposal,
            account_names,
            validation,
            proactive_insights,
            ..
        } => (proposal, account_names, validation, proactive_insights),
        MessageResponse::Text { text } => panic!("expected Proposal, got Text: {text}"),
    };
    // No prior posts → no duplicate insight expected on this turn.
    assert!(
        proactive_insights.is_empty(),
        "first turn should not raise a duplicate insight, got: {proactive_insights:?}",
    );

    // The shape used by `proposalResponse(...)` in e2e/fixtures/responses.ts.
    assert_eq!(proposal.lines.len(), 2);
    assert_eq!(proposal.memo.as_deref(), Some("Coffee"));
    assert_eq!(account_names.get("acc_chk").map(|s| s.as_str()), Some("Checking"));
    assert_eq!(account_names.get("acc_grc").map(|s| s.as_str()), Some("Groceries"));
    assert!(validation.is_accepted(), "expected validation to accept, got: {validation:?}");

    // The Proposal shape must serialize to the same JSON keys the E2E mock
    // hard-codes (camel-cased Rust → snake_case JSON via serde derives).
    let json = serde_json::to_value(&proposal).unwrap();
    assert!(json.get("memo").is_some());
    assert!(json.get("txn_date_ms").is_some());
    let lines = json.get("lines").and_then(|v| v.as_array()).expect("lines array");
    let first = &lines[0];
    assert!(first.get("account_id").is_some());
    assert!(first.get("amount_cents").is_some());
    assert!(first.get("side").is_some());

    // 2) commit_proposal — real ledger writes the transaction.
    let txn_id = commit_proposal(&pool, &hh_id, &proposal)
        .await
        .expect("commit_proposal");
    assert_eq!(txn_id.len(), 26, "commit returns a ULID");

    let (status,): (String,) = sqlx::query_as("SELECT status FROM transactions WHERE id = ?")
        .bind(&txn_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "posted");

    // 2a) T-069: stamping the row with AiUsage round-trips into the new
    // ai_input_tokens / ai_output_tokens / ai_cache_hit columns. Drives
    // the same helper the Tauri commit_proposal wrapper calls.
    stamp_ai_usage(
        &pool,
        &txn_id,
        &AiUsage {
            input_tokens: 1234,
            output_tokens: 56,
            cache_hit: true,
        },
    )
    .await
    .expect("stamp_ai_usage");
    let (in_tokens, out_tokens, cache_hit): (i64, i64, i64) = sqlx::query_as(
        "SELECT ai_input_tokens, ai_output_tokens, ai_cache_hit FROM transactions WHERE id = ?",
    )
    .bind(&txn_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(in_tokens, 1234);
    assert_eq!(out_tokens, 56);
    assert_eq!(cache_hit, 1);

    // 3) undo_last_transaction — wrapper-equivalent path used by /undo.
    let reversal_id = undo_last_transaction(&pool, &hh_id)
        .await
        .expect("undo_last_transaction");
    assert_eq!(reversal_id.len(), 26);

    let (after_status,): (String,) = sqlx::query_as("SELECT status FROM transactions WHERE id = ?")
        .bind(&txn_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(after_status, "void", "original txn must be voided after undo");

    // The reversal must have flipped sides on every line.
    let line_sides: Vec<(String,)> = sqlx::query_as(
        "SELECT side FROM journal_lines WHERE transaction_id = ? ORDER BY side",
    )
    .bind(&reversal_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    let sides: Vec<&str> = line_sides.iter().map(|(s,)| s.as_str()).collect();
    assert!(sides.contains(&"credit"));
    assert!(sides.contains(&"debit"));
}

#[tokio::test]
async fn undo_with_no_history_returns_not_found() {
    let (pool, hh_id) = seed_household().await;

    // Only the opening_balance transaction exists; undo must skip it.
    let err = undo_last_transaction(&pool, &hh_id)
        .await
        .expect_err("undo should fail with no posted txns");

    let msg = format!("{err}");
    assert!(
        msg.contains("not found") || msg.contains("not posted"),
        "unexpected error: {msg}",
    );
}
