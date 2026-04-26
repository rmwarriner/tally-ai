# P2 Section 9.7 — Testing & Polish — Design

**Date:** 2026-04-26
**Tickets:** T-060, T-061, T-062, T-063, T-064, T-065
**Spec source:** `docs/tally_phase1_spec.md` Section 9.7

## Goal

Phase 1 ships with:

1. Every validation tier and transaction-card state covered by explicit
   behavior tests.
2. One canonical surface for translating Rust errors into user-facing
   `RecoveryAction[]`.
3. A WCAG 2.1 AA pass on leaf-level accessibility issues.
4. End-to-end smoke coverage of four critical flows: onboarding fresh-start,
   transaction entry happy path, `/fix`, `/undo`.
5. A discipline that keeps `CLAUDE.md` current as tickets land.

## Non-goals

- No coverage-percent target above the existing 80% pre-commit floor.
- No structural a11y rework (custom widget → native element refactors,
  streaming-token announcement). Those become Phase 2 tickets.
- No E2E coverage of paths outside the four flows above (per-account
  ledger queries, `/budget` reports, GnuCash import — already covered by
  unit/integration tests in their own PRs).
- No retroactive issue creation for T-045–T-049 / T-071–T-074. They were
  tracked via PR; that history stays.

## Architectural choices

### 1. E2E approach: hybrid (browser-mocked frontend + Rust integration tests)

- Playwright drives the React app in a normal browser via Vite dev server;
  a mock `invoke` is registered on `window.__TAURI_INTERNALS__` during test
  bootstrap. Mock returns canned fixtures.
- Rust integration tests exercise orchestrator → validator → committer
  against a real (temp-file) SQLCipher DB with a `MockClaudeAdapter`.
- Each layer covers what it is good at. We do not run `tauri-driver`.

**Rationale:** The four target flows are UI flows. `tauri-driver` would
make CI fragile (macOS support is uneven, build is slow) without buying
meaningful coverage of the IPC layer that is not already covered by
`#[tauri::command]` unit tests + the orchestrator integration suite.

### 2. Error → RecoveryAction surface: centralized `safeInvoke` wrapper

- New `apps/desktop/src/lib/safeInvoke.ts` wraps `invoke<T>(cmd, args)`.
- Catches Rust errors; normalizes to `RecoveryError = { message: string,
  recovery: NonEmpty<RecoveryAction> }`.
- Two call shapes:
  - `safeInvoke<T>(cmd, args)` returns `Result<T, RecoveryError>` for
    inline handling (used by card-local flows like `commit_proposal`).
  - `safeInvokeOrAdvise<T>(cmd, args)` emits a system advisory chat
    message via the chat store on error and returns `T | null`.
- Rust side: every `#[tauri::command]` returns `Result<T, RecoveryError>`.
  A top-level `catch_unwind` shim translates panics into a generic
  `RecoveryError { recovery: [ShowHelp, Discard] }`.
- `core-types` gets a serializable `RecoveryError` type so the wire shape
  is enforced at compile time on both sides.
- A thin React `<ErrorBoundary>` wraps `<App>` and catches *render-time*
  crashes only. It renders a system message with `[ShowHelp]`. It is the
  safety net, not the primary error path.

**Rationale:** The CLAUDE.md non-negotiable rule "every error carries
NonEmpty<RecoveryAction>" needs *one* place that does the translation.
Per-call-site handling (option B) duplicates work and misses sites; a
React ErrorBoundary alone (option A) does not catch most failures
because they are awaited promise rejections, not render errors.

### 3. A11y remediation: audit + leaf fixes; defer structural

- Audit captured in `docs/superpowers/a11y-2026-04.md`.
- Fix in this PR: `aria-label` on icon-only buttons, focus-visible rings,
  contrast token sweep, slash-palette keyboard parity verification, tab
  order on onboarding cards, reduced-motion respect on auto-scroll.
- Defer (each gets a Phase 2 ticket): custom widgets that should be
  native elements, screen-reader announcement for streaming chat tokens,
  full keyboard map for power users.
- axe-core integration in T-061 (`expect(await axe(...))`) catches
  regressions on every component test run.

### 4. "Full coverage" interpretation: behavior matrix, not coverage %

- Spec phrasing — "full coverage of validation tiers" and "transaction
  card states, chat thread" — points at *behaviors*, not lines.
- Pre-commit 80% gate stays as a floor against accidental drops.
- Reviewer enforces matrix completeness; if the matrix forces coverage
  above 80% naturally, fine.

## Per-ticket detail

### T-060 — Rust validation behavior matrix

- New file `src-tauri/src/core/validate/tests/matrix.rs`.
- Tier 1 (`HardError`): all 6 rules × {pass, fail, edge} × asserts the
  recovery action set against the spec.
- Tier 2 (`SoftWarning`): all 5 rules × same shape.
- Tier 3 (`AIAdvisory`): all 4 advisory types × triggering snapshot
  fixture.
- Existing scattered tests stay; `matrix.rs` is the canonical inventory.

### T-061 — React component behavior matrix

- New file `apps/desktop/src/__tests__/MATRIX.md` listing every
  component requirement.
- Tests live alongside components (existing convention).
- Targets:
  - `TransactionCard` — 4 states × {render, info-circle, journal-line
    drawer, card-local error}.
  - `ChatThread` — message rendering by kind, date separators,
    auto-scroll, new-message pill, infinite history.
  - `InputBar` — slash palette filter + keyboard nav, chip strip,
    auto-grow.
  - `safeInvoke` — error normalization, advisory emission, opt-out path.
  - `ErrorBoundary` — render-crash → system message.
- Every component test runs `expect(await axe(...)).toHaveNoViolations()`.

### T-062 — Playwright flows + Rust orchestrator integration

- Playwright suite `apps/desktop/e2e/`. Four flows:
  - Onboarding fresh-start (household → accounts → envelopes → handoff).
  - Entry happy path (chat message → pending card → confirm → posted).
  - `/fix` flow on a posted transaction.
  - `/undo` last entry.
- Mock `invoke` injected via `import.meta.env.MODE === 'test'` branch
  in a single bootstrap file. Mock handlers are typed against the
  command signatures in `core-types`; a contract test enumerates every
  `#[tauri::command]` and asserts the mock has a matching handler with
  the same arg/return shape. New Rust commands fail the contract test
  until a mock is added.
- Rust integration `src-tauri/tests/orchestrator_integration.rs`. Real
  temp-file SQLCipher DB. `MockClaudeAdapter` returns fixture
  proposals. Same four flows.
- CI: Playwright headless on Linux job; Rust integration as `cargo test
  --test orchestrator_integration` on every push.

### T-063 — A11y audit + leaf fixes

- Audit doc `docs/superpowers/a11y-2026-04.md` enumerates findings with
  status (fixed in this PR / deferred to ticket #).
- `axe-config.ts` codifies the rule set; every disabled rule cites the
  audit doc.
- Phase 2 follow-up tickets filed for structural items before this PR
  merges.

### T-064 — `safeInvoke` wrapper + ErrorBoundary

- See architectural choice 2 above.
- Migration is opt-in command-by-command. The old shape is removed in
  the same PR once the last call site is migrated, so we never carry
  a half-migrated state past PR boundary.

### T-065 — Keep-current discipline

- Add to CLAUDE.md "Code conventions": *"Update the implementation
  status section in this file as part of any feat: PR that lands ticket
  work."*
- Add a corresponding paragraph to `CONTRIBUTING.md`.
- No code; lives or dies on review.
- Folded into PR 1.

## Sequencing

Three PRs, in order:

1. **`feat(core): T-060 Rust validation behavior matrix`** — pure Rust,
   independent of UI. Includes the T-065 doc additions.
2. **`feat(ui): T-064 + T-061 + T-063`** — one branch; T-064 ships
   first inside the branch, T-061 adds tests that exercise it, T-063
   fixes a11y leaves and wires axe-core into the test suite.
3. **`test(e2e): T-062 — Playwright + Rust orchestrator integration`**
   — last because the flows assume the wrapper and the matrix are in
   place. Adds the Playwright CI job.

Each PR's pre-commit hook runs the full Rust + TS + 80% coverage gate.

## Risks and mitigations

| Risk | Mitigation |
|------|-----------|
| Mock `invoke` drift from real Rust signatures | Generate mock from `core-types`; contract test pins shape parity per command |
| axe-core noise on low-priority rules | Codify enforced rule set in `axe-config.ts`; disabling any rule must cite the audit doc |
| `safeInvoke` migration blast radius | Migrate command-by-command via compile-time errors; remove old shape only after last site migrated, in same PR |
| `window.__TAURI_INTERNALS__` injection breaks on Tauri upgrade | Hide injection behind one `import.meta.env.MODE === 'test'` bootstrap so breakage is one fix |

## Success criteria

- **PR 1:** matrix file lists every Tier 1/2/3 rule with +/- tests
  asserting recovery action set; `cargo test` passes; coverage ≥ 80%.
- **PR 2:** `MATRIX.md` lists every component requirement; `pnpm test`
  passes; every component test wrapped with axe assertion; audit doc
  enumerates findings with status; coverage ≥ 80%.
- **PR 3:** four Playwright flows green headless on Linux CI; four
  Rust integration tests green; mock-invoke contract test passes.
- **All:** zero regressions; pre-commit hook passes on every commit
  (no `--no-verify`).
