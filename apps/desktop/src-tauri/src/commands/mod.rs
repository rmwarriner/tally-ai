// Tauri command handlers — thin wrappers delegating to core/ai layers
//
// State model: AppState holds the live SqlitePool after the household is created
// or unlocked. Commands that require a DB connection read from that state.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager, State};

use crate::core::coa::seed_chart_of_accounts;
use crate::db::create_encrypted_db;
use crate::id::new_ulid;

// ── App state ────────────────────────────────────────────────────────────────

pub struct AppState {
    pub pool: Mutex<Option<SqlitePool>>,
    pub household_id: Mutex<Option<String>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            pool: Mutex::new(None),
            household_id: Mutex::new(None),
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
pub async fn check_setup_status(app: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    let path = db_path(&app)?;
    if !path.exists() {
        return Ok(false);
    }
    let sp = salt_path(&app)?;
    if !sp.exists() {
        return Ok(false);
    }
    // Clone pool out of lock so the guard is dropped before any await
    let pool_opt = state.pool.lock().expect("pool lock").clone();
    if let Some(pool) = pool_opt {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM households")
            .fetch_one(&pool)
            .await
            .map_err(|e| e.to_string())?;
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
) -> Result<String, String> {
    let path = db_path(&app)?;
    let sp = salt_path(&app)?;

    // Generate a fresh random 16-byte salt
    let salt: [u8; 16] = rand_salt();
    std::fs::write(&sp, salt).map_err(|e| format!("Cannot write salt: {e}"))?;

    let pool = create_encrypted_db(&path, &args.passphrase, &salt)
        .await
        .map_err(|e| e.to_string())?;

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
    .map_err(|e| e.to_string())?;

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
    .map_err(|e| e.to_string())?;

    // Seed chart of accounts
    seed_chart_of_accounts(&pool, &household_id)
        .await
        .map_err(|e| e.to_string())?;

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
) -> Result<String, String> {
    let pool = state.pool.lock().expect("pool lock").clone().ok_or("Database not open")?;
    let household_id = state.household_id.lock().expect("household_id lock").clone().ok_or("Household not set")?;

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
    .map_err(|e| e.to_string())?;

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
    .map_err(|e| e.to_string())?;

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
) -> Result<(), String> {
    let pool = state.pool.lock().expect("pool lock").clone().ok_or("Database not open")?;
    let household_id = state.household_id.lock().expect("household_id lock").clone().ok_or("Household not set")?;

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
    .map_err(|e| e.to_string())?;

    let obe_id = obe.map(|(id,)| id).ok_or("Opening Balance Equity account not found")?;

    // Get account type to determine debit/credit side
    let account_type: (String,) =
        sqlx::query_as("SELECT type FROM accounts WHERE id = ?")
            .bind(&args.account_id)
            .fetch_one(&pool)
            .await
            .map_err(|e| e.to_string())?;

    let (primary_side, equity_side) = match account_type.0.as_str() {
        "asset" => ("debit", "credit"),
        "liability" => ("credit", "debit"),
        _ => ("debit", "credit"),
    };

    let txn_id = new_ulid();
    let line1_id = new_ulid();
    let line2_id = new_ulid();
    let ts = now_ms();

    sqlx::query(
        "INSERT INTO transactions (id, household_id, txn_date, entry_date, status, source, memo, created_at)
         VALUES (?, ?, ?, ?, 'posted', 'opening_balance', 'Opening balance', ?)",
    )
    .bind(&txn_id)
    .bind(&household_id)
    .bind(ts)
    .bind(ts)
    .bind(ts)
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

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
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

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
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[derive(Deserialize)]
pub struct CreateEnvelopeArgs {
    pub name: String,
}

/// Creates a new envelope (budget category). Links to the first expense account.
/// Returns the new envelope ULID.
#[tauri::command]
pub async fn create_envelope(
    state: State<'_, AppState>,
    args: CreateEnvelopeArgs,
) -> Result<String, String> {
    let pool = state.pool.lock().expect("pool lock").clone().ok_or("Database not open")?;
    let household_id = state.household_id.lock().expect("household_id lock").clone().ok_or("Household not set")?;

    // Find or create an expense account matching the envelope name
    let expense_account: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM accounts WHERE household_id = ? AND type = 'expense' AND is_placeholder = 0 LIMIT 1",
    )
    .bind(&household_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let account_id = if let Some((id,)) = expense_account {
        id
    } else {
        // Create a generic expense account for this envelope
        let id = new_ulid();
        let ts = now_ms();
        let parent: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM accounts WHERE household_id = ? AND type = 'expense' AND is_placeholder = 1 AND name = 'Expenses' LIMIT 1",
        )
        .bind(&household_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| e.to_string())?;

        sqlx::query(
            "INSERT INTO accounts (id, household_id, parent_id, name, type, normal_balance, is_placeholder, currency, created_at)
             VALUES (?, ?, ?, ?, 'expense', 'debit', 0, 'USD', ?)",
        )
        .bind(&id)
        .bind(&household_id)
        .bind(parent.map(|(pid,)| pid))
        .bind(&args.name)
        .bind(ts)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

        id
    };

    let envelope_id = new_ulid();
    let ts = now_ms();

    sqlx::query(
        "INSERT INTO envelopes (id, household_id, account_id, name, created_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&envelope_id)
    .bind(&household_id)
    .bind(&account_id)
    .bind(&args.name)
    .bind(ts)
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(envelope_id)
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
pub async fn import_hledger(args: ImportHledgerArgs) -> Result<String, String> {
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
pub async fn get_ai_defaults(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
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

/// Reverses the most recently posted AI transaction. Stub for Phase 1.
#[tauri::command]
pub async fn undo_last_transaction(state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.pool.lock().expect("pool lock");
    if guard.is_none() {
        return Err("No database open".to_string());
    }
    // TODO(phase2): implement full GAAP reversal via core::correction
    Ok(())
}

// ── Deterministic salt generation (simple, uses OS random) ────────────────────

fn rand_salt() -> [u8; 16] {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    // Use a combination of time + process ID for entropy.
    // Not cryptographically strong but sufficient for Phase 1 before
    // we wire a proper CSPRNG. TODO(phase2): use getrandom crate.
    let mut hasher = DefaultHasher::new();
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos()
        .hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    let h1 = hasher.finish();
    std::thread::current().id().hash(&mut hasher);
    let h2 = hasher.finish();

    let mut salt = [0u8; 16];
    salt[..8].copy_from_slice(&h1.to_le_bytes());
    salt[8..].copy_from_slice(&h2.to_le_bytes());
    salt
}
