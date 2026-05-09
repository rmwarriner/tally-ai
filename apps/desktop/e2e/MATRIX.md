# Playwright E2E Matrix — T-062

Mirrors the discipline of `apps/desktop/src/__tests__/MATRIX.md` and
`core::validation_matrix`: every flow covered by an end-to-end spec has a
row here, and any new spec must add one. Specs run against a Vite dev server
with a mocked `window.__TAURI_INTERNALS__.invoke` (see `e2e/setup.ts`); a
Rust contract test (`src-tauri/tests/orchestrator_e2e_contract.rs`) guards
against drift between the canned response shapes and the real backend.

## Onboarding — fresh-start (`e2e/onboarding.spec.ts`)

- Walks household_name → timezone → passphrase → confirm → 1 account +
  balance → no more accounts → 1 envelope → no more envelopes → API key →
  handoff. Asserts the expected Tauri commands (`create_household`,
  `create_account`, `set_opening_balance`, `create_envelope`, `set_api_key`)
  fire in order.
- Passphrase mismatch: a non-matching confirmation drops back to the
  passphrase step; re-entering matching values advances normally.
- Skip API key: typing `skip` at the api_key step still completes the flow
  but never invokes `set_api_key`.

Out of scope (deferred): migration / hledger import path; GnuCash import
flow (already covered by the Rust integration test
`src-tauri/tests/gnucash_import_integration.rs`).

## Entry — submit_message + commit_proposal (`e2e/entry.spec.ts`)

- Text response: `submit_message` returns `{ kind: "text", text }` →
  `AIMessage` renders in the thread.
- Proposal response: `submit_message` returns `{ kind: "proposal", … }` →
  pending `TransactionCard` with Confirm and Discard buttons + "Proposed"
  badge.
- Confirm: clicking Confirm calls `commit_proposal`; on `committed`, the
  card flips to posted (Confirm/Discard buttons disappear).
- Discard: clicking Discard removes the proposal card without calling
  `commit_proposal`; the user's input message remains visible.
- Validation rejection: `commit_proposal` returning `{ status: "rejected",
  validation }` keeps the card pending and renders the error in a
  `role="alert"` region.

## /fix command (`e2e/fix.spec.ts`)

- Palette opens on `/`; typing `/f` narrows to a single matching option
  (`/fix`).
- `/fix <text>` dispatches a `submit_message` with `text === "Fix: <text>"`;
  the response renders as the next AI message.

## /undo command (`e2e/undo.spec.ts`)

- Happy path: `undo_last_transaction` resolves with a reversal ULID; the
  "Last transaction undone." system message appears, and a follow-up
  `get_account_balances` invoke fires (sidebar invalidation).
- No-history path: `undo_last_transaction` rejects with the standard
  Discard-flavored RecoveryError; the dispatcher swallows and renders the
  "Nothing to undo, or the last transaction cannot be reversed." message.

## Proactive engine (`e2e/proactive.spec.ts`)

- Morning briefing: `session_open` returning a `morning_briefing` insight
  renders as a proactive bubble with the "Morning briefing" aria-label
  and blue accent.
- Envelope-over alert: `commit_proposal` returning `proactive_insights[]`
  with an `envelope_over` payload renders the alert as a proactive bubble
  with the "Proactive alert" aria-label and red accent.
- `/sensitivity quiet` dispatches `set_sensitivity` and confirms via system
  message; `/sensitivity` with no args reads the current value via
  `get_sensitivity`.
- Out of scope (deferred): a stress-test scenario asserting that a
  `quiet` sensitivity actually suppresses the briefing — covered at the
  Rust unit level (`core::insight::tests::should_emit_*`).
