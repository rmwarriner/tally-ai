// Tauri command handlers — thin wrappers delegating to core/ai layers
//
// State model: AppState holds the live SqlitePool after the household is created
// or unlocked. Commands that require a DB connection read from that state.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager, State};

use crate::ai::adapter::claude::ClaudeAdapter;
use crate::ai::orchestrator::{MessageResponse, Orchestrator};
use crate::chat::{ChatMessageRow, ChatRepo};
use crate::core::coa::seed_chart_of_accounts;
use crate::ai::adapter::AiUsage;
use crate::core::correction::{undo_last_transaction as core_undo, CorrectionError};
use crate::core::insight::{day_bucket_ms, log_if_new, should_emit, InsightKind, ProactiveInsight, Sensitivity};
use crate::core::ledger::{commit_proposal as ledger_commit, LedgerError};
use crate::core::proposal::TransactionProposal;
use crate::core::triggers::briefing as briefing_trigger;
use crate::core::triggers::envelope as envelope_triggers;
use crate::core::validation::ValidationResult;
use crate::db::create_encrypted_db;
use crate::error::{NonEmpty, RecoveryAction, RecoveryError, RecoveryKind};
use crate::id::new_ulid;
use crate::secrets::{
    delete_claude_api_key, has_claude_api_key, load_claude_api_key, save_claude_api_key,
    KeyringStore,
};

// ── App state ────────────────────────────────────────────────────────────────

pub struct AppState {
    pub pool: Mutex<Option<SqlitePool>>,
    pub household_id: Mutex<Option<String>>,
    pub active_import: Mutex<Option<crate::core::import::gnucash::ImportPlan>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            pool: Mutex::new(None),
            household_id: Mutex::new(None),
            active_import: Mutex::new(None),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn db_path(app: &AppHandle) -> Result<PathBuf, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve app data dir: {e}"))?;
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("Cannot create data dir: {e}"))?;
    Ok(data_dir.join("tally.db"))
}

fn salt_path(app: &AppHandle) -> Result<PathBuf, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve app data dir: {e}"))?;
    Ok(data_dir.join("tally.salt"))
}


fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time before epoch")
        .as_millis() as i64
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Returns true if a household already exists (DB file + household row present).
#[tauri::command]
pub async fn check_setup_status(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, RecoveryError> {
    let path = db_path(&app).map_err(RecoveryError::show_help)?;
    if !path.exists() {
        return Ok(false);
    }
    let sp = salt_path(&app).map_err(RecoveryError::show_help)?;
    if !sp.exists() {
        return Ok(false);
    }
    // Clone pool out of lock so the guard is dropped before any await
    let pool_opt = state.pool.lock().expect("pool lock").clone();
    if let Some(pool) = pool_opt {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM households")
            .fetch_one(&pool)
            .await
            .map_err(|e| RecoveryError::show_help(e.to_string()))?;
        return Ok(count.0 > 0);
    }
    // DB file exists but pool isn't open — household exists (needs unlock flow).
    Ok(true)
}

#[derive(Deserialize)]
pub struct CreateHouseholdArgs {
    pub name: String,
    pub timezone: String,
    pub passphrase: String,
}

/// Creates the encrypted database, seeds CoA, and inserts the household row.
/// Returns the new household ULID.
#[tauri::command]
pub async fn create_household(
    app: AppHandle,
    state: State<'_, AppState>,
    args: CreateHouseholdArgs,
) -> Result<String, RecoveryError> {
    let path = db_path(&app).map_err(RecoveryError::show_help)?;
    let sp = salt_path(&app).map_err(RecoveryError::show_help)?;

    // Generate a fresh random 16-byte salt via the rand crate's CSPRNG (#142).
    // Replaces the prior DefaultHasher-of-(time, pid) construction, which was
    // predictable enough to undermine Argon2id's defense against rainbow tables.
    let salt: [u8; 16] = crate::crypto::generate_salt();
    std::fs::write(&sp, salt)
        .map_err(|e| RecoveryError::show_help(format!("Cannot write salt: {e}")))?;

    let pool = create_encrypted_db(&path, &args.passphrase, &salt)
        .await
        .map_err(|e| {
            // Wrong passphrase / DB-open failure is recoverable by retyping the
            // passphrase, so surface EditField as the primary action.
            RecoveryError::new(
                format!("Could not open the database: {e}"),
                NonEmpty::new(
                    RecoveryAction {
                        kind: RecoveryKind::EditField,
                        label: "Re-enter passphrase".to_string(),
                        is_primary: true,
                    },
                    vec![RecoveryAction {
                        kind: RecoveryKind::Discard,
                        label: "Discard".to_string(),
                        is_primary: false,
                    }],
                ),
            )
        })?;

    let household_id = new_ulid();
    let ts = now_ms();

    sqlx::query(
        "INSERT INTO households (id, name, timezone, schema_version, created_at)
         VALUES (?, ?, ?, 1, ?)",
    )
    .bind(&household_id)
    .bind(&args.name)
    .bind(&args.timezone)
    .bind(ts)
    .execute(&pool)
    .await
    .map_err(|e| RecoveryError::show_help(e.to_string()))?;

    // Seed owner user
    let user_id = new_ulid();
    sqlx::query(
        "INSERT INTO users (id, household_id, display_name, role, is_active, created_at)
         VALUES (?, ?, ?, 'owner', 1, ?)",
    )
    .bind(&user_id)
    .bind(&household_id)
    .bind(&args.name)
    .bind(ts)
    .execute(&pool)
    .await
    .map_err(|e| RecoveryError::show_help(e.to_string()))?;

    // Seed chart of accounts
    seed_chart_of_accounts(&pool, &household_id)
        .await
        .map_err(|e| RecoveryError::show_help(e.to_string()))?;

    // Store pool and household id in state
    *state.pool.lock().expect("pool lock") = Some(pool);
    *state.household_id.lock().expect("household_id lock") = Some(household_id.clone());

    Ok(household_id)
}

#[derive(Deserialize)]
pub struct CreateAccountArgs {
    pub name: String,
    pub account_type: String,
}

/// Creates a new leaf account under the household.
/// Returns the new account ULID.
#[tauri::command]
pub async fn create_account(
    state: State<'_, AppState>,
    args: CreateAccountArgs,
) -> Result<String, RecoveryError> {
    let pool = state
        .pool
        .lock()
        .expect("pool lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Database not open"))?;
    let household_id = state
        .household_id
        .lock()
        .expect("household_id lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Household not set"))?;

    let normal_balance = match args.account_type.as_str() {
        "asset" | "expense" => "debit",
        _ => "credit",
    };

    // Find the parent placeholder account matching account_type
    let parent: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM accounts WHERE household_id = ? AND type = ? AND is_placeholder = 1 AND parent_id IS NOT NULL LIMIT 1",
    )
    .bind(&household_id)
    .bind(&args.account_type)
    .fetch_optional(&pool)
    .await
    .map_err(|e| RecoveryError::show_help(e.to_string()))?;

    let account_id = new_ulid();
    let ts = now_ms();

    sqlx::query(
        "INSERT INTO accounts (id, household_id, parent_id, name, type, normal_balance, is_placeholder, currency, created_at)
         VALUES (?, ?, ?, ?, ?, ?, 0, 'USD', ?)",
    )
    .bind(&account_id)
    .bind(&household_id)
    .bind(parent.map(|(id,)| id))
    .bind(&args.name)
    .bind(&args.account_type)
    .bind(normal_balance)
    .bind(ts)
    .execute(&pool)
    .await
    .map_err(|e| RecoveryError::show_help(e.to_string()))?;

    Ok(account_id)
}

#[derive(Deserialize)]
pub struct SetOpeningBalanceArgs {
    pub account_id: String,
    pub amount_cents: i64,
}

/// Records an opening balance journal entry for the given account.
#[tauri::command]
pub async fn set_opening_balance(
    state: State<'_, AppState>,
    args: SetOpeningBalanceArgs,
) -> Result<(), RecoveryError> {
    let pool = state
        .pool
        .lock()
        .expect("pool lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Database not open"))?;
    let household_id = state
        .household_id
        .lock()
        .expect("household_id lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Household not set"))?;

    if args.amount_cents == 0 {
        return Ok(());
    }

    // Resolve the Opening Balance Equity account
    let obe: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM accounts WHERE household_id = ? AND name = 'Opening Balance Equity' LIMIT 1",
    )
    .bind(&household_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| RecoveryError::show_help(e.to_string()))?;

    let obe_id = obe
        .map(|(id,)| id)
        .ok_or_else(|| RecoveryError::show_help("Opening Balance Equity account not found"))?;

    // Get account type to determine debit/credit side
    let account_type: (String,) =
        sqlx::query_as("SELECT type FROM accounts WHERE id = ?")
            .bind(&args.account_id)
            .fetch_one(&pool)
            .await
            .map_err(|e| RecoveryError::show_help(e.to_string()))?;

    let (primary_side, equity_side) = match account_type.0.as_str() {
        "asset" => ("debit", "credit"),
        "liability" => ("credit", "debit"),
        _ => ("debit", "credit"),
    };

    let txn_id = new_ulid();
    let line1_id = new_ulid();
    let line2_id = new_ulid();
    let ts = now_ms();

    // #143: wrap the three writes + audit row in one SQL transaction so a
    // partial failure doesn't leave the ledger inconsistent.
    let mut sql_tx = pool
        .begin()
        .await
        .map_err(|e| RecoveryError::show_help(e.to_string()))?;

    sqlx::query(
        "INSERT INTO transactions (id, household_id, txn_date, entry_date, status, source, memo, created_at)
         VALUES (?, ?, ?, ?, 'posted', 'opening_balance', 'Opening balance', ?)",
    )
    .bind(&txn_id)
    .bind(&household_id)
    .bind(ts)
    .bind(ts)
    .bind(ts)
    .execute(&mut *sql_tx)
    .await
    .map_err(|e| RecoveryError::show_help(e.to_string()))?;

    sqlx::query(
        "INSERT INTO journal_lines (id, transaction_id, account_id, amount, side, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&line1_id)
    .bind(&txn_id)
    .bind(&args.account_id)
    .bind(args.amount_cents)
    .bind(primary_side)
    .bind(ts)
    .execute(&mut *sql_tx)
    .await
    .map_err(|e| RecoveryError::show_help(e.to_string()))?;

    sqlx::query(
        "INSERT INTO journal_lines (id, transaction_id, account_id, amount, side, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&line2_id)
    .bind(&txn_id)
    .bind(&obe_id)
    .bind(args.amount_cents)
    .bind(equity_side)
    .bind(ts)
    .execute(&mut *sql_tx)
    .await
    .map_err(|e| RecoveryError::show_help(e.to_string()))?;

    crate::core::audit::write(
        &mut sql_tx,
        &household_id,
        "transactions",
        &txn_id,
        crate::core::audit::AuditAction::Insert,
        &serde_json::json!({
            "memo": "Opening balance",
            "source": "opening_balance",
            "txn_date_ms": ts,
            "lines": [
                { "account_id": args.account_id, "amount_cents": args.amount_cents, "side": primary_side },
                { "account_id": obe_id, "amount_cents": args.amount_cents, "side": equity_side },
            ],
        }),
        ts,
    )
    .await
    .map_err(|e| RecoveryError::show_help(e.to_string()))?;

    sql_tx
        .commit()
        .await
        .map_err(|e| RecoveryError::show_help(e.to_string()))?;

    Ok(())
}

#[derive(Deserialize)]
pub struct CreateEnvelopeArgs {
    pub name: String,
}

/// Creates a new envelope and seeds a current-month envelope_periods row.
/// Returns the new envelope ULID.
#[tauri::command]
pub async fn create_envelope(
    state: State<'_, AppState>,
    args: CreateEnvelopeArgs,
) -> Result<String, RecoveryError> {
    let pool = state
        .pool
        .lock()
        .expect("pool lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Database not open"))?;
    let household_id = state
        .household_id
        .lock()
        .expect("household_id lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Household not set"))?;

    crate::core::envelope::create_envelope_with_current_period(
        &pool,
        &household_id,
        &args.name,
        now_ms(),
    )
    .await
    .map_err(RecoveryError::show_help)
}

#[derive(Deserialize)]
pub struct ImportHledgerArgs {
    pub content: String,
}

#[derive(Serialize)]
pub struct ImportSummary {
    pub message: String,
}

/// Stub hledger import — TODO(phase2): full parser with CoA mapping.
/// Currently counts non-comment, non-blank lines and returns a summary.
#[tauri::command]
pub async fn import_hledger(args: ImportHledgerArgs) -> Result<String, RecoveryError> {
    let lines: Vec<&str> = args
        .content
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with(';'))
        .collect();

    // Very basic heuristic: count transaction header lines (lines starting with a date)
    let txn_count = lines
        .iter()
        .filter(|l| l.starts_with(|c: char| c.is_ascii_digit()))
        .count();

    Ok(format!(
        "{} transaction{} found — full CoA mapping will be available in Phase 2",
        txn_count,
        if txn_count == 1 { "" } else { "s" }
    ))
}

/// Returns AI defaults (timezone, default account). Stub for Phase 1.
#[tauri::command]
pub async fn get_ai_defaults(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, RecoveryError> {
    let hh_guard = state.household_id.lock().expect("household_id lock");
    if hh_guard.is_none() {
        return Ok(serde_json::json!({ "status": "No household configured" }));
    }
    // TODO(phase2): read from a persisted defaults table
    Ok(serde_json::json!({
        "timezone": "configured during setup",
        "payment_account": "Checking"
    }))
}

/// Reverses the most recently posted, non-opening-balance transaction.
/// Returns the new reversal ULID on success.
#[tauri::command]
pub async fn undo_last_transaction(state: State<'_, AppState>) -> Result<String, RecoveryError> {
    let pool = state
        .pool
        .lock()
        .expect("pool lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Database not open"))?;
    let household_id = state
        .household_id
        .lock()
        .expect("household_id lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Household not set"))?;

    core_undo(&pool, &household_id).await.map_err(map_undo_error)
}

fn map_undo_error(err: CorrectionError) -> RecoveryError {
    match err {
        CorrectionError::NotFound => {
            RecoveryError::discard("Nothing to undo. No posted transactions to reverse.")
        }
        CorrectionError::NotCorrectable => {
            RecoveryError::show_help("That transaction can't be undone.")
        }
        CorrectionError::ValidationFailed(_) => {
            RecoveryError::show_help("Couldn't undo that transaction.")
        }
        CorrectionError::Database(e) => {
            RecoveryError::show_help(format!("Database error while undoing: {e}"))
        }
    }
}

/// Returns the household's current proactive-engine sensitivity setting.
/// One of `quiet | normal | proactive`.
#[tauri::command]
pub async fn get_sensitivity(state: State<'_, AppState>) -> Result<String, RecoveryError> {
    let pool = state
        .pool
        .lock()
        .expect("pool lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Database not open"))?;
    let household_id = state
        .household_id
        .lock()
        .expect("household_id lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Household not set"))?;

    let (raw,): (String,) =
        sqlx::query_as("SELECT sensitivity FROM households WHERE id = ?")
            .bind(&household_id)
            .fetch_one(&pool)
            .await
            .map_err(|e| RecoveryError::show_help(e.to_string()))?;

    // Normalize through Sensitivity::parse so callers always see a valid value.
    let sensitivity = Sensitivity::parse(&raw).unwrap_or(Sensitivity::Normal);
    Ok(sensitivity.as_str().to_string())
}

#[derive(Deserialize)]
pub struct SetSensitivityArgs {
    pub sensitivity: String,
}

/// Sets the household's sensitivity. Rejects unknown values with a plain
/// EditField recovery so the UI can re-prompt.
#[tauri::command]
pub async fn set_sensitivity(
    state: State<'_, AppState>,
    args: SetSensitivityArgs,
) -> Result<(), RecoveryError> {
    let pool = state
        .pool
        .lock()
        .expect("pool lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Database not open"))?;
    let household_id = state
        .household_id
        .lock()
        .expect("household_id lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Household not set"))?;

    let sensitivity = Sensitivity::parse(&args.sensitivity).ok_or_else(|| {
        RecoveryError::new(
            "Sensitivity must be one of: quiet, normal, proactive.".to_string(),
            crate::error::NonEmpty::new(
                RecoveryAction {
                    kind: RecoveryKind::EditField,
                    label: "Try again".to_string(),
                    is_primary: true,
                },
                vec![],
            ),
        )
    })?;

    sqlx::query("UPDATE households SET sensitivity = ? WHERE id = ?")
        .bind(sensitivity.as_str())
        .bind(&household_id)
        .execute(&pool)
        .await
        .map_err(|e| RecoveryError::show_help(e.to_string()))?;

    Ok(())
}

#[derive(Deserialize)]
pub struct AppendChatMessageArgs {
    pub id: String,
    pub kind: String,
    pub payload: String,
    pub ts: i64,
}

/// Persists one chat message to the thread history.
/// The client owns the ULID and the JSON payload; this command is a thin
/// append. Returns nothing — the UI renders optimistically and calls this
/// for durability, not for an echo.
#[tauri::command]
pub async fn append_chat_message(
    state: State<'_, AppState>,
    args: AppendChatMessageArgs,
) -> Result<(), RecoveryError> {
    let pool = state
        .pool
        .lock()
        .expect("pool lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Database not open"))?;
    let household_id = state
        .household_id
        .lock()
        .expect("household_id lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Household not set"))?;

    ChatRepo::new(pool)
        .append(&household_id, &args.id, &args.kind, &args.payload, args.ts, now_ms())
        .await
        .map_err(|e| RecoveryError::show_help(e.to_string()))
}

#[derive(Deserialize)]
pub struct ListChatMessagesArgs {
    pub before_ts: Option<i64>,
    pub limit: Option<i64>,
}

/// Returns up to `limit` messages with ts < `before_ts`, newest first.
/// Defaults: `before_ts = i64::MAX` (the tail), `limit = 50`.
#[tauri::command]
pub async fn list_chat_messages(
    state: State<'_, AppState>,
    args: ListChatMessagesArgs,
) -> Result<Vec<ChatMessageRow>, RecoveryError> {
    let pool = state
        .pool
        .lock()
        .expect("pool lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Database not open"))?;
    let household_id = state
        .household_id
        .lock()
        .expect("household_id lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Household not set"))?;

    let before_ts = args.before_ts.unwrap_or(i64::MAX);
    let limit = args.limit.unwrap_or(50).clamp(1, 500);

    ChatRepo::new(pool)
        .list_before(&household_id, before_ts, limit)
        .await
        .map_err(|e| RecoveryError::show_help(e.to_string()))
}

// ── Chat turn orchestration ───────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SubmitMessageArgs {
    pub text: String,
}

/// Processes one user chat turn and returns a structured response the UI can
/// render. For transaction intents this round-trips through Claude and returns
/// a `Proposal` variant; for balance queries it's answered from the snapshot.
#[tauri::command]
pub async fn submit_message(
    state: State<'_, AppState>,
    args: SubmitMessageArgs,
) -> Result<MessageResponse, RecoveryError> {
    let pool = state
        .pool
        .lock()
        .expect("pool lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Database not open"))?;
    let household_id = state
        .household_id
        .lock()
        .expect("household_id lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Household not set"))?;

    let api_key = load_claude_api_key(&KeyringStore::new())
        .map_err(|e| RecoveryError::show_help(e.to_string()))?
        .ok_or_else(|| {
            // Missing API key is recoverable by re-entering it.
            RecoveryError::new(
                "No Claude API key configured. Paste your key into chat when prompted, \
                 or set CLAUDE_API_KEY for development.",
                NonEmpty::new(
                    RecoveryAction {
                        kind: RecoveryKind::EditField,
                        label: "Enter API key".to_string(),
                        is_primary: true,
                    },
                    vec![RecoveryAction {
                        kind: RecoveryKind::ShowHelp,
                        label: "Get help".to_string(),
                        is_primary: false,
                    }],
                ),
            )
        })?;

    let adapter = Arc::new(ClaudeAdapter::new(api_key));
    let orchestrator = Orchestrator::new(pool, adapter);

    orchestrator
        .handle(&household_id, args.text.trim())
        .await
        .map_err(|e| RecoveryError::show_help(e.to_string()))
}

#[derive(Deserialize)]
pub struct CommitProposalArgs {
    pub proposal: TransactionProposal,
    /// Echoed back from the orchestrator's `MessageResponse::Proposal.ai_usage`
    /// (T-069). Optional because non-AI commit paths (future manual-mode,
    /// migrations, tests) won't have one. Persisted onto the resulting row.
    #[serde(default)]
    pub ai_usage: Option<AiUsage>,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CommitOutcome {
    Committed {
        txn_id: String,
        /// Proactive insights produced by post-commit triggers (e.g. envelope
        /// over budget). The UI renders each as a proactive chat message.
        /// Always present; empty if no triggers fired or sensitivity blocked.
        proactive_insights: Vec<ProactiveInsight>,
    },
    /// Validation rejected the proposal. The UI surfaces the errors from `validation`.
    Rejected {
        validation: ValidationResult,
    },
}

/// Validates the proposal and, if accepted, writes transaction + journal_lines.
/// Rejections return a `Rejected` outcome so the UI can render the error tier
/// without throwing. Database failures surface as an Err.
#[tauri::command]
pub async fn commit_proposal(
    state: State<'_, AppState>,
    args: CommitProposalArgs,
) -> Result<CommitOutcome, RecoveryError> {
    let pool = state
        .pool
        .lock()
        .expect("pool lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Database not open"))?;
    let household_id = state
        .household_id
        .lock()
        .expect("household_id lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Household not set"))?;

    match ledger_commit(&pool, &household_id, &args.proposal).await {
        Ok(txn_id) => {
            // Best-effort: a token-stamping failure must not roll back the
            // committed ledger entry — the columns are observability, not
            // load-bearing. Errors print and continue.
            if let Some(usage) = args.ai_usage.as_ref() {
                if let Err(e) = stamp_ai_usage(&pool, &txn_id, usage).await {
                    eprintln!("[commit_proposal] failed to stamp ai usage on {txn_id}: {e}");
                }
            }
            let proactive_insights = run_post_commit_triggers(&pool, &household_id).await;
            Ok(CommitOutcome::Committed {
                txn_id,
                proactive_insights,
            })
        }
        Err(LedgerError::ValidationFailed(result)) => {
            Ok(CommitOutcome::Rejected { validation: result })
        }
        Err(LedgerError::Database(e)) => Err(RecoveryError::show_help(e.to_string())),
        Err(LedgerError::OpeningBalanceExists) => {
            // The opening balance already exists; the user's recovery is to
            // discard this attempt rather than overwrite it.
            Err(RecoveryError::discard(
                "Opening balance already exists for this account.",
            ))
        }
    }
}

/// Runs every post-commit trigger and persists the surviving (sensitivity-
/// gated, deduped) insights. Failures are logged but never surfaced to the
/// user — a proactive insight is best-effort, not load-bearing.
async fn run_post_commit_triggers(
    pool: &sqlx::SqlitePool,
    household_id: &str,
) -> Vec<ProactiveInsight> {
    let now = now_ms();
    let (sensitivity, tz) = match load_sensitivity_and_tz(pool, household_id).await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[triggers] failed to load sensitivity/tz: {e}");
            return Vec::new();
        }
    };

    let drafts = match envelope_triggers::evaluate(pool, household_id, now).await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[triggers] envelope evaluator failed: {e}");
            return Vec::new();
        }
    };

    let mut emitted = Vec::with_capacity(drafts.len());
    for draft in drafts {
        if !should_emit(draft.kind, sensitivity) {
            continue;
        }
        match log_if_new(pool, household_id, &tz, now, draft).await {
            Ok(Some(insight)) => emitted.push(insight),
            Ok(None) => {} // dedup'd — fine
            Err(e) => eprintln!("[triggers] insight log failed: {e}"),
        }
    }
    emitted
}

/// Writes `ai_input_tokens` / `ai_output_tokens` / `ai_cache_hit` onto the
/// transaction row created by `commit_proposal`. Best-effort by contract;
/// see the call site for why we never fail the commit on a stamp error.
///
/// `pub` so the orchestrator contract test can drive the same SQL the
/// command wrapper uses, proving the column round-trip end-to-end.
pub async fn stamp_ai_usage(
    pool: &sqlx::SqlitePool,
    txn_id: &str,
    usage: &AiUsage,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE transactions
            SET ai_input_tokens  = ?,
                ai_output_tokens = ?,
                ai_cache_hit     = ?
          WHERE id = ?",
    )
    .bind(usage.input_tokens as i64)
    .bind(usage.output_tokens as i64)
    .bind(if usage.cache_hit { 1_i64 } else { 0_i64 })
    .bind(txn_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn load_sensitivity_and_tz(
    pool: &sqlx::SqlitePool,
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

/// Called once on App mount. Returns the morning briefing if (a) sensitivity
/// is `proactive`, (b) the local-day bucket exceeds `households.last_briefing_at_day`,
/// and (c) the singleton dedup hasn't already logged one. Otherwise returns
/// `null`. Best-effort: query failures yield `null`, never an error.
#[tauri::command]
pub async fn session_open(
    state: State<'_, AppState>,
) -> Result<Option<ProactiveInsight>, RecoveryError> {
    let pool = state
        .pool
        .lock()
        .expect("pool lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Database not open"))?;
    let household_id = state
        .household_id
        .lock()
        .expect("household_id lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Household not set"))?;

    let now = now_ms();
    let (sensitivity, tz) = match load_sensitivity_and_tz(&pool, &household_id).await {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };

    if !should_emit(InsightKind::MorningBriefing, sensitivity) {
        return Ok(None);
    }

    let today_bucket = match day_bucket_ms(&tz, now) {
        Ok(b) => b,
        Err(_) => return Ok(None),
    };

    let last_seen: Option<(Option<i64>,)> =
        sqlx::query_as("SELECT last_briefing_at_day FROM households WHERE id = ?")
            .bind(&household_id)
            .fetch_optional(&pool)
            .await
            .ok()
            .flatten();
    if let Some((Some(last),)) = last_seen {
        if last >= today_bucket {
            return Ok(None);
        }
    }

    let draft = match briefing_trigger::assemble(&pool, &household_id, now).await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[session_open] briefing assemble failed: {e}");
            return Ok(None);
        }
    };

    let insight = match log_if_new(&pool, &household_id, &tz, now, draft).await {
        Ok(Some(i)) => i,
        Ok(None) => return Ok(None), // already logged today
        Err(e) => {
            eprintln!("[session_open] insight log failed: {e}");
            return Ok(None);
        }
    };

    // Best-effort: even if this update fails, dedup still works via the
    // singleton partial index — the field is a fast-path early-out for the
    // common case.
    let _ = sqlx::query("UPDATE households SET last_briefing_at_day = ? WHERE id = ?")
        .bind(today_bucket)
        .bind(&household_id)
        .execute(&pool)
        .await;

    Ok(Some(insight))
}

// ── Secret management (Claude API key) ────────────────────────────────────────

#[derive(Deserialize)]
pub struct SetApiKeyArgs {
    pub key: String,
}

/// Saves the Claude API key to the OS keychain.
#[tauri::command]
pub async fn set_api_key(args: SetApiKeyArgs) -> Result<(), RecoveryError> {
    let trimmed = args.key.trim();
    if trimmed.is_empty() {
        // Empty input is recoverable by retyping the key.
        return Err(RecoveryError::new(
            "API key cannot be empty.",
            NonEmpty::new(
                RecoveryAction {
                    kind: RecoveryKind::EditField,
                    label: "Re-enter key".to_string(),
                    is_primary: true,
                },
                vec![RecoveryAction {
                    kind: RecoveryKind::Discard,
                    label: "Discard".to_string(),
                    is_primary: false,
                }],
            ),
        ));
    }
    save_claude_api_key(&KeyringStore::new(), trimmed)
        .map_err(|e| RecoveryError::show_help(e.to_string()))
}

/// Returns true if an API key is configured (env var or keychain).
#[tauri::command]
pub async fn has_api_key() -> Result<bool, RecoveryError> {
    has_claude_api_key(&KeyringStore::new())
        .map_err(|e| RecoveryError::show_help(e.to_string()))
}

/// Removes the Claude API key from the keychain. Env var (if set) is untouched.
#[tauri::command]
pub async fn delete_api_key() -> Result<(), RecoveryError> {
    delete_claude_api_key(&KeyringStore::new())
        .map_err(|e| RecoveryError::show_help(e.to_string()))
}

// ── Sidebar read queries (T-048) ──────────────────────────────────────────────

use crate::core::read::{
    account_balances as read_balances, coming_up_transactions as read_coming_up,
    current_envelope_periods as read_envelopes, AccountBalance, ComingUpTxn, EnvelopeStatus,
};

#[tauri::command]
pub async fn get_account_balances(
    state: State<'_, AppState>,
) -> Result<Vec<AccountBalance>, RecoveryError> {
    let pool = state
        .pool
        .lock()
        .expect("pool lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Database not open"))?;
    let household_id = state
        .household_id
        .lock()
        .expect("household_id lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Household not set"))?;

    read_balances(&pool, &household_id)
        .await
        .map_err(|e| RecoveryError::show_help(e.to_string()))
}

#[tauri::command]
pub async fn get_current_envelope_periods(
    state: State<'_, AppState>,
) -> Result<Vec<EnvelopeStatus>, RecoveryError> {
    let pool = state
        .pool
        .lock()
        .expect("pool lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Database not open"))?;
    let household_id = state
        .household_id
        .lock()
        .expect("household_id lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Household not set"))?;

    read_envelopes(&pool, &household_id, now_ms())
        .await
        .map_err(|e| RecoveryError::show_help(e.to_string()))
}

#[tauri::command]
pub async fn get_pending_transactions(
    state: State<'_, AppState>,
) -> Result<Vec<ComingUpTxn>, RecoveryError> {
    let pool = state
        .pool
        .lock()
        .expect("pool lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Database not open"))?;
    let household_id = state
        .household_id
        .lock()
        .expect("household_id lock")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("Household not set"))?;

    read_coming_up(&pool, &household_id, now_ms(), 50)
        .await
        .map_err(|e| RecoveryError::show_help(e.to_string()))
}

// ── GnuCash import commands ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ReadGnuCashArgs {
    pub path: String,
}

#[tauri::command]
pub async fn read_gnucash_file(
    args: ReadGnuCashArgs,
) -> Result<crate::core::import::gnucash::GnuCashPreview, RecoveryError> {
    use std::path::Path;
    crate::core::import::gnucash::reader::preview(Path::new(&args.path))
        .await
        .map_err(|e| RecoveryError::show_help(e.to_string()))
}

#[derive(Deserialize)]
pub struct BuildImportPlanArgs {
    pub path: String,
}

#[tauri::command]
pub async fn gnucash_build_default_plan(
    state: State<'_, AppState>,
    args: BuildImportPlanArgs,
) -> Result<crate::core::import::gnucash::ImportPlan, RecoveryError> {
    use crate::core::import::gnucash::{mapper, reader};
    use std::path::Path;

    let household_id = {
        let g = state.household_id.lock().expect("household_id");
        g.clone()
            .ok_or_else(|| RecoveryError::show_help("No household configured"))?
    };
    let book = reader::read(Path::new(&args.path))
        .await
        .map_err(|e| RecoveryError::show_help(e.to_string()))?;

    let import_id = new_ulid();
    let plan = mapper::build_default_plan(household_id, import_id, &book, new_ulid)
        .map_err(|e| RecoveryError::show_help(e.to_string()))?;

    *state.active_import.lock().expect("active_import") = Some(plan.clone());
    Ok(plan)
}

#[derive(Deserialize)]
pub struct ApplyMappingEditArgs {
    pub edit: crate::core::import::gnucash::mapper::MappingEdit,
}

#[tauri::command]
pub async fn gnucash_apply_mapping_edit(
    state: State<'_, AppState>,
    args: ApplyMappingEditArgs,
) -> Result<crate::core::import::gnucash::ImportPlan, RecoveryError> {
    use crate::core::import::gnucash::mapper;

    let mut guard = state.active_import.lock().expect("active_import");
    let plan = guard
        .as_mut()
        .ok_or_else(|| RecoveryError::show_help("No active import plan"))?;
    mapper::apply_mapping_edit(plan, &args.edit)
        .map_err(|e| RecoveryError::show_help(e.to_string()))?;
    Ok(plan.clone())
}

#[tauri::command]
pub async fn commit_gnucash_import(
    state: State<'_, AppState>,
) -> Result<crate::core::import::gnucash::ImportReceipt, RecoveryError> {
    let pool_opt = state.pool.lock().expect("pool").clone();
    let pool = pool_opt.ok_or_else(|| RecoveryError::show_help("No database open"))?;

    let plan = {
        let g = state.active_import.lock().expect("active_import");
        g.clone()
            .ok_or_else(|| RecoveryError::show_help("No active import plan"))?
    };

    // Import-commit failures roll back atomically; offer Discard alongside ShowHelp
    // so the user can abandon the import without leaving a partial state.
    let receipt = crate::core::import::gnucash::committer::commit(&pool, &plan, now_ms())
        .await
        .map_err(|e| {
            RecoveryError::new(
                format!("Could not import the GnuCash file: {e}"),
                NonEmpty::new(
                    RecoveryAction {
                        kind: RecoveryKind::ShowHelp,
                        label: "Get help".to_string(),
                        is_primary: true,
                    },
                    vec![RecoveryAction {
                        kind: RecoveryKind::Discard,
                        label: "Discard import".to_string(),
                        is_primary: false,
                    }],
                ),
            )
        })?;

    *state.active_import.lock().expect("active_import") = None;
    Ok(receipt)
}

#[derive(Deserialize)]
pub struct RollbackArgs {
    pub import_id: String,
}

#[tauri::command]
pub async fn rollback_gnucash_import(
    state: State<'_, AppState>,
    args: RollbackArgs,
) -> Result<(), RecoveryError> {
    let pool_opt = state.pool.lock().expect("pool").clone();
    let pool = pool_opt.ok_or_else(|| RecoveryError::show_help("No database open"))?;
    crate::core::import::gnucash::committer::rollback(&pool, &args.import_id)
        .await
        .map_err(|e| RecoveryError::show_help(e.to_string()))
}

#[derive(Deserialize)]
pub struct ReconcileArgs {
    pub import_id: String,
    pub path: String,
}

#[tauri::command]
pub async fn reconcile_gnucash_import(
    state: State<'_, AppState>,
    args: ReconcileArgs,
) -> Result<crate::core::import::gnucash::reconcile::BalanceReportArtifact, RecoveryError> {
    use crate::core::import::gnucash::{reader, reconcile, AccountMapping, ImportPlan};
    use std::path::Path;

    let pool_opt = state.pool.lock().expect("pool").clone();
    let pool = pool_opt.ok_or_else(|| RecoveryError::show_help("No database open"))?;
    let household_id = state
        .household_id
        .lock()
        .expect("hh")
        .clone()
        .ok_or_else(|| RecoveryError::show_help("No household configured"))?;

    let book = reader::read(Path::new(&args.path))
        .await
        .map_err(|e| RecoveryError::show_help(e.to_string()))?;

    // Rebuild minimal AccountMapping set from accounts table (rows stamped with this import_id).
    // Query by gnc_guid directly — leaf-name matching is unsound for books with repeated
    // leaf names under different parents (e.g. Assets:Savings vs Investments:Savings).
    #[derive(sqlx::FromRow)]
    struct Row { id: String, name: String, gnc_guid: Option<String> }
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, name, gnc_guid FROM accounts WHERE household_id = ? AND import_id = ?",
    )
    .bind(&household_id)
    .bind(&args.import_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| RecoveryError::show_help(e.to_string()))?;

    let by_guid: std::collections::HashMap<&str, &crate::core::import::gnucash::GncAccount> =
        book.accounts.iter().map(|a| (a.guid.as_str(), a)).collect();

    let account_mappings: Vec<AccountMapping> = rows.iter().filter_map(|r| {
        let guid = r.gnc_guid.as_deref()?;
        let ga = by_guid.get(guid)?;
        Some(AccountMapping {
            gnc_guid: guid.to_string(),
            gnc_full_name: ga.full_name.clone(),
            tally_account_id: r.id.clone(),
            tally_name: r.name.clone(),
            tally_parent_id: None,
            tally_type: crate::core::import::gnucash::AccountType::Asset,
            tally_normal_balance: crate::core::import::gnucash::NormalBalance::Debit,
        })
    }).collect();

    let plan = ImportPlan {
        household_id,
        import_id: args.import_id,
        account_mappings,
        transactions: vec![], // unused by reconcile
    };

    // Reconcile mismatches: offer Discard so the user can roll back the import.
    reconcile::reconcile(&pool, &plan, &book)
        .await
        .map_err(|e| {
            RecoveryError::new(
                format!("Could not reconcile the GnuCash import: {e}"),
                NonEmpty::new(
                    RecoveryAction {
                        kind: RecoveryKind::ShowHelp,
                        label: "Get help".to_string(),
                        is_primary: true,
                    },
                    vec![RecoveryAction {
                        kind: RecoveryKind::Discard,
                        label: "Discard import".to_string(),
                        is_primary: false,
                    }],
                ),
            )
        })
}
