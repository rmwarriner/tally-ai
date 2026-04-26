# P2 Section 9.7 — Testing & Polish — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land Phase 1 testing & polish: behavior matrices for Rust validators and React components, centralized error → RecoveryAction wrapper, WCAG 2.1 AA leaf-fix pass, and Playwright + Rust integration E2E for the four critical flows.

**Architecture:** Three sequential PRs. PR 1 = pure-Rust validation matrix + T-065 doc rule. PR 2 = `safeInvoke` wrapper (with every `#[tauri::command]` migrated to `Result<T, RecoveryError>`) + React component matrix + a11y leaf fixes wired through axe-core. PR 3 = Playwright E2E with mock-`invoke` injection + Rust orchestrator integration tests.

**Tech Stack:** Rust 2021 (sqlx, thiserror), TypeScript strict, React 19 functional, Vitest 4, Playwright 1.x, axe-core, Tauri 2.

**Spec:** [`docs/superpowers/specs/2026-04-26-p2-testing-and-polish-design.md`](../specs/2026-04-26-p2-testing-and-polish-design.md)

---

## File Structure

### PR 1 (T-060 + T-065)
- **Create:** `apps/desktop/src-tauri/src/core/validation_matrix.rs` — canonical rule × {pass, fail, edge} test inventory.
- **Modify:** `apps/desktop/src-tauri/src/core/mod.rs` — register the new test module.
- **Modify:** `CLAUDE.md` (Code conventions section) — add the keep-current rule.
- **Create:** `CONTRIBUTING.md` — convention paragraph (only if missing).

### PR 2 (T-064 + T-061 + T-063)
- **Modify:** `apps/desktop/src-tauri/src/error.rs` — add `RecoveryError` type.
- **Modify:** `apps/desktop/src-tauri/src/commands/mod.rs` — every `#[tauri::command]` returns `Result<T, RecoveryError>`.
- **Modify:** `packages/core-types/src/index.ts` — add `RecoveryError` mirror.
- **Create:** `apps/desktop/src/lib/safeInvoke.ts` — typed wrapper.
- **Create:** `apps/desktop/src/lib/safeInvoke.test.ts`.
- **Create:** `apps/desktop/src/components/ErrorBoundary.tsx`.
- **Create:** `apps/desktop/src/components/ErrorBoundary.test.tsx`.
- **Modify:** `apps/desktop/src/main.tsx` — wrap `<App>` in `<ErrorBoundary>`.
- **Modify (sweep):** every existing call site under `apps/desktop/src/hooks/` and `apps/desktop/src/components/` that imports `invoke` directly.
- **Create:** `apps/desktop/src/__tests__/MATRIX.md` — component behavior inventory.
- **Create/Modify:** test files alongside each component the matrix targets.
- **Create:** `apps/desktop/src/test/axe.ts` — axe-core helper + rule config.
- **Create:** `docs/superpowers/a11y-2026-04.md` — audit findings + status per item.
- **Modify:** components flagged by audit (aria-label, focus-visible, contrast tokens, tab order, reduced-motion).
- **Modify:** `apps/desktop/.eslintrc.cjs` (or `eslint.config.js`) — restrict direct `invoke` imports.

### PR 3 (T-062)
- **Create:** `apps/desktop/playwright.config.ts`.
- **Create:** `apps/desktop/e2e/setup.ts` — mock `invoke` injector.
- **Create:** `apps/desktop/e2e/fixtures/responses.ts` — canned command responses.
- **Create:** `apps/desktop/e2e/onboarding.spec.ts`, `entry.spec.ts`, `fix.spec.ts`, `undo.spec.ts`.
- **Create:** `apps/desktop/e2e/contract.spec.ts` — mock-vs-Rust command contract.
- **Modify:** `apps/desktop/src/main.tsx` — `import.meta.env.MODE === 'test'` branch installs the mock.
- **Modify:** `apps/desktop/package.json` — `test:e2e` script + Playwright dep.
- **Create:** `apps/desktop/src-tauri/tests/orchestrator_integration.rs`.
- **Create:** `apps/desktop/src-tauri/tests/common/mod.rs` — `MockClaudeAdapter`.
- **Modify:** `.github/workflows/ci.yml` — add Playwright headless job + Rust integration step.

---

# PR 1 — `feat(core): T-060 Rust validation behavior matrix + T-065 doc discipline`

**Branch:** `feat/t-060-validation-matrix` (off `main`).

**Pre-flight:**

```bash
git checkout main && git pull && git checkout -b feat/t-060-validation-matrix
```

---

### Task 1: Add the keep-current discipline rule (T-065)

**Files:**
- Modify: `CLAUDE.md`
- Create: `CONTRIBUTING.md` (only if missing)

- [ ] **Step 1: Confirm CONTRIBUTING.md state.**

```bash
ls -la /Users/robert/Projects/tally.ai/CONTRIBUTING.md
```

If it exists, skip Step 2. Read its contents and append the convention paragraph.

- [ ] **Step 2: Create CONTRIBUTING.md (only if missing).**

Write `/Users/robert/Projects/tally.ai/CONTRIBUTING.md`:

```markdown
# Contributing to Tally.ai

## Documentation discipline

When a `feat:` PR lands ticket work, update the **Implementation status**
section of `CLAUDE.md` in the same PR. The section is the source of truth
for "what's currently shipped" and review depends on it being current.

- New components or hooks: add a one-line entry under the relevant subsection.
- New Tauri commands or migrations: add a one-line entry.
- Behavior changes that update an existing entry: edit the existing line in place.

If the change is documentation-only (no code), skip the status update.

## Architectural decisions

Any architectural choice not covered by the Phase 1 spec is logged in
`DECISIONS.md` *before* implementation, in the same PR. See the file for format.
```

- [ ] **Step 3: Add the convention rule to CLAUDE.md.**

In `CLAUDE.md`, find the "Code conventions" section (around line 30). Add this line after the existing "Commit messages" bullet:

```markdown
- Update the "Implementation status" section in this file as part of any
  feat: PR that lands ticket work. See CONTRIBUTING.md for detail.
```

- [ ] **Step 4: Commit.**

```bash
git add CLAUDE.md CONTRIBUTING.md
git commit -m "docs(t-065): keep-current discipline for CLAUDE.md"
```

The pre-commit hook runs full Rust + TS test suite + 80% coverage gate. Wait for it.

---

### Task 2: Scaffold the validation matrix file

**Files:**
- Create: `apps/desktop/src-tauri/src/core/validation_matrix.rs`
- Modify: `apps/desktop/src-tauri/src/core/mod.rs`

The matrix is a single Rust test module that exists *alongside* `validation.rs`. It does not replace existing tests — it adds a canonical inventory. Each rule asserts: (a) the result variant, (b) the error/warning code, (c) the recovery action set.

- [ ] **Step 1: Read `core/mod.rs`.**

Use the `Read` tool on `/Users/robert/Projects/tally.ai/apps/desktop/src-tauri/src/core/mod.rs`.

- [ ] **Step 2: Add the matrix module to `core/mod.rs`.**

After the existing module declarations, add:

```rust
#[cfg(test)]
mod validation_matrix;
```

- [ ] **Step 3: Create the matrix file with module skeleton + shared helpers.**

Write `/Users/robert/Projects/tally.ai/apps/desktop/src-tauri/src/core/validation_matrix.rs`:

```rust
//! T-060 — canonical inventory of validation behaviors.
//!
//! Every Tier 1 (HardError), Tier 2 (SoftWarning), and Tier 3 (AIAdvisory)
//! variant has at least one positive-trigger test, one non-trigger test, and
//! (where meaningful) one boundary/edge test. Each test asserts the expected
//! recovery action set against the spec, not just the error variant.

#![cfg(test)]

use sqlx::SqlitePool;

use crate::ai::advisories;
use crate::core::proposal::{ProposedLine, Side, TransactionProposal};
use crate::core::validation::{
    validate, AIAdvisory, HardError, HardErrorCode, SoftWarning, SoftWarningCode,
    ValidationResult,
};
use crate::error::{RecoveryAction, RecoveryKind};

// Shared fixture helpers ---------------------------------------------------

async fn fresh_pool() -> SqlitePool {
    // Use whatever in-memory pool helper validation.rs::tests uses.
    // See Task 3 for adapting this to the actual helper name.
    todo!("see Task 3")
}

fn baseline_proposal_for(seed: &SeedIds) -> TransactionProposal {
    todo!("see Task 3 — fill in once seed accounts exist")
}

struct SeedIds {
    household_id: String,
    cash_account_id: String,
    expense_account_id: String,
    grocery_envelope_id: Option<String>,
}

async fn seed_household(_pool: &SqlitePool) -> SeedIds {
    todo!("see Task 3")
}

fn hard_codes(result: &ValidationResult) -> Vec<HardErrorCode> {
    match result {
        ValidationResult::Rejected { errors, .. } => {
            errors.iter().map(|e| e.code).collect()
        }
        _ => vec![],
    }
}

fn soft_codes(result: &ValidationResult) -> Vec<SoftWarningCode> {
    match result {
        ValidationResult::Warnings { warnings, .. } => {
            warnings.iter().map(|w| w.code).collect()
        }
        ValidationResult::Rejected { warnings, .. } => {
            warnings.iter().map(|w| w.code).collect()
        }
        _ => vec![],
    }
}

fn recovery_kinds_of_hard(err: &HardError) -> Vec<RecoveryKind> {
    err.actions.iter().map(|a| a.kind).collect()
}

fn recovery_kinds_of_soft(warn: &SoftWarning) -> Vec<RecoveryKind> {
    warn.actions.iter().map(|a| a.kind).collect()
}

fn recovery_kinds_of_advisory(adv: &AIAdvisory) -> Vec<RecoveryKind> {
    adv.actions.iter().map(|a| a.kind).collect()
}
```

- [ ] **Step 4: Verify the project still compiles.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop/src-tauri && cargo build --tests
```

Expected: clean build (the `todo!()` only fires if a test calls these helpers; we haven't written tests yet).

- [ ] **Step 5: Commit.**

```bash
git add apps/desktop/src-tauri/src/core/mod.rs apps/desktop/src-tauri/src/core/validation_matrix.rs
git commit -m "test(t-060): scaffold validation behavior matrix module"
```

---

### Task 3: Verify the existing test-pool helper, then implement seed + baseline

**Files:**
- Modify: `apps/desktop/src-tauri/src/core/validation_matrix.rs`

- [ ] **Step 1: Find the existing in-memory pool helper.**

```bash
grep -rn "async fn fresh_in_memory_pool\|async fn pool\|async fn test_pool\|MIGRATIONS" /Users/robert/Projects/tally.ai/apps/desktop/src-tauri/src/db/
```

- [ ] **Step 2: Read how validation.rs::tests sets up its pool + fixtures.**

```bash
grep -n "async fn\|fn validate_accepts_valid_proposal\|fn soft_warn_future_date" /Users/robert/Projects/tally.ai/apps/desktop/src-tauri/src/core/validation.rs
```

Use `Read` on the lines around `validate_accepts_valid_proposal` and `soft_warn_future_date` to copy the exact seed sequence.

- [ ] **Step 3: Confirm `TransactionProposal` field names.**

Use `Read` on `/Users/robert/Projects/tally.ai/apps/desktop/src-tauri/src/core/proposal.rs` (lines 1–80).

- [ ] **Step 4: Replace the `todo!()` bodies with the real helpers.**

Pattern (adapt names to actual code):

```rust
async fn fresh_pool() -> SqlitePool {
    crate::db::fresh_in_memory_pool().await // or whatever the real helper is
}

async fn seed_household(pool: &SqlitePool) -> SeedIds {
    // Mirror what validation.rs::tests::validate_accepts_valid_proposal does:
    //   1. Insert household.
    //   2. Seed CoA.
    //   3. Find / create the cash account, expense account, grocery envelope.
    // Return their ULIDs.
    SeedIds {
        household_id: /* ... */,
        cash_account_id: /* ... */,
        expense_account_id: /* ... */,
        grocery_envelope_id: /* ... */,
    }
}

fn baseline_proposal_for(seed: &SeedIds) -> TransactionProposal {
    TransactionProposal {
        // Confirmed field names from proposal.rs go here. Example shape:
        payee: "Test Payee".to_string(),
        txn_date_ms: 1_714_000_000_000,
        memo: None,
        lines: vec![
            ProposedLine {
                account_id: seed.expense_account_id.clone(),
                envelope_id: seed.grocery_envelope_id.clone(),
                amount: 1500,
                side: Side::Debit,
                line_memo: None,
            },
            ProposedLine {
                account_id: seed.cash_account_id.clone(),
                envelope_id: None,
                amount: 1500,
                side: Side::Credit,
                line_memo: None,
            },
        ],
        confidence: 0.9,
        confidence_notes: vec![],
        needs_clarification: false,
        clarification_prompt: None,
        advisories: vec![],
    }
}
```

> **Do not invent fields.** If `proposal.rs` has different field names than the example above, use the real names. If a field is missing from this template, add it.

- [ ] **Step 5: Add a smoke test that exercises the seed.**

Inside the same file:

```rust
#[tokio::test]
async fn matrix_baseline_validates_clean() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let proposal = baseline_proposal_for(&seed);
    let result = validate(&pool, &proposal, &seed.household_id).await.unwrap();
    assert!(matches!(result, ValidationResult::Accepted));
}
```

> **Confirm `validate` signature** with `grep -n "pub async fn validate" apps/desktop/src-tauri/src/core/validation.rs`. Adjust args to match (it may take additional context such as `now_ms` for date-based rules).

- [ ] **Step 6: Run.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop/src-tauri && cargo test --lib core::validation_matrix::matrix_baseline_validates_clean
```

Expected: PASS.

- [ ] **Step 7: Commit.**

```bash
git add apps/desktop/src-tauri/src/core/validation_matrix.rs
git commit -m "test(t-060): seed + baseline proposal smoke for matrix"
```

---

### Task 4: Tier 1 (HardError) matrix — 8 variants × {pass, fail, edge}

**Files:**
- Modify: `apps/desktop/src-tauri/src/core/validation_matrix.rs`

For each variant, write three tests in this fixed shape:
1. `tier1_<variant>_triggers` — proposal that hits the rule, asserts `Rejected`, contains the code, recovery actions are correct.
2. `tier1_<variant>_does_not_trigger_when_clean` — variant of the baseline that *does not* trigger the rule, asserts not in result.
3. `tier1_<variant>_edge_<scenario>` — boundary case. Skip with a `// no edge case applicable` comment if not meaningful.

Variants (8): `NoLines`, `UnbalancedLines`, `ZeroAmount`, `NegativeAmount`, `UnknownAccount`, `PlaceholderAccount`, `AbnormalBalance`, `EnvelopeMismatch`.

**Recovery action expectations** — verify against `validation.rs` before asserting; the table below is the expected set:

| Code | Primary RecoveryKind | Other expected kinds |
|---|---|---|
| `NoLines` | `EditField` | `Discard` |
| `UnbalancedLines` | `EditField` | `Discard` |
| `ZeroAmount` | `EditField` | `Discard` |
| `NegativeAmount` | `EditField` | `Discard` |
| `UnknownAccount` | `CreateMissing` | `EditField`, `Discard` |
| `PlaceholderAccount` | `EditField` | `Discard` |
| `AbnormalBalance` | `EditField` | `Discard`, `PostAnyway` (if rule allows override) |
| `EnvelopeMismatch` | `EditField` | `Discard` |

> **Verify the table against the actual code** before asserting in tests. Use `Read` or `grep -B2 -A12 "HardErrorCode::NoLines"` on `validation.rs` for each row. If a row contradicts the code, change the table, not the code (this PR doesn't change behavior).

- [ ] **Step 1: Write `tier1_no_lines_*` tests.**

```rust
#[tokio::test]
async fn tier1_no_lines_triggers() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let mut p = baseline_proposal_for(&seed);
    p.lines.clear();
    let result = validate(&pool, &p, &seed.household_id).await.unwrap();

    assert!(hard_codes(&result).contains(&HardErrorCode::NoLines));
    let err = match &result {
        ValidationResult::Rejected { errors, .. } => {
            errors.iter().find(|e| e.code == HardErrorCode::NoLines).expect("NoLines error")
        }
        _ => panic!("expected Rejected"),
    };
    let kinds = recovery_kinds_of_hard(err);
    assert_eq!(kinds.first().copied(), Some(RecoveryKind::EditField));
    assert!(kinds.contains(&RecoveryKind::Discard));
}

#[tokio::test]
async fn tier1_no_lines_does_not_trigger_when_clean() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let p = baseline_proposal_for(&seed);
    let result = validate(&pool, &p, &seed.household_id).await.unwrap();
    assert!(!hard_codes(&result).contains(&HardErrorCode::NoLines));
}
// no edge case applicable — NoLines is binary
```

- [ ] **Step 2: Run those two tests.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop/src-tauri && cargo test --lib core::validation_matrix::tier1_no_lines
```

Expected: both PASS.

- [ ] **Step 3: Repeat the triple-shape for the remaining 7 variants.**

Mutation cookbook (each row is a one-liner change to `baseline_proposal_for(seed)`):

| Variant | Mutation to trigger | Edge to add |
|---|---|---|
| `UnbalancedLines` | `p.lines[0].amount = 1500; p.lines[1].amount = 1499;` | no edge — 0-cent imbalance is the clean baseline |
| `ZeroAmount` | `p.lines[0].amount = 0; p.lines[1].amount = 0;` | edge: `amount = 1` (one cent) passes |
| `NegativeAmount` | depends on `amount` type — if `i64` use `-1500`; if `u64` skip with comment | none |
| `UnknownAccount` | replace `p.lines[0].account_id` with a fresh ULID not in the seed | none |
| `PlaceholderAccount` | use a placeholder account ULID from the CoA seed (find one with `is_placeholder = TRUE`) | none |
| `AbnormalBalance` | flip side on a normally-debit account, large amount; consult existing `validate_rejects_abnormal_balance` test in `validation.rs` for canonical setup | edge: amount that produces $0.00 abnormal swing — should pass |
| `EnvelopeMismatch` | attach an envelope_id to a non-expense line (e.g., the cash credit line) | none |

> Each mutation must be derived from existing-test patterns in `validation.rs`. Cross-check with `grep -A15 "fn validate_rejects_<name>" apps/desktop/src-tauri/src/core/validation.rs`.

For each variant, follow this code template (substitute the variant name and mutation):

```rust
#[tokio::test]
async fn tier1_<variant>_triggers() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let mut p = baseline_proposal_for(&seed);
    /* mutation */;
    let result = validate(&pool, &p, &seed.household_id).await.unwrap();

    assert!(hard_codes(&result).contains(&HardErrorCode::<Variant>));
    let err = match &result {
        ValidationResult::Rejected { errors, .. } => {
            errors.iter().find(|e| e.code == HardErrorCode::<Variant>).expect("<Variant> error")
        }
        _ => panic!("expected Rejected"),
    };
    let kinds = recovery_kinds_of_hard(err);
    assert_eq!(kinds.first().copied(), Some(RecoveryKind::<PrimaryFromTable>));
    /* assert each "Other expected kind" is present */
}

#[tokio::test]
async fn tier1_<variant>_does_not_trigger_when_clean() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let p = baseline_proposal_for(&seed);
    let result = validate(&pool, &p, &seed.household_id).await.unwrap();
    assert!(!hard_codes(&result).contains(&HardErrorCode::<Variant>));
}

#[tokio::test]
async fn tier1_<variant>_edge_<scenario>() {
    /* boundary scenario from the cookbook */
}
```

- [ ] **Step 4: Run the full Tier 1 batch.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop/src-tauri && cargo test --lib core::validation_matrix::tier1
```

Expected: all PASS. Tier 1 should produce ~17–19 tests (8 variants × 2–3 tests each, minus skipped edges).

- [ ] **Step 5: Commit.**

```bash
git add apps/desktop/src-tauri/src/core/validation_matrix.rs
git commit -m "test(t-060): Tier 1 HardError matrix — 8 variants"
```

---

### Task 5: Tier 2 (SoftWarning) matrix — 5 variants × {trigger, no-trigger, edge}

**Files:**
- Modify: `apps/desktop/src-tauri/src/core/validation_matrix.rs`

Variants: `FutureDate`, `StaleDate`, `LargeAmount`, `EnvelopeOverdraft`, `PossibleDuplicate`.

**Recovery action expectations** (verify against code before asserting):

| Code | Primary | Other |
|---|---|---|
| `FutureDate` | `PostAnyway` | `EditField`, `Discard` |
| `StaleDate` | `PostAnyway` | `EditField`, `Discard` |
| `LargeAmount` | `PostAnyway` | `EditField`, `Discard` |
| `EnvelopeOverdraft` | `PostAnyway` | `EditField`, `Discard` |
| `PossibleDuplicate` | `PostAnyway` | `Discard` |

- [ ] **Step 1: Write `tier2_future_date_*` tests.**

Pattern follows Tier 1 exactly, except the assertion uses `soft_codes(&result)` and the result variant is `Warnings` (not `Rejected`).

```rust
#[tokio::test]
async fn tier2_future_date_triggers() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let mut p = baseline_proposal_for(&seed);
    let now_ms: i64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
    p.txn_date_ms = now_ms + 90 * 24 * 60 * 60 * 1000; // 90 days in future
    let result = validate(&pool, &p, &seed.household_id).await.unwrap();

    assert!(soft_codes(&result).contains(&SoftWarningCode::FutureDate));
    let warn = match &result {
        ValidationResult::Warnings { warnings, .. } => {
            warnings.iter().find(|w| w.code == SoftWarningCode::FutureDate).expect("FutureDate warn")
        }
        ValidationResult::Rejected { warnings, .. } => {
            warnings.iter().find(|w| w.code == SoftWarningCode::FutureDate).expect("FutureDate warn")
        }
        _ => panic!("expected warnings"),
    };
    let kinds = recovery_kinds_of_soft(warn);
    assert_eq!(kinds.first().copied(), Some(RecoveryKind::PostAnyway));
    assert!(kinds.contains(&RecoveryKind::Discard));
}

#[tokio::test]
async fn tier2_future_date_does_not_trigger_at_today() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let mut p = baseline_proposal_for(&seed);
    p.txn_date_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
    let result = validate(&pool, &p, &seed.household_id).await.unwrap();
    assert!(!soft_codes(&result).contains(&SoftWarningCode::FutureDate));
}

#[tokio::test]
async fn tier2_future_date_edge_at_threshold() {
    // Boundary: exactly at the rule's threshold (read it from validation.rs).
    // Assert which side of the boundary triggers/does not, matching the rule's <= or <.
}
```

- [ ] **Step 2: Run them.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop/src-tauri && cargo test --lib core::validation_matrix::tier2_future_date
```

- [ ] **Step 3: Repeat the triple-shape for the other 4 variants.**

Mutation cookbook:

| Variant | Mutation | Edge |
|---|---|---|
| `StaleDate` | `txn_date_ms = now_ms - 400 * 86_400_000;` | Boundary at the rule's threshold (read it from `validation.rs`) |
| `LargeAmount` | both line amounts = the rule's threshold + 1 | Exactly at threshold |
| `EnvelopeOverdraft` | line amount > envelope's `cap_cents` | Equal to cap (warn vs not) |
| `PossibleDuplicate` | seed a posted txn yesterday with same payee/amount/account; then propose same | Different payee → no warn |

> Each rule's threshold is in the corresponding `pub fn soft_warn_*` in `validation.rs`. Read it before writing the edge test.

For each variant, use this template (substitute name and mutation):

```rust
#[tokio::test]
async fn tier2_<variant>_triggers() { /* mutate baseline; assert soft_codes contains; assert recovery primary + others */ }

#[tokio::test]
async fn tier2_<variant>_does_not_trigger_when_clean() { /* clean baseline; assert !contains */ }

#[tokio::test]
async fn tier2_<variant>_edge_<scenario>() { /* boundary */ }
```

- [ ] **Step 4: Run the full Tier 2 batch.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop/src-tauri && cargo test --lib core::validation_matrix::tier2
```

Expected: ~15 tests PASS.

- [ ] **Step 5: Commit.**

```bash
git add apps/desktop/src-tauri/src/core/validation_matrix.rs
git commit -m "test(t-060): Tier 2 SoftWarning matrix — 5 variants"
```

---

### Task 6: Tier 3 (AIAdvisory) matrix — 4 builders × shape assertions

**Files:**
- Modify: `apps/desktop/src-tauri/src/core/validation_matrix.rs`

Tier 3 is different from Tiers 1/2 — advisories are produced by builders in `ai/advisories.rs`. The matrix verifies each builder produces the expected user-facing message and recovery action set.

Builders: `unknown_payee`, `suggested_account`, `possible_duplicate`, `envelope_near_limit`.

**Recovery action expectations:**

| Builder | Primary | Other |
|---|---|---|
| `unknown_payee` | `EditField` | `CreateMissing`, `Discard` |
| `suggested_account` | `UseSuggested` | `EditField`, `Discard` |
| `possible_duplicate` | `PostAnyway` | `Discard` |
| `envelope_near_limit` | `PostAnyway` | `EditField`, `Discard` |

> Verify against `ai/advisories.rs` before asserting.

- [ ] **Step 1: Implement `tier3_unknown_payee_advisory_shape`.**

```rust
#[test]
fn tier3_unknown_payee_advisory_shape() {
    let advisory = advisories::unknown_payee("Trader Joe's");
    assert!(advisory.user_message.contains("Trader Joe's"));
    let kinds = recovery_kinds_of_advisory(&advisory);
    assert_eq!(kinds.first().copied(), Some(RecoveryKind::EditField));
    assert!(kinds.contains(&RecoveryKind::CreateMissing));
    assert!(kinds.contains(&RecoveryKind::Discard));
}
```

- [ ] **Step 2: Add the other three builder shape tests.**

For each builder, mirror the template above with the right primary + others from the table.

```rust
#[test]
fn tier3_suggested_account_advisory_shape() { /* ... */ }

#[test]
fn tier3_possible_duplicate_advisory_shape() { /* ... */ }

#[test]
fn tier3_envelope_near_limit_advisory_shape() { /* ... */ }
```

- [ ] **Step 3: Run.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop/src-tauri && cargo test --lib core::validation_matrix::tier3
```

Expected: 4 PASS.

- [ ] **Step 4: Commit.**

```bash
git add apps/desktop/src-tauri/src/core/validation_matrix.rs
git commit -m "test(t-060): Tier 3 AIAdvisory matrix — 4 builders"
```

---

### Task 7: Update CLAUDE.md status; PR 1 success-criteria check; open PR

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Run the whole matrix.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop/src-tauri && cargo test --lib core::validation_matrix
```

Expected: ~36 tests PASS (8 × 2-3 + 5 × 3 + 4).

- [ ] **Step 2: Confirm the 80% coverage gate still passes.**

Run whatever the pre-commit hook runs. Inspect `.husky/pre-commit` (or `package.json` `scripts.precommit`) if uncertain.

- [ ] **Step 3: Add an Implementation status entry in CLAUDE.md.**

In the "Implementation status" block, add:

```markdown
**Validation behavior matrix (T-060):**
- `core::validation_matrix` is the canonical inventory of validation behaviors.
  Tier 1 (8 HardError variants), Tier 2 (5 SoftWarning variants), Tier 3
  (4 AIAdvisory builders). Every variant has +/- tests asserting the recovery
  action set. New rules MUST add a row to this matrix.
```

- [ ] **Step 4: Commit + push + open PR.**

```bash
git add CLAUDE.md
git commit -m "docs: record T-060 validation matrix in implementation status"
git push -u origin feat/t-060-validation-matrix
gh pr create --title "feat(core): T-060 Rust validation behavior matrix + T-065 doc discipline" --body "$(cat <<'EOF'
## Summary

Adds canonical Rust validation behavior matrix (T-060) covering all current
Tier 1, 2, and 3 variants. Adds the keep-current discipline rule (T-065) to
CLAUDE.md and CONTRIBUTING.md.

- T-060: ~36 new tests under `core::validation_matrix`. Every HardError
  variant (8), every SoftWarning variant (5), every AIAdvisory builder (4)
  has explicit positive + negative + edge (where applicable) tests with
  recovery action set assertions.
- T-065: One-line rule in CLAUDE.md "Code conventions"; matching paragraph
  in CONTRIBUTING.md.

Spec: docs/superpowers/specs/2026-04-26-p2-testing-and-polish-design.md
Plan: docs/superpowers/plans/2026-04-26-p2-testing-and-polish.md

## Test plan

- [x] cargo test --lib core::validation_matrix — all matrix tests pass
- [x] cargo test --lib — full Rust suite green, no regressions
- [x] pnpm test — TS suite unchanged (no code change)
- [x] Pre-commit hook green on every commit
- [x] Coverage ≥ 80% (existing floor)
EOF
)"
```

> **Wait for CI green.** Do not proceed to PR 2 until PR 1 is merged.

---

# PR 2 — `feat(ui): T-064 + T-061 + T-063 — error boundary, component matrix, a11y leaf fixes`

**Branch:** `feat/t-064-t-061-t-063-ui-polish` (off `main` after PR 1 merges).

**Pre-flight:**

```bash
git checkout main && git pull && git checkout -b feat/t-064-t-061-t-063-ui-polish
```

---

### Task 8: Define `RecoveryError` Rust type

**Files:**
- Modify: `apps/desktop/src-tauri/src/error.rs`

- [ ] **Step 1: Read current `error.rs`.**

Use `Read` on the first 80 lines. Note exports: `NonEmpty`, `RecoveryAction`, `RecoveryKind`.

- [ ] **Step 2: Write the failing test for `RecoveryError`.**

In `error.rs::tests` (or appended to existing `#[cfg(test)] mod tests`):

```rust
#[test]
fn recovery_error_serializes_with_message_and_recovery_array() {
    let err = RecoveryError {
        message: "Account does not exist".to_string(),
        recovery: NonEmpty::new(
            RecoveryAction {
                kind: RecoveryKind::CreateMissing,
                label: "Create account".to_string(),
                is_primary: true,
            },
            vec![RecoveryAction {
                kind: RecoveryKind::Discard,
                label: "Discard".to_string(),
                is_primary: false,
            }],
        ),
    };
    let json = serde_json::to_value(&err).unwrap();
    assert_eq!(json["message"], "Account does not exist");
    assert_eq!(json["recovery"][0]["kind"], "CREATE_MISSING");
    assert_eq!(json["recovery"][1]["kind"], "DISCARD");
}

#[test]
fn recovery_error_deserializes_from_screaming_snake_keys() {
    let json = serde_json::json!({
        "message": "x",
        "recovery": [{"kind": "SHOW_HELP", "label": "Help", "is_primary": true}],
    });
    let err: RecoveryError = serde_json::from_value(json).unwrap();
    assert_eq!(err.message, "x");
    assert_eq!(err.recovery.first().kind, RecoveryKind::ShowHelp);
}
```

- [ ] **Step 3: Run; observe failure.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop/src-tauri && cargo test --lib error::tests::recovery_error
```

Expected: FAIL — `RecoveryError` not defined.

- [ ] **Step 4: Implement.**

Add to `error.rs`:

```rust
/// Wire-shape carried by every `Result<T, RecoveryError>` returned from a
/// `#[tauri::command]`. Translated by the frontend `safeInvoke` into a
/// user-facing advisory or an inline error UI.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecoveryError {
    pub message: String,
    pub recovery: NonEmpty<RecoveryAction>,
}

impl RecoveryError {
    pub fn new(message: impl Into<String>, recovery: NonEmpty<RecoveryAction>) -> Self {
        Self {
            message: message.into(),
            recovery,
        }
    }

    pub fn show_help(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            recovery: NonEmpty::new(
                RecoveryAction {
                    kind: RecoveryKind::ShowHelp,
                    label: "Get help".to_string(),
                    is_primary: true,
                },
                vec![],
            ),
        }
    }

    pub fn discard(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            recovery: NonEmpty::new(
                RecoveryAction {
                    kind: RecoveryKind::Discard,
                    label: "Discard".to_string(),
                    is_primary: true,
                },
                vec![],
            ),
        }
    }
}
```

- [ ] **Step 5: Run; verify pass.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop/src-tauri && cargo test --lib error::tests::recovery_error
```

- [ ] **Step 6: Commit.**

```bash
git add apps/desktop/src-tauri/src/error.rs
git commit -m "feat(t-064): add RecoveryError type as command error wire shape"
```

---

### Task 9: Mirror `RecoveryError` in `core-types`

**Files:**
- Modify: `packages/core-types/src/index.ts`
- Modify: `packages/core-types/src/index.test.ts`

- [ ] **Step 1: Locate the `RecoveryAction` block.**

Use `Read` on `packages/core-types/src/index.ts` to find the existing `RecoveryAction` interface.

- [ ] **Step 2: Write a failing test asserting the shape.**

Append to `packages/core-types/src/index.test.ts`:

```ts
import { describe, it, expectTypeOf } from "vitest";
import type { RecoveryError, RecoveryAction } from "./index.js";

describe("RecoveryError", () => {
  it("has message and non-empty recovery tuple", () => {
    const err: RecoveryError = {
      message: "x",
      recovery: [{ kind: "SHOW_HELP", label: "Help", is_primary: true }],
    };
    expectTypeOf(err.recovery).toMatchTypeOf<[RecoveryAction, ...RecoveryAction[]]>();
  });
});
```

- [ ] **Step 3: Run; observe failure.**

```bash
cd /Users/robert/Projects/tally.ai/packages/core-types && pnpm test
```

- [ ] **Step 4: Add the type.**

In `packages/core-types/src/index.ts`, after the existing `RecoveryAction` interface:

```ts
/** Mirrors Rust `RecoveryError` — wire shape for any `Result<T, RecoveryError>` from a Tauri command. */
export interface RecoveryError {
  message: string;
  recovery: [RecoveryAction, ...RecoveryAction[]]; // NonEmpty
}
```

- [ ] **Step 5: Run; verify pass.**

```bash
cd /Users/robert/Projects/tally.ai/packages/core-types && pnpm test
```

- [ ] **Step 6: Commit.**

```bash
git add packages/core-types/src/index.ts packages/core-types/src/index.test.ts
git commit -m "feat(t-064): mirror RecoveryError in core-types"
```

---

### Task 10: Implement `safeInvoke`

**Files:**
- Create: `apps/desktop/src/lib/safeInvoke.ts`
- Create: `apps/desktop/src/lib/safeInvoke.test.ts`

`safeInvoke` has two shapes:
1. `safeInvoke<T>(cmd, args?)` returns `{ ok: true; value: T } | { ok: false; error: RecoveryError }` — for inline handling.
2. `safeInvokeOrAdvise<T>(cmd, args?)` returns `T | null`; on error, dispatches a system advisory chat message via the chat store.

Both share a normalizer that converts whatever Tauri throws into a `RecoveryError`. Tauri serializes `Err(RecoveryError)` as a structured object; non-`RecoveryError` errors (panics, IPC failures) come through as strings — those map to a generic `[ShowHelp, Discard]`.

- [ ] **Step 1: Failing tests.**

Create `apps/desktop/src/lib/safeInvoke.test.ts`:

```ts
import { describe, it, expect, vi } from "vitest";
import { safeInvoke, safeInvokeOrAdvise } from "./safeInvoke";
import type { RecoveryError } from "@tally/core-types";

describe("safeInvoke", () => {
  it("returns ok=true with value on success", async () => {
    const fakeInvoke = vi.fn().mockResolvedValue({ id: "x" });
    const r = await safeInvoke<{ id: string }>("get_thing", undefined, { invoke: fakeInvoke });
    expect(r).toEqual({ ok: true, value: { id: "x" } });
  });

  it("returns ok=false with structured RecoveryError on Tauri Err(RecoveryError)", async () => {
    const recoveryErr: RecoveryError = {
      message: "Account does not exist",
      recovery: [{ kind: "CREATE_MISSING", label: "Create", is_primary: true }],
    };
    const fakeInvoke = vi.fn().mockRejectedValue(recoveryErr);
    const r = await safeInvoke("create_thing", undefined, { invoke: fakeInvoke });
    expect(r.ok).toBe(false);
    if (!r.ok) {
      expect(r.error.message).toBe("Account does not exist");
      expect(r.error.recovery[0].kind).toBe("CREATE_MISSING");
    }
  });

  it("normalizes string errors (panic / IPC) into ShowHelp + Discard", async () => {
    const fakeInvoke = vi.fn().mockRejectedValue("ipc connection failed");
    const r = await safeInvoke("anything", undefined, { invoke: fakeInvoke });
    expect(r.ok).toBe(false);
    if (!r.ok) {
      expect(r.error.message).toBe("ipc connection failed");
      const kinds = r.error.recovery.map(a => a.kind);
      expect(kinds).toContain("SHOW_HELP");
      expect(kinds).toContain("DISCARD");
    }
  });
});

describe("safeInvokeOrAdvise", () => {
  it("returns the value on success", async () => {
    const fakeInvoke = vi.fn().mockResolvedValue(42);
    const dispatch = vi.fn();
    const v = await safeInvokeOrAdvise<number>("get_n", undefined, { invoke: fakeInvoke, dispatchAdvisory: dispatch });
    expect(v).toBe(42);
    expect(dispatch).not.toHaveBeenCalled();
  });

  it("returns null and dispatches an advisory on error", async () => {
    const recoveryErr: RecoveryError = {
      message: "Bang",
      recovery: [{ kind: "SHOW_HELP", label: "Help", is_primary: true }],
    };
    const fakeInvoke = vi.fn().mockRejectedValue(recoveryErr);
    const dispatch = vi.fn();
    const v = await safeInvokeOrAdvise("x", undefined, { invoke: fakeInvoke, dispatchAdvisory: dispatch });
    expect(v).toBeNull();
    expect(dispatch).toHaveBeenCalledWith(recoveryErr);
  });
});
```

- [ ] **Step 2: Run; observe failure.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop && pnpm test src/lib/safeInvoke.test.ts
```

- [ ] **Step 3: Implement.**

Create `apps/desktop/src/lib/safeInvoke.ts`:

```ts
// eslint-disable-next-line no-restricted-imports
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type { RecoveryError, RecoveryAction } from "@tally/core-types";
import { useChatStore } from "../stores/chatStore";

export type Result<T> =
  | { ok: true; value: T }
  | { ok: false; error: RecoveryError };

interface Deps {
  invoke?: typeof tauriInvoke;
  dispatchAdvisory?: (err: RecoveryError) => void;
}

const DEFAULT_RECOVERY: [RecoveryAction, ...RecoveryAction[]] = [
  { kind: "SHOW_HELP", label: "Get help", is_primary: true },
  { kind: "DISCARD", label: "Discard", is_primary: false },
];

function normalize(raw: unknown): RecoveryError {
  if (raw && typeof raw === "object" && "message" in raw && "recovery" in raw) {
    return raw as RecoveryError;
  }
  if (typeof raw === "string") {
    return { message: raw, recovery: DEFAULT_RECOVERY };
  }
  return { message: "Something went wrong.", recovery: DEFAULT_RECOVERY };
}

export async function safeInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
  deps: Deps = {},
): Promise<Result<T>> {
  const invoke = deps.invoke ?? tauriInvoke;
  try {
    const value = await invoke<T>(cmd, args);
    return { ok: true, value };
  } catch (raw) {
    return { ok: false, error: normalize(raw) };
  }
}

export async function safeInvokeOrAdvise<T>(
  cmd: string,
  args?: Record<string, unknown>,
  deps: Deps = {},
): Promise<T | null> {
  const result = await safeInvoke<T>(cmd, args, deps);
  if (result.ok) return result.value;
  const dispatch = deps.dispatchAdvisory ?? defaultDispatch;
  dispatch(result.error);
  return null;
}

function defaultDispatch(err: RecoveryError): void {
  // Dispatched as a system advisory message via the chat store.
  // Task 12 adds appendAdvisory to the store; until then this is a no-op via
  // optional chaining.
  useChatStore.getState().appendAdvisory?.(err);
}
```

- [ ] **Step 4: Run; verify pass.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop && pnpm test src/lib/safeInvoke.test.ts
```

- [ ] **Step 5: Commit.**

```bash
git add apps/desktop/src/lib/safeInvoke.ts apps/desktop/src/lib/safeInvoke.test.ts
git commit -m "feat(t-064): add safeInvoke wrapper for centralized error → RecoveryAction"
```

---

### Task 11: Render-time `<ErrorBoundary>`

**Files:**
- Create: `apps/desktop/src/components/ErrorBoundary.tsx`
- Create: `apps/desktop/src/components/ErrorBoundary.test.tsx`
- Modify: `apps/desktop/src/main.tsx`

- [ ] **Step 1: Failing test.**

```tsx
// apps/desktop/src/components/ErrorBoundary.test.tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { ErrorBoundary } from "./ErrorBoundary";

function Boom(): JSX.Element {
  throw new Error("kaboom");
}

describe("ErrorBoundary", () => {
  it("renders children when no error", () => {
    render(<ErrorBoundary><div>hello</div></ErrorBoundary>);
    expect(screen.getByText("hello")).toBeInTheDocument();
  });

  it("renders a system message with Get help action when a child throws", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    render(<ErrorBoundary><Boom /></ErrorBoundary>);
    expect(screen.getByText(/something went wrong/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /get help/i })).toBeInTheDocument();
    spy.mockRestore();
  });
});
```

- [ ] **Step 2: Run; observe failure.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop && pnpm test src/components/ErrorBoundary.test.tsx
```

- [ ] **Step 3: Implement.**

```tsx
// apps/desktop/src/components/ErrorBoundary.tsx
import { Component, type ReactNode } from "react";

interface Props { children: ReactNode }
interface State { hasError: boolean }

export class ErrorBoundary extends Component<Props, State> {
  state: State = { hasError: false };

  static getDerivedStateFromError(): State {
    return { hasError: true };
  }

  componentDidCatch(error: unknown): void {
    console.error("ErrorBoundary caught:", error);
  }

  render(): ReactNode {
    if (this.state.hasError) {
      return (
        <div role="alert" aria-live="assertive" className="error-boundary">
          <p>Something went wrong.</p>
          <button type="button" onClick={() => window.location.reload()}>
            Get help
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
```

- [ ] **Step 4: Run; verify pass.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop && pnpm test src/components/ErrorBoundary.test.tsx
```

- [ ] **Step 5: Wrap `<App>` in `main.tsx`.**

Use `Read` on `apps/desktop/src/main.tsx`. Wrap the existing root render in `<ErrorBoundary>`:

```tsx
import { ErrorBoundary } from "./components/ErrorBoundary";
// ...
ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ErrorBoundary>
      {/* existing providers, e.g. <QueryClientProvider> */}
      <App />
      {/* end existing providers */}
    </ErrorBoundary>
  </React.StrictMode>,
);
```

- [ ] **Step 6: Run full TS suite.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop && pnpm test
```

- [ ] **Step 7: Commit.**

```bash
git add apps/desktop/src/components/ErrorBoundary.tsx apps/desktop/src/components/ErrorBoundary.test.tsx apps/desktop/src/main.tsx
git commit -m "feat(t-064): add render-time ErrorBoundary wrapping App"
```

---

### Task 12: Add `appendAdvisory` to chat store

**Files:**
- Modify: `apps/desktop/src/stores/chatStore.ts`
- Modify or Create: `apps/desktop/src/stores/chatStore.test.ts`

The store action takes a `RecoveryError`, appends a system advisory message to the chat thread.

- [ ] **Step 1: Read current store shape.**

Use `Read` on `apps/desktop/src/stores/chatStore.ts`. Find the existing append-message action; identify the message-kind discriminator used for system messages or proactive advisories.

- [ ] **Step 2: Failing test.**

In `chatStore.test.ts`:

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { useChatStore } from "./chatStore";

describe("appendAdvisory", () => {
  beforeEach(() => useChatStore.getState().reset?.()); // adjust if no reset exists

  it("appends a system advisory message with the recovery actions", () => {
    useChatStore.getState().appendAdvisory({
      message: "Boom",
      recovery: [{ kind: "SHOW_HELP", label: "Get help", is_primary: true }],
    });
    const msgs = useChatStore.getState().messages;
    const last = msgs[msgs.length - 1];
    expect(last.kind).toBe("system_advisory"); // or whatever kind name fits the existing union
  });
});
```

- [ ] **Step 3: Implement** — add `appendAdvisory(err: RecoveryError)` to the store actions; emit a `system_advisory` (or `proactive_advisory` if that already exists in the message-kind union — pick whichever matches; do not invent a new kind unless necessary).

If a new message kind is required, also update `MessageList.tsx` to render it.

- [ ] **Step 4: Run.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop && pnpm test src/stores/chatStore.test.ts
```

- [ ] **Step 5: Commit.**

```bash
git add apps/desktop/src/stores/chatStore.ts apps/desktop/src/stores/chatStore.test.ts apps/desktop/src/components/chat/MessageList.tsx
git commit -m "feat(t-064): add appendAdvisory store action for safeInvokeOrAdvise"
```

---

### Task 13: Migrate every Rust command to `Result<T, RecoveryError>`

**Files:**
- Modify: `apps/desktop/src-tauri/src/commands/mod.rs`

There are 24 `#[tauri::command]` functions in `commands/mod.rs`. Goal: every command returns `Result<T, RecoveryError>`.

Pattern for migrating one command:

**Before:**
```rust
#[tauri::command]
pub async fn create_account(
    state: tauri::State<'_, AppState>,
    name: String,
    account_type: String,
) -> Result<String, String> {
    something().map_err(|e| e.to_string())
}
```

**After:**
```rust
#[tauri::command]
pub async fn create_account(
    state: tauri::State<'_, AppState>,
    name: String,
    account_type: String,
) -> Result<String, RecoveryError> {
    something().map_err(|e| {
        RecoveryError::show_help(format!("Could not create account: {e}"))
    })
}
```

Use the helpers `RecoveryError::show_help(msg)` and `RecoveryError::discard(msg)` for generic errors. For known-shaped errors (validation failures, account-not-found), build a `RecoveryError` with the specific recovery actions the user can take.

- [ ] **Step 1: Inventory.**

```bash
grep -A2 "#\[tauri::command\]" /Users/robert/Projects/tally.ai/apps/desktop/src-tauri/src/commands/mod.rs | grep "pub async fn\|pub fn"
```

List the 24 commands with their current return types.

- [ ] **Step 2: Migrate one command end-to-end as the template.**

Pick `create_account` (or whichever is simplest). Update the signature and the error mappers. Add `use crate::error::RecoveryError;` at the top of the file if not already imported.

- [ ] **Step 3: Run that command's test.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop/src-tauri && cargo test --lib commands
```

If tests fail because they assert on `String` errors, update the assertions to match the new `RecoveryError` shape.

- [ ] **Step 4: Commit the template.**

```bash
git add apps/desktop/src-tauri/src/commands/mod.rs
git commit -m "feat(t-064): migrate create_account to Result<T, RecoveryError> (template)"
```

- [ ] **Step 5: Repeat for the other 23 commands in batches.**

Suggested batches (one commit per batch):
1. **Account commands** — `create_household`, `set_opening_balance`, `create_envelope`.
2. **AI defaults / chat persistence** — `get_ai_defaults`, `append_chat_message`, `list_chat_messages`.
3. **API key** — `set_api_key`, `has_api_key`, `delete_api_key`.
4. **Chat I/O** — `submit_message`, `commit_proposal`, `undo_last_transaction`.
5. **Sidebar reads** — `get_account_balances`, `get_current_envelope_periods`, `get_pending_transactions`.
6. **GnuCash** — `read_gnucash_file`, `gnucash_build_default_plan`, `gnucash_apply_mapping_edit`, `commit_gnucash_import`, `rollback_gnucash_import`, `reconcile_gnucash_import`.
7. **Setup / hledger** — `check_setup_status`, `import_hledger`.

For each batch:
- Apply the same pattern: change signature → update error mappers → fix tests.
- Run `cargo test` after each batch; do not commit if tests fail.

- [ ] **Step 6: Verify full Rust suite passes.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop/src-tauri && cargo test
```

---

### Task 14: Migrate every TS call site to `safeInvoke`

**Files:**
- Modify: `apps/desktop/src/hooks/useSendMessage.ts`
- Modify: `apps/desktop/src/hooks/useCommitProposal.ts`
- Modify: `apps/desktop/src/hooks/useSidebarData.ts`
- Modify: any other file importing `invoke` from `@tauri-apps/api/core`

- [ ] **Step 1: List the call sites.**

```bash
grep -rn "from \"@tauri-apps/api/core\"\|from '@tauri-apps/api/core'" /Users/robert/Projects/tally.ai/apps/desktop/src
```

- [ ] **Step 2: Migrate `useCommitProposal.ts` first** (uses `safeInvoke`'s `Result` shape because it has card-local error handling).

```ts
import { safeInvoke } from "../lib/safeInvoke";
// ...
const r = await safeInvoke<CommitOutcome>("commit_proposal", { proposalJson });
if (!r.ok) {
  return { kind: "rejected", error: r.error };
}
const outcome = r.value;
```

- [ ] **Step 3: Run the hook's test.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop && pnpm test src/hooks/useCommitProposal.test.tsx
```

Update mocks where needed: tests likely call `invoke.mockRejectedValue(...)` — those still work because `safeInvoke` calls into the same `invoke`. Tests asserting on a thrown error must change to assert on the `Result` shape.

- [ ] **Step 4: Migrate `useSendMessage.ts`** — uses `safeInvokeOrAdvise` because there is no card-local error UI.

```ts
import { safeInvokeOrAdvise } from "../lib/safeInvoke";
// ...
const response = await safeInvokeOrAdvise<MessageResponse>("submit_message", { ...args });
if (response === null) return; // advisory already dispatched
// ... use response
```

- [ ] **Step 5: Migrate `useSidebarData.ts`** — TanStack Query setup. Convert to a thrown error so React Query treats it as a query failure.

```ts
queryFn: async () => {
  const r = await safeInvoke<AccountBalance[]>("get_account_balances");
  if (!r.ok) throw r.error;
  return r.value;
},
```

- [ ] **Step 6: Repeat for any remaining direct `invoke` call sites.**

- [ ] **Step 7: Run the full TS suite.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop && pnpm test
```

- [ ] **Step 8: Add an ESLint rule preventing direct `invoke` imports.**

Inspect existing ESLint config (`apps/desktop/.eslintrc.cjs` or `eslint.config.js`). Add to the `rules` block:

```js
"no-restricted-imports": ["error", {
  paths: [{
    name: "@tauri-apps/api/core",
    importNames: ["invoke"],
    message: "Use safeInvoke / safeInvokeOrAdvise from src/lib/safeInvoke.ts.",
  }],
}],
```

`safeInvoke.ts` itself is exempted by the `// eslint-disable-next-line no-restricted-imports` comment placed in Task 10.

- [ ] **Step 9: Run lint.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop && pnpm lint
```

- [ ] **Step 10: Commit.**

```bash
git add apps/desktop/src apps/desktop/.eslintrc.cjs
git commit -m "feat(t-064): migrate every invoke site to safeInvoke + add eslint guard"
```

---

### Task 15: Add axe-core test helper

**Files:**
- Create: `apps/desktop/src/test/axe.ts`
- Create: `apps/desktop/src/test/axe.test.tsx`
- Modify: `apps/desktop/package.json`

- [ ] **Step 1: Install `axe-core`.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop && pnpm add -D axe-core
```

- [ ] **Step 2: Create the helper.**

```ts
// apps/desktop/src/test/axe.ts
import axe, { type AxeResults, type RunOptions } from "axe-core";

const RULE_OVERRIDES: NonNullable<RunOptions["rules"]> = {
  // Disabled rules go here, each with a short reason that maps to
  // docs/superpowers/a11y-2026-04.md. Empty until audit is run.
};

export async function checkA11y(container: Element): Promise<AxeResults> {
  return axe.run(container, { rules: RULE_OVERRIDES });
}

export function expectNoA11yViolations(results: AxeResults): void {
  if (results.violations.length === 0) return;
  const rendered = results.violations
    .map(v => `[${v.id}] ${v.description}\n  ${v.nodes.map(n => n.html).join("\n  ")}`)
    .join("\n\n");
  throw new Error(`a11y violations:\n${rendered}`);
}
```

- [ ] **Step 3: Smoke-test against a known-clean component.**

```tsx
// apps/desktop/src/test/axe.test.tsx
import { render } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { checkA11y, expectNoA11yViolations } from "./axe";

describe("axe helper", () => {
  it("passes on a clean button", async () => {
    const { container } = render(<button type="button">Hello</button>);
    const results = await checkA11y(container);
    expectNoA11yViolations(results);
    expect(results.violations).toEqual([]);
  });
});
```

- [ ] **Step 4: Run.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop && pnpm test src/test/axe.test.tsx
```

- [ ] **Step 5: Commit.**

```bash
git add apps/desktop/package.json apps/desktop/src/test/axe.ts apps/desktop/src/test/axe.test.tsx pnpm-lock.yaml
git commit -m "test(t-061): add axe-core helper for a11y assertions"
```

---

### Task 16: Component matrix — `MATRIX.md` + tests

**Files:**
- Create: `apps/desktop/src/__tests__/MATRIX.md`
- Modify/Create: test file alongside each listed component.

- [ ] **Step 1: Write `MATRIX.md`.**

```markdown
# React Component Behavior Matrix — T-061

Every component listed here has a test file enforcing the requirements below.
Every test wraps render with `expectNoA11yViolations(await checkA11y(container))`.

## TransactionCard (`src/components/chat/TransactionCard.tsx`)
- Render in 4 states: `posted`, `pending`, `voided`, `correction-pair`.
- Info-circle is visible and has an aria-label.
- Journal-line drawer toggles; lines render with debit/credit and amount.
- Card-local error renders with message + first recovery action label.

## ChatThread (`src/components/chat/ChatThread.tsx`)
- Renders messages of every kind currently in the union.
- Date separators between days.
- Auto-scroll to bottom on new message.
- New-message pill appears when scrolled up + a new message arrives.
- Infinite-scroll callback fires when scrolled to top.

## InputBar (`src/components/input/InputBar.tsx`)
- Slash palette filters as user types.
- Arrow keys navigate palette; Enter selects.
- Chip strip renders chips from store; clicking dismisses.
- Textarea grows with content up to a max height.

## safeInvoke (`src/lib/safeInvoke.ts`) — covered in Task 10.

## ErrorBoundary (`src/components/ErrorBoundary.tsx`) — covered in Task 11.
```

- [ ] **Step 2: Read the current TransactionCard implementation and existing tests.**

Use `Read` on `apps/desktop/src/components/chat/TransactionCard.tsx` and on `TransactionCard.test.tsx` (if it exists). Identify the prop shape.

- [ ] **Step 3: Write tests for `TransactionCard` covering all 4 states + axe.**

Augment the existing test file. Each `describe` block matches a state:

```tsx
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect } from "vitest";
import { TransactionCard } from "./TransactionCard";
import { checkA11y, expectNoA11yViolations } from "../../test/axe";

const baseProps = { /* read TransactionCard.tsx; copy minimal fixture */ };

describe("TransactionCard — posted", () => {
  it("renders payee, date, lines", () => { /* ... */ });
  it("info circle has aria-label", () => { /* ... */ });
  it("journal-line drawer toggles", async () => { /* ... */ });
  it("passes axe", async () => {
    const { container } = render(<TransactionCard state="posted" {...baseProps} />);
    expectNoA11yViolations(await checkA11y(container));
  });
});

describe("TransactionCard — pending", () => {
  it("shows Confirm and Discard buttons", () => { /* ... */ });
  it("renders card-local error when commit rejects", () => { /* ... */ });
  it("passes axe", async () => { /* ... */ });
});

describe("TransactionCard — voided", () => { /* ... */ });
describe("TransactionCard — correction-pair", () => { /* ... */ });
```

> **Read the component before writing selectors.** Adjust labels, roles, and prop shapes to match the real DOM.

- [ ] **Step 4: Run.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop && pnpm test src/components/chat/TransactionCard.test.tsx
```

- [ ] **Step 5: Repeat for `ChatThread` and `InputBar`.**

For each, use the matrix item list as the test inventory. Augment existing tests; do not duplicate.

- [ ] **Step 6: Run the full TS suite.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop && pnpm test
```

Coverage stays ≥ 80%.

- [ ] **Step 7: Commit.**

```bash
git add apps/desktop/src/__tests__/MATRIX.md apps/desktop/src/components
git commit -m "test(t-061): React component behavior matrix + axe assertions"
```

---

### Task 17: Run a11y audit and produce `a11y-2026-04.md`

**Files:**
- Create: `docs/superpowers/a11y-2026-04.md`

- [ ] **Step 1: Run the dev app.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop && pnpm tauri dev
```

In another terminal:

- [ ] **Step 2: Run Lighthouse on the webview URL** (whatever port Tauri's webview uses; usually `http://localhost:1420`). Alternatively use the **axe DevTools** browser extension.

- [ ] **Step 3: Walk every primary UI surface** and record findings in this template:

```markdown
# A11y Audit — 2026-04

Tooling: axe DevTools 4.x + Lighthouse 12.x against `pnpm tauri dev`.

| # | Surface | Finding | Severity | Status | Ticket |
|---|---|---|---|---|---|
| 1 | Sidebar toggle button | No aria-label on icon-only button | Serious | Fixed in this PR | — |
| 2 | Slash command palette | Tab cycles out of palette mid-list | Moderate | Fixed in this PR | — |
| 3 | Streaming chat tokens | No live-region announcement | Moderate | Deferred | #NNN |
| ... | | | | | |

## Disabled axe rules

| Rule ID | Reason |
|---|---|
| (none yet) | |
```

- [ ] **Step 4: Commit the audit doc.**

```bash
git add docs/superpowers/a11y-2026-04.md
git commit -m "docs(t-063): a11y audit findings — 2026-04"
```

---

### Task 18: Apply leaf-level a11y fixes; file deferred items

**Files:**
- Modify: components flagged "Fixed in this PR" in the audit.
- Modify: `docs/superpowers/a11y-2026-04.md` (status updates + ticket #s).

- [ ] **Step 1: For each "Fixed in this PR" row in the audit:**

- Add `aria-label` on icon-only buttons.
- Add `:focus-visible` ring (use existing token; if none, define one in `index.css`).
- Adjust contrast tokens where Lighthouse flagged failures.
- Verify keyboard parity on slash palette.
- Add `prefers-reduced-motion` query to auto-scroll behavior in `ChatThread.tsx`:
  ```css
  @media (prefers-reduced-motion: reduce) {
    .chat-thread { scroll-behavior: auto !important; }
  }
  ```
- Ensure tab order on onboarding cards goes input → confirm → cancel.

- [ ] **Step 2: Re-run the component tests** (axe assertions catch regressions).

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop && pnpm test
```

- [ ] **Step 3: Re-run the dev app and re-audit** with axe DevTools to confirm fixed rows are clean.

- [ ] **Step 4: Update the audit doc** — mark fixed rows ✅, leave deferred rows as-is.

- [ ] **Step 5: File deferred items as Phase 2 GitHub issues.**

```bash
gh issue create \
  --title "T-Pxxx: Streaming chat token live-region announcement" \
  --body "Deferred from T-063 a11y pass on 2026-04. See docs/superpowers/a11y-2026-04.md row #3." \
  --label P2,ui
```

Repeat per deferred row. Update the audit doc with the new ticket numbers in the "Ticket" column.

- [ ] **Step 6: Commit.**

```bash
git add apps/desktop/src docs/superpowers/a11y-2026-04.md
git commit -m "fix(t-063): a11y leaf fixes — aria-labels, focus, contrast, reduced-motion"
```

---

### Task 19: Update CLAUDE.md status; PR 2 success-criteria check; open PR

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Run the full test suite + lint.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop && pnpm test && pnpm lint
cd /Users/robert/Projects/tally.ai/apps/desktop/src-tauri && cargo test
```

- [ ] **Step 2: Confirm 80% coverage gate passes.**

- [ ] **Step 3: Add Implementation status entries.**

```markdown
**safeInvoke + ErrorBoundary (T-064):**
- `apps/desktop/src/lib/safeInvoke.ts` is the single surface for translating
  Tauri command errors into `RecoveryError` (`{ message, recovery: NonEmpty<RecoveryAction> }`).
- Every `#[tauri::command]` returns `Result<T, RecoveryError>`.
- ESLint guard prevents direct imports of `invoke` from `@tauri-apps/api/core`.
- Render-time `<ErrorBoundary>` wraps `<App>` as the safety net for crashes.

**Component behavior matrix + a11y leaf fixes (T-061, T-063):**
- `apps/desktop/src/__tests__/MATRIX.md` lists every component requirement.
- Component tests wrap render with `expectNoA11yViolations(await checkA11y(...))`.
- A11y audit findings live in `docs/superpowers/a11y-2026-04.md`.
- Phase 2 follow-ups for structural items filed as separate tickets.
```

- [ ] **Step 4: Commit + push + open PR.**

```bash
git add CLAUDE.md
git commit -m "docs: record T-061/T-063/T-064 in implementation status"
git push -u origin feat/t-064-t-061-t-063-ui-polish
gh pr create --title "feat(ui): T-064 + T-061 + T-063 — error boundary, component matrix, a11y leaf fixes" --body "$(cat <<'EOF'
## Summary

- T-064: New safeInvoke / safeInvokeOrAdvise wrapper. Every #[tauri::command]
  migrated to Result<T, RecoveryError>. New RecoveryError type in core-types.
  Render-time ErrorBoundary wraps App. ESLint guard prevents direct invoke
  imports.
- T-061: apps/desktop/src/__tests__/MATRIX.md defines the React component
  test inventory. Every component test asserts no axe-core violations via
  the new src/test/axe.ts helper.
- T-063: A11y audit captured in docs/superpowers/a11y-2026-04.md. Leaf-level
  fixes applied (aria-labels, focus-visible, contrast, reduced-motion, tab
  order). Structural items deferred as Phase 2 tickets (linked in audit doc).

Spec: docs/superpowers/specs/2026-04-26-p2-testing-and-polish-design.md
Plan: docs/superpowers/plans/2026-04-26-p2-testing-and-polish.md

## Test plan

- [x] cargo test — 24 commands return Result<T, RecoveryError>
- [x] pnpm test — component matrix + axe assertions clean
- [x] pnpm lint — eslint guard prevents direct invoke imports
- [x] Pre-commit hook green on every commit
- [ ] Manual: trigger a Rust panic → ErrorBoundary catches it
- [ ] Manual: trigger a structured RecoveryError from commit_proposal →
      card-local error renders the recovery action
EOF
)"
```

> **Wait for CI green.** Do not start PR 3 until PR 2 merges.

---

# PR 3 — `test(e2e): T-062 — Playwright + Rust orchestrator integration`

**Branch:** `test/t-062-e2e` (off `main` after PR 2 merges).

```bash
git checkout main && git pull && git checkout -b test/t-062-e2e
```

---

### Task 20: Install Playwright; scaffold config

**Files:**
- Modify: `apps/desktop/package.json`
- Create: `apps/desktop/playwright.config.ts`
- Create: `apps/desktop/e2e/smoke.spec.ts`

- [ ] **Step 1: Install.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop && pnpm add -D @playwright/test
pnpm exec playwright install chromium
```

- [ ] **Step 2: Add `test:e2e` script** to `apps/desktop/package.json`:

```json
{
  "scripts": {
    "test:e2e": "playwright test",
    "test:e2e:ui": "playwright test --ui"
  }
}
```

- [ ] **Step 3: Create `apps/desktop/playwright.config.ts`.**

```ts
import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  timeout: 30_000,
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? "line" : "list",
  use: {
    baseURL: "http://localhost:1420",
    trace: "on-first-retry",
  },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
  ],
  webServer: {
    command: "pnpm dev -- --mode test --port 1420",
    url: "http://localhost:1420",
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
```

- [ ] **Step 4: Smoke test.**

Create `apps/desktop/e2e/smoke.spec.ts`:

```ts
import { test, expect } from "@playwright/test";

test("loads the app shell", async ({ page }) => {
  await page.goto("/");
  await expect(page).toHaveTitle(/tally/i);
});
```

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop && pnpm test:e2e e2e/smoke.spec.ts
```

Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add apps/desktop/package.json apps/desktop/playwright.config.ts apps/desktop/e2e/smoke.spec.ts pnpm-lock.yaml
git commit -m "test(t-062): scaffold Playwright + smoke test"
```

---

### Task 21: Mock-`invoke` injector + fixtures + contract test

**Files:**
- Create: `apps/desktop/e2e/setup.ts`
- Create: `apps/desktop/e2e/fixtures/responses.ts`
- Create: `apps/desktop/e2e/contract.spec.ts`
- Modify: `apps/desktop/src/main.tsx`

- [ ] **Step 1: Add the test-mode branch in `main.tsx`.**

```tsx
if (import.meta.env.MODE === "test") {
  // Top-level await may not be available — wrap in a sync IIFE that
  // schedules the install before the React mount.
  void (async () => {
    const { installMockInvoke } = await import("../e2e/setup");
    installMockInvoke();
  })();
}
```

> If the build complains about `../e2e` outside the project root, add `e2e` to `tsconfig.json` `include` (test mode only) or use a `?test` query suffix and a Vite alias.

- [ ] **Step 2: Implement `installMockInvoke`.**

```ts
// apps/desktop/e2e/setup.ts
import { responses, type CommandName } from "./fixtures/responses";

export function installMockInvoke(): void {
  const handler = async (cmd: CommandName, args: unknown): Promise<unknown> => {
    const responder = responses[cmd];
    if (!responder) {
      throw {
        message: `mock-invoke: no responder for ${cmd}`,
        recovery: [{ kind: "SHOW_HELP", label: "Get help", is_primary: true }],
      };
    }
    return responder(args);
  };
  (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
    invoke: handler,
    transformCallback: (cb: unknown) => cb,
  };
}
```

> **Confirm `__TAURI_INTERNALS__` shape** by inspecting `node_modules/@tauri-apps/api/core.js`. The mock must satisfy whatever properties `invoke()` reaches into. If shape changed in a later Tauri version, adapt.

- [ ] **Step 3: Implement `fixtures/responses.ts` with one entry per command.**

```ts
// apps/desktop/e2e/fixtures/responses.ts

export type CommandName =
  | "check_setup_status"
  | "create_household"
  | "create_account"
  | "set_opening_balance"
  | "create_envelope"
  | "import_hledger"
  | "get_ai_defaults"
  | "undo_last_transaction"
  | "append_chat_message"
  | "list_chat_messages"
  | "set_api_key"
  | "has_api_key"
  | "delete_api_key"
  | "submit_message"
  | "commit_proposal"
  | "get_account_balances"
  | "get_current_envelope_periods"
  | "get_pending_transactions"
  | "read_gnucash_file"
  | "gnucash_build_default_plan"
  | "gnucash_apply_mapping_edit"
  | "commit_gnucash_import"
  | "rollback_gnucash_import"
  | "reconcile_gnucash_import";

export const responses: Record<CommandName, (args: unknown) => Promise<unknown>> = {
  check_setup_status: async () => ({ has_household: false, has_api_key: false }),
  create_household: async () => "01HOUSEHOLDULID000000000000",
  // ... fill out every command with realistic shapes
};
```

> Each fixture's return shape must match what the Rust command actually returns. Cross-check by reading the command's signature in `commands/mod.rs`.

- [ ] **Step 4: Contract test reads the Rust source and asserts the mock has each command.**

```ts
// apps/desktop/e2e/contract.spec.ts
import { test, expect } from "@playwright/test";
import { responses } from "./fixtures/responses";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const COMMANDS_FILE = resolve(__dirname, "../src-tauri/src/commands/mod.rs");

test("mock-invoke fixtures cover every #[tauri::command]", () => {
  const source = readFileSync(COMMANDS_FILE, "utf-8");
  const lines = source.split("\n");

  // Find every #[tauri::command] attribute and pair it with the next
  // pub (async) fn <name>(...) line.
  const realCommands = new Set<string>();
  for (let i = 0; i < lines.length; i++) {
    if (!lines[i].includes("#[tauri::command]")) continue;
    for (let j = i + 1; j < Math.min(i + 5, lines.length); j++) {
      const m = lines[j].match(/^\s*pub\s+(?:async\s+)?fn\s+([a-z_][a-z0-9_]*)\s*\(/);
      if (m) {
        realCommands.add(m[1]);
        break;
      }
    }
  }

  const mockCommands = new Set(Object.keys(responses));

  const missing = [...realCommands].filter(c => !mockCommands.has(c));
  expect(missing, "Rust commands without a mock responder").toEqual([]);

  const extra = [...mockCommands].filter(c => !realCommands.has(c));
  expect(extra, "mock responders for non-existent Rust commands").toEqual([]);
});
```

- [ ] **Step 5: Run.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop && pnpm test:e2e e2e/contract.spec.ts
```

Expected: PASS. If a command is missing from the mock, the test fails with the specific name.

- [ ] **Step 6: Commit.**

```bash
git add apps/desktop/e2e apps/desktop/src/main.tsx
git commit -m "test(t-062): mock-invoke injector + fixtures + contract test"
```

---

### Task 22: Onboarding fresh-start E2E flow

**Files:**
- Create: `apps/desktop/e2e/onboarding.spec.ts`

- [ ] **Step 1: Write the spec.**

```ts
import { test, expect } from "@playwright/test";

test("fresh-start onboarding completes through handoff", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText(/welcome/i)).toBeVisible();

  // Household
  await page.getByLabel(/household name/i).fill("Test Family");
  await page.getByLabel(/timezone/i).selectOption("America/Chicago");
  await page.getByRole("button", { name: /continue/i }).click();

  // Passphrase
  await page.getByLabel(/passphrase/i).fill("correct horse battery staple");
  await page.getByRole("button", { name: /continue/i }).click();

  // API key
  await page.getByLabel(/api key/i).fill("sk-ant-test");
  await page.getByRole("button", { name: /continue/i }).click();

  // Account creation
  await page.getByRole("button", { name: /add account/i }).click();
  // ... fill in account name, type, opening balance per the actual UI

  // Envelopes
  // ... per actual UI

  // Handoff
  await expect(page.getByText(/all set/i)).toBeVisible();
  await expect(page.getByText(/test family/i)).toBeVisible();
});
```

> **Selectors must match real DOM.** Open the dev app, inspect each onboarding step, prefer `getByLabel` and `getByRole` over test ids.

- [ ] **Step 2: Adjust mock fixtures** in `e2e/fixtures/responses.ts` so the chain returns realistic ULIDs and the handoff has data.

- [ ] **Step 3: Run.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop && pnpm test:e2e e2e/onboarding.spec.ts
```

- [ ] **Step 4: Commit.**

```bash
git add apps/desktop/e2e
git commit -m "test(t-062): onboarding fresh-start E2E flow"
```

---

### Task 23: Entry happy-path E2E

**Files:**
- Create: `apps/desktop/e2e/entry.spec.ts`

- [ ] **Step 1: Write.**

```ts
import { test, expect } from "@playwright/test";

test("user enters an expense and confirms it posts", async ({ page }) => {
  // Pre-condition: mock has check_setup_status return has_household=true.
  await page.goto("/");

  await page.getByLabel(/message/i).fill("paid 12.50 for coffee from cash");
  await page.keyboard.press("Enter");

  await expect(page.getByText(/coffee/i)).toBeVisible();
  await expect(page.getByRole("button", { name: /confirm/i })).toBeVisible();

  await page.getByRole("button", { name: /confirm/i }).click();

  await expect(page.getByText(/posted/i)).toBeVisible();
});
```

- [ ] **Step 2: Tune mock fixtures** so `submit_message` returns a `TransactionProposal` and `commit_proposal` returns a posted outcome.

- [ ] **Step 3: Run, commit.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop && pnpm test:e2e e2e/entry.spec.ts
git add apps/desktop/e2e
git commit -m "test(t-062): entry happy-path E2E flow"
```

---

### Task 24: `/fix` flow E2E

**Files:**
- Create: `apps/desktop/e2e/fix.spec.ts`

- [ ] **Step 1: Write.**

```ts
import { test, expect } from "@playwright/test";

test("/fix on a posted transaction creates a correction", async ({ page }) => {
  // Mock has list_chat_messages returning a single posted transaction.
  await page.goto("/");
  await page.getByLabel(/message/i).fill("/fix");
  await page.keyboard.press("Enter");
  await expect(page.getByText(/correct/i)).toBeVisible();
  await page.getByRole("button", { name: /confirm/i }).click();
  await expect(page.getByText(/correction/i)).toBeVisible();
});
```

- [ ] **Step 2: Tune fixtures.**

- [ ] **Step 3: Run, commit.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop && pnpm test:e2e e2e/fix.spec.ts
git add apps/desktop/e2e
git commit -m "test(t-062): /fix flow E2E"
```

---

### Task 25: `/undo` flow E2E

**Files:**
- Create: `apps/desktop/e2e/undo.spec.ts`

- [ ] **Step 1: Write.**

```ts
import { test, expect } from "@playwright/test";

test("/undo last entry reverses the most recent posted transaction", async ({ page }) => {
  await page.goto("/");
  await page.getByLabel(/message/i).fill("/undo");
  await page.keyboard.press("Enter");
  await page.getByRole("button", { name: /confirm undo/i }).click();
  await expect(page.getByText(/reversed/i)).toBeVisible();
});
```

- [ ] **Step 2: Tune fixtures.**

- [ ] **Step 3: Run, commit.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop && pnpm test:e2e e2e/undo.spec.ts
git add apps/desktop/e2e
git commit -m "test(t-062): /undo flow E2E"
```

---

### Task 26: Rust integration suite

**Files:**
- Create: `apps/desktop/src-tauri/tests/orchestrator_integration.rs`
- Create: `apps/desktop/src-tauri/tests/common/mod.rs`

- [ ] **Step 1: Read existing integration test for reference.**

Use `Read` on `apps/desktop/src-tauri/tests/gnucash_import_integration.rs` (lines 1–80). Note the temp-DB setup pattern.

- [ ] **Step 2: Implement `MockClaudeAdapter`.**

```rust
// apps/desktop/src-tauri/tests/common/mod.rs
use async_trait::async_trait;
use tally_desktop_lib::ai::adapter::{ClaudeAdapter, ClaudeAdapterError};
use tally_desktop_lib::core::proposal::TransactionProposal;

pub struct MockClaudeAdapter {
    pub canned: Vec<TransactionProposal>,
}

#[async_trait]
impl ClaudeAdapter for MockClaudeAdapter {
    async fn propose(&self, _prompt: &str) -> Result<TransactionProposal, ClaudeAdapterError> {
        self.canned.first().cloned().ok_or(ClaudeAdapterError::NoResponse)
    }
}
```

> Confirm the actual `ClaudeAdapter` trait signature with `grep -A10 "trait ClaudeAdapter\|pub trait" apps/desktop/src-tauri/src/ai/adapter/claude.rs`. Adapt `propose` to whatever the real method is named (`generate`, `complete`, etc.) and adjust the error type.

- [ ] **Step 3: Write `orchestrator_integration.rs` with one test per flow.**

```rust
mod common;

use common::MockClaudeAdapter;
use tally_desktop_lib::ai::orchestrator;
use tally_desktop_lib::core::proposal::TransactionProposal;

async fn fresh_db() -> sqlx::SqlitePool {
    // Mirror gnucash_import_integration.rs::fresh_db pattern: temp-file
    // SQLCipher pool with migrations applied.
    todo!()
}

#[tokio::test]
async fn entry_happy_path_posts_a_transaction() {
    let pool = fresh_db().await;
    // seed household + accounts + envelope (use the same helpers as in core::validation_matrix)
    let canned: Vec<TransactionProposal> = vec![/* TransactionProposal for "paid 12.50 for coffee" */];
    let adapter = MockClaudeAdapter { canned };
    let response = orchestrator::submit_message(&pool, &adapter, "...household_id...", "paid 12.50 for coffee from cash").await.unwrap();
    // assert validation accepted; commit produced posted txn
}

#[tokio::test]
async fn onboarding_fresh_start_seeds_database() { /* ... */ }

#[tokio::test]
async fn fix_creates_correction_pair() { /* ... */ }

#[tokio::test]
async fn undo_voids_last_posted_transaction() { /* ... */ }
```

- [ ] **Step 4: Run.**

```bash
cd /Users/robert/Projects/tally.ai/apps/desktop/src-tauri && cargo test --test orchestrator_integration
```

- [ ] **Step 5: Commit.**

```bash
git add apps/desktop/src-tauri/tests
git commit -m "test(t-062): Rust orchestrator integration suite — 4 flows"
```

---

### Task 27: CI — Playwright job + Rust integration step

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Read current workflow.**

Use `Read` on `.github/workflows/ci.yml`.

- [ ] **Step 2: Add Playwright job (Linux only).**

Append:

```yaml
e2e:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v6
    - uses: pnpm/action-setup@v3
      with: { version: 9 }
    - uses: actions/setup-node@v4
      with: { node-version: 20, cache: pnpm }
    - run: pnpm install --frozen-lockfile
    - run: pnpm exec playwright install --with-deps chromium
      working-directory: apps/desktop
    - run: pnpm test:e2e
      working-directory: apps/desktop
    - if: failure()
      uses: actions/upload-artifact@v4
      with:
        name: playwright-report
        path: apps/desktop/playwright-report
```

- [ ] **Step 3: Add `cargo test --test orchestrator_integration`** to the existing Rust job (after the existing `cargo test` step).

- [ ] **Step 4: Push branch and watch CI.**

```bash
git add .github/workflows/ci.yml
git commit -m "ci(t-062): add Playwright job + Rust orchestrator integration step"
git push -u origin test/t-062-e2e
```

- [ ] **Step 5: Iterate on CI failures.** Typical issues: Playwright browser install, port conflicts, mock-invoke shape drift on the test build.

---

### Task 28: PR 3 — open + merge

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Add Implementation status entry.**

```markdown
**E2E coverage (T-062):**
- Playwright suite at `apps/desktop/e2e/` covers the four critical flows:
  onboarding fresh-start, entry happy path, `/fix`, `/undo`.
- Mock `invoke` is injected only when `import.meta.env.MODE === "test"`;
  fixtures live in `apps/desktop/e2e/fixtures/responses.ts`.
- A contract test asserts the mock has a responder for every
  `#[tauri::command]`. New Rust commands fail the contract test until
  a mock fixture is added.
- Rust orchestrator integration (`apps/desktop/src-tauri/tests/orchestrator_integration.rs`)
  exercises validator → committer against a real SQLCipher temp DB with
  a `MockClaudeAdapter`.
- CI: Playwright runs headless on Linux; integration runs via
  `cargo test --test orchestrator_integration`.
```

- [ ] **Step 2: Commit and open PR.**

```bash
git add CLAUDE.md
git commit -m "docs: record T-062 in implementation status"
git push
gh pr create --title "test(e2e): T-062 — Playwright + Rust orchestrator integration" --body "$(cat <<'EOF'
## Summary

- Playwright suite covering the four critical flows: onboarding fresh-start,
  entry happy path, /fix, /undo.
- Mock invoke injected via import.meta.env.MODE === "test"; mock surface
  pinned by a contract test that enumerates every #[tauri::command] and
  asserts the mock has a matching responder.
- Rust orchestrator integration suite hitting a real SQLCipher temp DB
  through a MockClaudeAdapter.
- New Playwright headless CI job; integration step added to the existing
  Rust CI job.

Spec: docs/superpowers/specs/2026-04-26-p2-testing-and-polish-design.md
Plan: docs/superpowers/plans/2026-04-26-p2-testing-and-polish.md

## Test plan

- [x] pnpm test:e2e — 5 specs (4 flows + contract) green locally
- [x] cargo test --test orchestrator_integration — 4 flows green
- [x] CI green: Rust unit + integration + TS unit + Playwright headless
EOF
)"
```

---

# Self-review

After all three PRs merge, confirm against spec:

- **§Goal item 1** (validation tier + card-state behavior tests) → PR 1 (Rust) + PR 2 (React).
- **§Goal item 2** (canonical error → RecoveryAction surface) → PR 2 (`safeInvoke`).
- **§Goal item 3** (WCAG 2.1 AA leaf pass) → PR 2 (T-063 + axe wiring in T-061).
- **§Goal item 4** (E2E coverage of the four flows) → PR 3.
- **§Goal item 5** (CLAUDE.md keep-current discipline) → PR 1 (T-065).

- **§Risk: mock-invoke drift** → mitigated by Task 21 contract test.
- **§Risk: axe-core noise** → mitigated by Task 15 `RULE_OVERRIDES` + audit doc reference.
- **§Risk: safeInvoke migration blast** → mitigated by Task 13's batch sequencing + Task 14's eslint guard.
- **§Risk: `__TAURI_INTERNALS__` injection breakage** → mitigated by Task 21's single-file injection point.

- **§Success criterion PR 1** → Tasks 4–6 produce ~36 matrix tests; Task 7 verifies green + ≥80% coverage.
- **§Success criterion PR 2** → Task 16 produces `MATRIX.md`; Task 15 wires axe; Task 17 produces audit doc; Task 19 verifies suite green.
- **§Success criterion PR 3** → Task 27 adds CI job; Tasks 22–26 produce four Playwright + four Rust integration tests; Task 21's contract test is green.

If any task references a function, file, or flag that does not exist after implementation, the plan is wrong, not the code — update the plan and re-verify.
