# GnuCash Import — Design Spec

**Date:** 2026-04-24
**Tickets:** T-071, T-072, T-073, T-074
**Status:** Approved (brainstorming phase)

## Goal

Let the user migrate from an existing GnuCash SQLite book into Tally during onboarding, so they can beta-test Tally alongside their current GnuCash ledger. Import must be faithful enough to GnuCash that account balances match 1:1 after commit.

## Scope

**In scope:** accounts, commodities (USD only), posted transactions, and splits from a GnuCash SQLite file.

**Out of scope for this batch:**
- Non-USD currencies and non-currency commodities (stocks, mutual funds). Hard-fail at read time.
- GnuCash budgets. Envelopes will be created by the user after import via the normal chat flow.
- GnuCash scheduled transactions. Phase 2 only (see `scheduler/` stub).
- Investment prices and lots. Phase 2.
- Re-entrant / standalone import. The import path lives inside onboarding only. No `/import-gnucash` slash command.

## Architectural decisions (with rationale)

### Entry point: onboarding only

The GnuCash branch is an extension of the existing T-042 hledger migration stub inside `buildOnboardingHandler`. The user answers "I'm migrating from GnuCash" during setup, picks a file, and the CoA mapping session runs inline before handoff. Rationale: this is a one-shot migration for beta; a standalone slash command would need to handle re-entrance against already-populated households, which is unnecessary complexity.

### Hard-fail on non-USD

If the reader finds any account whose commodity is not USD, or any non-currency commodity (STOCK, MUTUAL), it aborts before any writes. Rationale: Phase 1 is USD-only. Silently skipping non-USD accounts would drop splits and make T-074 reconciliation meaningless. Partial imports are worse than no import.

### Preserve GnuCash opening-balance pattern verbatim

GnuCash uses an `Equity:Opening Balances` account as the other side of initial-balance transactions. The importer creates a matching Tally equity account and imports those transactions with `source='import'` (not `source='opening_balance'`). Rationale: faithfulness beats cleanliness for a migration tool. Pattern-detection to collapse opening balances into Tally's native `opening_balance` source would be fragile and could misclassify custom splits.

### CoA mapping: confirm-all-or-edit, single turn

The mapper applies default GnuCash-type-to-Tally-type mapping (see table below), renders every account on one artifact card, and waits for the user to either confirm wholesale or request targeted edits. Rationale: GnuCash's account-type field maps cleanly; real books have 30-80 accounts and per-account prompting would be exhausting; zero-prompt auto-map hides mistakes until they're posted against.

### Skip envelopes at import time

Expense transactions import with `envelope_id = NULL` on their journal lines (column is nullable). User creates envelopes afterward via normal chat. Rationale: importing history into zero-allocation envelopes would make the sidebar show everything "over budget" and drown out the current month.

### Reconciliation: account-level balance artifact, blocks handoff

After commit, the user sees a side-by-side `BalanceReport` artifact of Tally balances vs GnuCash balances with mismatches flagged. Handoff to normal chat is blocked until they click "Looks right, continue" or "Something's off, roll back". Rationale: migration is one-time; the confirmation step is cheap insurance. Account-level balances are the number the user will verify against GnuCash anyway.

### Pipeline shape: three-phase, all-or-nothing

Read → Map → Commit, with a plain struct at each boundary. Commit runs in a single DB transaction. Rationale: GnuCash files are small enough to fit in memory (typically single-digit MB); atomicity matters for T-074; clean phase boundaries give each ticket an independent testable surface.

## Module layout

```
apps/desktop/src-tauri/src/core/import/
├── mod.rs                   # re-exports
└── gnucash/
    ├── mod.rs               # public types: GnuCashBook, ImportPlan, ImportError
    ├── reader.rs            # T-071: opens foreign SQLite, builds GnuCashBook
    ├── mapper.rs            # T-072: pure logic — book + user edits → ImportPlan
    ├── committer.rs         # T-073: executes ImportPlan against Tally pool
    └── reconcile.rs         # T-074: builds post-commit balance artifact
```

## Tauri commands (new)

| Command | Purpose |
|---|---|
| `read_gnucash_file(path) -> GnuCashPreview` | Reader phase. Returns account tree, txn count, and `non_usd_accounts` list. |
| `gnucash_build_default_plan(path) -> ImportPlan` | Mapper phase, initial plan with default type mapping applied. |
| `gnucash_apply_mapping_edit(plan, edit) -> ImportPlan` | Mapper phase, apply one user edit to a plan (pure; no DB). |
| `commit_gnucash_import(plan) -> ImportReceipt` | Committer phase. Single DB transaction. Idempotent on `source_ref`. |
| `reconcile_gnucash_import(import_id, path) -> BalanceReportArtifact` | Reconcile phase. Re-reads the file; compares. |
| `rollback_gnucash_import(import_id) -> ()` | Deletes all rows stamped with `import_id` in a single DB transaction. |

The existing stubbed `import_hledger` command stays as-is; this spec doesn't touch it.

## Database changes

**Migration `0006_gnucash_import_columns.sql`:**

```sql
ALTER TABLE transactions ADD COLUMN source_ref TEXT;
CREATE INDEX idx_transactions_source_ref ON transactions(source_ref) WHERE source_ref IS NOT NULL;
CREATE UNIQUE INDEX idx_transactions_source_ref_unique
    ON transactions(household_id, source_ref) WHERE source_ref IS NOT NULL;

ALTER TABLE accounts ADD COLUMN import_id TEXT;
CREATE INDEX idx_accounts_import_id ON accounts(import_id) WHERE import_id IS NOT NULL;
```

`source_ref` on `transactions` stores the GnuCash transaction GUID. The unique index gives idempotency: re-running an import skips rows whose GUID already exists for that household. `import_id` on `accounts` enables scoped rollback of import-created accounts.

No changes to `journal_lines` — GUIDs on splits aren't needed for our use case because journal lines are always deleted as a unit with their parent transaction.

## Data shapes (Rust)

### `GnuCashBook` (reader output)

```rust
pub struct GnuCashBook {
    pub book_guid: String,
    pub commodities: Vec<GncCommodity>,
    pub accounts: Vec<GncAccount>,
    pub transactions: Vec<GncTransaction>,
}

pub struct GncCommodity {
    pub guid: String,
    pub namespace: String,       // "CURRENCY" | "NASDAQ" | etc.
    pub mnemonic: String,        // "USD" | "AAPL" | etc.
}

pub struct GncAccount {
    pub guid: String,
    pub parent_guid: Option<String>,
    pub name: String,
    pub full_name: String,       // "Assets:Current:Checking"
    pub gnc_type: GncAccountType,
    pub commodity_guid: String,
    pub placeholder: bool,
    pub hidden: bool,
}

pub enum GncAccountType {
    Bank, Cash, Asset, Stock, Mutual, Receivable,
    Credit, Liability, Payable,
    Income, Expense, Equity,
    Root, Trading,  // skipped
}

pub struct GncTransaction {
    pub guid: String,
    pub post_date: i64,          // unix ms, UTC midnight of local date
    pub enter_date: i64,
    pub description: String,
    pub splits: Vec<GncSplit>,   // ≥ 2; amounts sum to zero in cents
}

pub struct GncSplit {
    pub guid: String,
    pub account_guid: String,
    pub amount_cents: i64,       // signed (GnuCash convention: positive toward debit side)
    pub memo: String,
    pub reconcile_state: char,   // 'n' | 'c' | 'y'
}
```

### `ImportPlan` (mapper output, committer input)

```rust
pub struct ImportPlan {
    pub household_id: String,
    pub import_id: String,                   // ULID; stamped on accounts and transactions
    pub account_mappings: Vec<AccountMapping>,
    pub transactions: Vec<PlannedTransaction>,
}

pub struct AccountMapping {
    pub gnc_guid: String,
    pub gnc_full_name: String,
    pub tally_account_id: String,            // ULID, pre-generated
    pub tally_name: String,                  // leaf
    pub tally_parent_id: Option<String>,
    pub tally_type: AccountType,
    pub tally_normal_balance: NormalBalance,
}

pub struct PlannedTransaction {
    pub gnc_guid: String,                    // → transactions.source_ref
    pub txn_date: i64,
    pub memo: Option<String>,
    pub lines: Vec<PlannedLine>,
}

pub struct PlannedLine {
    pub tally_account_id: String,            // references AccountMapping.tally_account_id
    pub amount_cents: i64,                   // always positive
    pub side: Side,                          // debit | credit; positive GnuCash split value → debit, negative → credit
}
```

### `GnuCashPreview` (command return)

```rust
pub struct GnuCashPreview {
    pub book_guid: String,
    pub account_count: u32,
    pub transaction_count: u32,
    pub non_usd_accounts: Vec<String>,       // full_names; non-empty → hard error
}
```

### `ImportReceipt` (command return)

```rust
pub struct ImportReceipt {
    pub import_id: String,
    pub accounts_created: u32,
    pub transactions_committed: u32,
    pub transactions_skipped: u32,           // idempotency; source_ref already existed
}
```

## Default type mapping

| GnuCash type | Tally `type` | Tally `normal_balance` |
|---|---|---|
| `BANK`, `CASH`, `ASSET`, `STOCK`, `MUTUAL`, `RECEIVABLE` | `asset` | `debit` |
| `CREDIT`, `LIABILITY`, `PAYABLE` | `liability` | `credit` |
| `INCOME` | `income` | `credit` |
| `EXPENSE` | `expense` | `debit` |
| `EQUITY` | `equity` | `credit` |
| `ROOT`, `TRADING` | *skipped* | — |

`STOCK` and `MUTUAL` are listed for completeness; in practice the currency scan in the reader rejects any book that contains them (non-currency commodities).

## Data flow

```
┌────────────────────┐
│ onboarding handler │ detects "migrating from GnuCash"
└─────────┬──────────┘
          ▼
    file dialog (TS)
          │
          ▼
read_gnucash_file(path) ──► GnuCashPreview
          │                       │
          │                       └─ non_usd_accounts non-empty ──► HardError, STOP
          ▼
gnucash_build_default_plan(path) ──► ImportPlan
          │
          ▼
 artifact card: "Looks right?" ◄──► (loop) gnucash_apply_mapping_edit(plan, edit)
          │
          ▼ user confirms
commit_gnucash_import(plan) ──► ImportReceipt
          │                       │
          │                       └─ commit error ──► HardError, rollback, STOP
          ▼
reconcile_gnucash_import(import_id, path) ──► BalanceReportArtifact
          │
          ▼
  artifact card + "Looks right, continue" / "Roll back"
          │                                       │
          ▼ continue                              ▼ roll back
     handoff message (T-044)         rollback_gnucash_import(import_id)
                                                  │
                                                  ▼
                                          back to file dialog
```

## Error handling

Every failure carries `NonEmpty<RecoveryAction>` per CLAUDE.md. User-facing text is plain language; internal codes go to logs only.

### Reader phase — HardError, import never starts

| Condition | Message | RecoveryActions |
|---|---|---|
| File unreadable | "Couldn't open that GnuCash file." | `EditField("path")`, `ShowHelp("gnucash-path")` |
| Not a GnuCash SQLite book | "That doesn't look like a GnuCash file." | `EditField("path")`, `Discard` |
| Non-USD commodity present | "This GnuCash book has accounts in other currencies. Tally currently supports USD only." + account list | `ShowHelp("gnucash-currency")`, `Discard` |
| Transaction splits don't sum to zero | "GnuCash book is inconsistent: one or more transactions don't balance." + first 5 GUIDs | `ShowHelp("gnucash-corruption")`, `Discard` |

### Mapper phase — SoftWarning, import can still proceed

| Condition | Message | RecoveryActions |
|---|---|---|
| Empty placeholder account | "Skipping empty placeholder accounts: …" | `PostAnyway`, `ShowHelp` |
| Hidden account has transactions | "Importing transactions from hidden account '…' anyway." | `PostAnyway`, `Discard("account:<guid>")` |
| Two mappings resolve to same Tally full-name | "Duplicate account name after mapping: …" | `EditField("mapping:<guid>")`, `ShowHelp` |

### Committer phase — HardError, DB rolls back

| Condition | Message | RecoveryActions |
|---|---|---|
| Integrity constraint violated | "Import aborted: database rejected a row." | `ShowHelp("gnucash-commit-failed")`, `Discard` |
| `core::validation` rejects a planned transaction | "Transaction on <date> '<memo>' didn't validate." | `ShowHelp`, `Discard` |
| `import_id` already exists in `accounts` | "An import with this ID was already run. Roll it back first or start a new one." | `Discard` (with rollback option), `ShowHelp` |

### Reconcile phase — AIAdvisory, non-blocking

| Condition | Message | RecoveryActions |
|---|---|---|
| One or more account balances don't match | "Some balances don't match GnuCash. Review the report below." | `PostAnyway` ("keep"), `Discard` ("roll back") |

### Rollback phase — HardError if it fails

Rollback runs in a single DB transaction, deleting in order: `journal_lines` → `transactions` → `accounts`, all filtered by `import_id`. Failure emits `HardError` with `ShowHelp("gnucash-rollback-failed")` and the `import_id` so the user has a support hook.

`audit_log` entries remain (INSERT-only per architectural rule); the rollback itself writes an audit entry recording the delete.

## Testing

### Rust unit tests

**`reader.rs`:**
- Happy-path fixture: accounts, transactions, splits parse correctly.
- Currency scan: EUR fixture → `non_usd_accounts` populated.
- Corrupt fixture: splits sum to 100¢ → "doesn't balance" HardError.
- Non-GnuCash SQLite file → "doesn't look like a GnuCash file".
- Hidden/placeholder flags round-trip.

**`mapper.rs`** (pure):
- Default type mapping — one assertion per `GncAccountType` variant.
- Hierarchy: Tally `parent_id` chain matches GnuCash's.
- User override: `AccountMappingEdit` changes only the targeted account.
- Duplicate-name detection → returns SoftWarning.

**`committer.rs`** (real encrypted test DB):
- Happy path: small plan commits; rows carry `source='import'`, `import_id`, `source_ref`.
- Idempotency: same plan twice → second run returns `transactions_skipped == n`.
- Atomicity: plan with one bad transaction → nothing commits.
- Opening balances: equity-side transactions commit with `source='import'`, not `source='opening_balance'`.
- Rollback: import → rollback → all rows with `import_id` gone, no orphaned `journal_lines`, `audit_log` entries preserved.

**`reconcile.rs`:**
- Happy fixture: per-account Tally balance matches GnuCash expected.
- Mismatch detection: corrupt a journal line after commit; reconcile flags the account.

### TypeScript tests

- Onboarding handler GnuCash branch emits expected setup cards in order.
- CoA mapping card renders every account with inferred type.
- Mapping-edit loop: handler applies one edit and re-renders without re-reading the file.

### Integration test

- Fixture GnuCash file: ~10 accounts, ~30 transactions including an opening-balance pattern.
- Full pipeline: read → default plan → commit → reconcile.
- Assertions: receipt counts, zero mismatches, one `audit_log` entry per transaction.

### Fixtures

Committed `.gnucash` SQLite files under `apps/desktop/src-tauri/tests/fixtures/gnucash/`. Generator script (`scripts/build_gnucash_fixtures.sh`) is one-time and not required at test time. Minimum fixture set:
- `happy.gnucash` — ~10 accounts, opening-balance pattern, ~30 transactions, all USD.
- `eur.gnucash` — one EUR account to exercise currency rejection.
- `corrupt.gnucash` — one transaction whose splits sum to non-zero.

### Coverage

80% overall (pre-commit gate). Reader and committer ≥90% — they're the risk surface.

## Ticket breakdown

| Ticket | Surface | Outcome |
|---|---|---|
| T-071 | `reader.rs`, `read_gnucash_file` command, migration `0006`, reader fixtures | Given a GnuCash file, return a validated `GnuCashBook` or HardError. |
| T-072 | `mapper.rs`, `gnucash_build_default_plan` / `gnucash_apply_mapping_edit` commands, onboarding handler GnuCash branch (TS), CoA mapping artifact card (TS) | Given a `GnuCashBook` and user confirmations/edits, yield a frozen `ImportPlan`. |
| T-073 | `committer.rs`, `commit_gnucash_import` / `rollback_gnucash_import` commands | Given an `ImportPlan`, atomically commit or roll back. Idempotent on `source_ref`. |
| T-074 | `reconcile.rs`, `reconcile_gnucash_import` command, reconciliation artifact card (TS), handoff gate (TS) | Given an `import_id`, produce a side-by-side balance report and block handoff until user resolves. |

All four tickets ship together as a single PR per the "batch PRs" memory — Rust CI is 4+ min per run.
