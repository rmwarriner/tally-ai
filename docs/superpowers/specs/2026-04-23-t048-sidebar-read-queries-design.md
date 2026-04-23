# T-048 — Sidebar Read Queries

**Status:** approved (2026-04-23)
**Scope:** Phase 1 beta push — real data behind the three health-sidebar panels.

## Goal

Replace the mock data behind the three health-sidebar panels (Accounts, Envelopes,
Coming up) with live DB-backed queries, so the sidebar reflects the true household
state at all times.

## Scope

In:

- Three Tauri commands backing `useSidebarData.ts`: `get_account_balances`,
  `get_current_envelope_periods`, `get_pending_transactions`.
- A new Rust module `core::read` that owns the SQL.
- Refactor `ai::snapshot` to delegate to `core::read` so there is a single source
  of truth for balance math.
- Fix `create_envelope` to also insert a current-month `envelope_periods` row.
- Frontend query invalidation after `commit_proposal` and after each onboarding
  write step.

Out:

- `/recent` slash command and any "ledger for an account" query — future ticket.
- Rolling or historical envelope periods — Phase 2 (scheduler/recurring).
- Schema migration — not needed; all queries use existing tables.

## Architecture

```
apps/desktop/src-tauri/src/
  core/
    read.rs                ← NEW: account_balances, current_envelope_periods, coming_up_transactions
    envelope.rs            ← currently empty; add current_month_bounds_ms helper here
  ai/
    snapshot.rs            ← refactored to call core::read
  commands/
    mod.rs                 ← three new Tauri wrappers; extends create_envelope
```

Tauri commands are thin wrappers: resolve `pool` and `household_id` from
`AppState`, delegate to `core::read`, map `sqlx::Error` to `String` per
existing pattern in `commands/mod.rs`.

## Data Shapes

Rust (in `core::read`):

```rust
pub struct AccountBalance {
    pub id: String,
    pub name: String,
    pub account_type: String,  // "asset"|"liability"|"income"|"expense"|"equity"
    pub balance_cents: i64,    // signed; positive = normal balance direction
}

pub struct EnvelopeStatus {
    pub envelope_id: String,
    pub name: String,
    pub allocated_cents: i64,
    pub spent_cents: i64,
}

pub struct ComingUpTxn {
    pub id: String,
    pub txn_date: i64,           // unix ms
    pub status: String,          // "pending" | "posted"
    pub payee: Option<String>,   // from transactions.memo
    pub memo: Option<String>,    // same column; UI prefers payee, falls back to memo
    pub amount_cents: i64,       // see Query Semantics below
}
```

Frontend: existing interfaces in `apps/desktop/src/hooks/useSidebarData.ts` match.
Rename `PendingTxn` → `ComingUpTxn` locally and add optional `status` field.
No UI code changes required — `ComingUpPanel` already does
`payee ?? memo ?? "Untitled"`.

## Query Semantics

### Account balances

Same SQL as `ai::snapshot::query_balances` today. Left-join journal lines and
transactions, sum `amount` conditionally on `side` and `status='posted'` so
pending proposals never move a balance. Exclude `is_placeholder=1`. Compute
signed balance using `normal_balance`.

### Current envelope periods

```sql
SELECT e.id AS envelope_id,
       e.name,
       COALESCE(ep.allocated, 0) AS allocated,
       COALESCE(ep.spent, 0)     AS spent
FROM envelopes e
LEFT JOIN envelope_periods ep
  ON ep.envelope_id = e.id
 AND ep.period_start <= :as_of
 AND ep.period_end   >= :as_of
WHERE e.household_id = ?
ORDER BY e.name
```

LEFT JOIN so envelopes without a current period (e.g. during monthly rollover,
or migration-created envelopes) appear with zeros instead of being silently
dropped.

### Coming-up transactions

Union of `status='pending'` (AI proposals awaiting confirmation) and
`status='posted' AND txn_date > :as_of` (future-dated posted, e.g. a post-dated
manual entry). `amount_cents` is derived:

1. If the transaction has debit lines to an `expense` account, return their sum.
2. Else fall back to the sum of debit lines to an `asset` account.
3. Else 0.

```sql
SELECT t.id,
       t.txn_date,
       t.status,
       t.memo,
       COALESCE(
         (SELECT SUM(jl.amount) FROM journal_lines jl
          JOIN accounts a ON a.id = jl.account_id
          WHERE jl.transaction_id = t.id AND jl.side='debit' AND a.type='expense'),
         (SELECT SUM(jl.amount) FROM journal_lines jl
          JOIN accounts a ON a.id = jl.account_id
          WHERE jl.transaction_id = t.id AND jl.side='debit' AND a.type='asset'),
         0
       ) AS amount_cents
FROM transactions t
WHERE t.household_id = ?
  AND (t.status = 'pending' OR (t.status = 'posted' AND t.txn_date > :as_of))
ORDER BY t.txn_date ASC
LIMIT ?
```

Limit default 50; `ComingUpPanel` slices to 5 client-side.

## `create_envelope` Fix

Today, `create_envelope` inserts an `envelopes` row but no `envelope_periods`
row. Until the user takes an action that creates a period, the sidebar shows
nothing. Fix: after inserting `envelopes`, insert an `envelope_periods` row
for the current local-calendar month with `allocated=0, spent=0`.

Month bounds are computed in the household's IANA timezone (from
`households.timezone`), then converted to UTC-midnight unix ms. `create_envelope`
already requires an active `AppState.household_id`, so the household row is
guaranteed to exist — read its `timezone` column inside the command. Helper:

```rust
// in core::envelope
pub fn current_month_bounds_ms(tz: &str, now_ms: i64) -> (i64, i64)
```

Uses `chrono` + `chrono-tz` (add to `Cargo.toml` if not present). Tests cover
DST transitions, Jan 1, Dec 31, and a non-US zone.

## Frontend Wiring

`apps/desktop/src/hooks/useSidebarData.ts`:

- No breaking shape changes.
- Rename local `PendingTxn` → `ComingUpTxn`; add optional `status?: "pending"|"posted"`.

New hook `apps/desktop/src/hooks/useInvalidateSidebar.ts`:

```ts
export function useInvalidateSidebar() {
  const queryClient = useQueryClient();
  return useCallback(
    () => queryClient.invalidateQueries({ queryKey: ["sidebar"] }),
    [queryClient],
  );
}
```

Call sites:

- `useSendMessage` — after `commit_proposal` resolves with `status: "committed"`.
- `useOnboardingEngine` — after each step that writes: `create_household`,
  `create_account`, `set_opening_balance`, `create_envelope`, `import_hledger`.
  One call at the end of each step is fine — the shared `["sidebar"]` root key
  refetches all three panels together.

## Error Handling

Per CLAUDE.md, commands return `Result<T, String>`. A `sqlx::Error` becomes
`e.to_string()` (logged only; user-facing copy is "Could not load" via the
panel's `error` boolean). No `RecoveryAction` needed — these are read-only,
and TanStack Query's refetch-on-focus already handles transient failures.

## Testing

### Rust (TDD, 80% coverage)

`core::read` tests:

- `account_balances`: pending transactions excluded, placeholders excluded,
  signed balance correct for credit-normal accounts, household isolation.
- `current_envelope_periods`: envelope with no period returns zeros, envelope
  with overlapping period returns correct values, household isolation.
- `coming_up_transactions`: pending included, future-posted included,
  past-posted excluded, limit respected, amount derived from expense then
  asset then 0.

`core::envelope::current_month_bounds_ms` tests: DST spring-forward,
DST fall-back, Jan 1 midnight, Dec 31 23:59, `America/Chicago`, `Asia/Tokyo`.

`commands::create_envelope` test: asserts an `envelope_periods` row exists
with the computed bounds after `create_envelope` runs.

`ai::snapshot` tests: unchanged — must continue to pass after the refactor
to `core::read`.

### TypeScript

- Existing `useSidebarData.test.tsx` and panel tests pass unchanged.
- New: `useSendMessage` test — after mocked `commit_proposal` resolves
  `committed`, `queryClient.invalidateQueries` is called with
  `{ queryKey: ['sidebar'] }`.
- New: `useOnboardingEngine` tests — each write step triggers invalidation.

## Non-Goals

- No new UI. All three panels render unchanged against real data.
- No new slash-command wiring. `/recent` continues to route through the AI
  as today.
- No read command for historical balances, per-account ledger, or multi-period
  envelope history — all deferred.

## Open Questions

None.
