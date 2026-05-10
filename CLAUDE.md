# Tally.ai — CLAUDE.md

## Project identity

Tally.ai is a conversational household finance app built with Tauri 2 (Rust backend),
React/TypeScript frontend, and Claude AI. The user interacts exclusively through
a chat interface. There are no forms and no edit screens — all writes go through chat.

## Non-negotiable architectural rules

- Money is ALWAYS stored as INTEGER cents. Never REAL or FLOAT for amounts.

- The AI layer NEVER writes to the database directly. It submits proposals.
  The Rust core validates and commits. This boundary must never be crossed.

- audit_log is INSERT-only. Never issue UPDATE or DELETE on audit_log.

- journal_lines.amount is always positive. The side field (debit|credit)
  encodes direction. Never use negative amounts.

- Every hard error, warning, and advisory must carry NonEmpty<RecoveryAction>.
  Zero-action errors are a compile error by design.

- Error messages shown to the user must be plain language. No error codes,
  no runtime text, no field names. Internal codes go to logs only.

- Interactive UI elements must always have a visible affordance (info circle).
  No invisible clickables anywhere in the app.

## Code conventions

- TDD-first: write tests before implementation. 80% coverage enforced pre-commit.

- Rust: use thiserror for error types. No unwrap() in production paths.

- TypeScript: strict mode. No any. Use core-types package for shared types.

- React: functional components only. No class components.

- State: Zustand for UI state. TanStack Query for server/DB state.

- Feature branches: never commit directly to main.

- Commit messages: conventional commits format (feat:, fix:, test:, docs:).

- Update the "Implementation status" section in this file as part of any
  feat: PR that lands ticket work. See CONTRIBUTING.md for detail.

## Key types (Rust)

- TransactionProposal: what the AI returns for entry intents
- ValidationResult: what the Rust core returns after validation
- RecoveryAction: typed next-step for every error (CreateMissing, UseSuggested,
  EditField, PostAnyway, Discard, ShowHelp)
- HardError / SoftWarning / AIAdvisory: three-tier validation results

## Database rules

- All dates stored as unix milliseconds UTC midnight of local date.
  Use household.timezone (IANA) for all local date conversions.

- ULID for all primary keys. Use ulid crate in Rust, ulid package in TS.

- SQLCipher encryption key derived from user passphrase via Argon2id.

- Migrations live in src-tauri/src/db/migrations/. Never edit past migrations.

## AI orchestration

- Claude API: always use tool use for TransactionProposal output.

- Never parse free-form text to extract transaction data.

- Prompt assembly order: BASE > SNAPSHOT > INTENT > HISTORY > MEMORY.

- BASE and SNAPSHOT are never trimmed. Others trim under token budget.

- Memory writes are always async — never block the response path.

## Phase 1 scope

- Desktop only (Tauri). No mobile, no sync, no multi-user.

- Claude backend only. No GPT, Gemini, or Ollama yet.

- Manual entry only. No SimpleFIN, no file import, no folder watch.

- No scheduled/recurring transactions yet.

- Stub Phase 2 extension points with clear TODO(phase2) comments.

## Implementation status (as of 2026-05-10, Phase 2 in progress)

**Chat surface (T-033–T-039, T-044):**
- Chat thread: message rendering by type, date separators, auto-scroll, new-message
  pill, infinite history loading.
- Transaction cards: posted, pending, voided, correction pair (journal line drawer).
- Artifact cards: framed inline panel, Copy action, LedgerTable and BalanceReport
  renderers.
- Proactive advisory variant: amber avatar, caution accent, optional advisory code pill.
- InfoCircle/Tooltip primitives in `src/components/ui/` for non-obvious affordances.
- Input bar: auto-growing textarea, context chip strip, send button, slash command
  palette with keyboard nav.
- Slash command routing (`useSlashDispatch`): `/budget`, `/balance`, `/recent`,
  `/fix` go through send-message; `/undo`, `/help`, `/defaults`, unknown handled
  locally via system/artifact insertion.
- Handoff message: summary card with account/envelope counts and starter prompts.

**Onboarding (T-040–T-044):**
- Adaptive phase detection in `buildOnboardingHandler(deps)` factory.
- Fresh-start path: household name, timezone, passphrase, accounts + opening
  balances, envelopes.
- Migration path: hledger import + CoA mapping session (stub; full mapper is
  Phase 2).
- Setup cards: `household_created`, `account_created`, `opening_balance`,
  `envelope_created` variants.

**Live chat loop (T-045–T-047, T-049):**
- `chat_messages` table, `ChatRepo`, `useChatPersistence` (hydrate on return,
  persist after onboarding completes).
- `submit_message` Tauri command backed by `ai::orchestrator`: classify →
  snapshot → Claude for entry intents; snapshot-only for `QueryBalance`;
  placeholder for other intents.
- `commit_proposal` Tauri command + Confirm/Discard on `TransactionCardPending`;
  validation rejections shown as card-local error.
- Claude API key in OS keychain via `keyring` crate; `CLAUDE_API_KEY` env var
  wins for dev. New `api_key` onboarding step.

**Live sidebar reads (T-048):**
- `core::read` module owns balance math (single source of truth).
  `ai::snapshot` delegates to it.
- Three Tauri commands back the sidebar: account balances, current envelope
  periods, coming-up transactions.
- `create_envelope` seeds a current-month `envelope_periods` row via
  `current_month_bounds_ms(tz, now_ms)` (chrono-tz).
- `useInvalidateSidebar` hook fires after every commit success and onboarding
  DB write so the sidebar refreshes without waiting for staleTime.
- Snapshot exposes every account ULID via `to_prompt_text_with_ids` (zero-balance
  included) so Claude returns valid account IDs.

**GnuCash SQLite import (T-071–T-074):**
- Reader, CoA mapper, atomic committer, post-import reconciler (onboarding-only).
- Idempotent on GnuCash transaction GUID via `transactions.source_ref`.
- Imported accounts stamped with `accounts.gnc_guid`; reconciler matches by GUID.
- New top-level message kinds `gnucash_mapping`, `gnucash_reconcile`
  (see DECISIONS.md 2026-04-24).
- Migrations 0006, 0007.

**Validation behavior matrix (T-060):**
- `core::validation_matrix` is the canonical inventory of validation behaviors.
  Tier 1 (8 HardError variants), Tier 2 (5 SoftWarning variants), Tier 3
  (4 AIAdvisory builders). Every variant has +/- tests asserting the recovery
  action set against actual code. New rules MUST add a row to this matrix.
- Two follow-ups filed: `EnvelopeMismatch` unimplemented (#113);
  `PossibleDuplicate` rule scoping (#114).

**safeInvoke + ErrorBoundary (T-064):**
- `apps/desktop/src/lib/safeInvoke.ts` is the single surface translating
  Tauri command errors into `RecoveryError = { message, recovery: NonEmpty<RecoveryAction> }`.
  Two shapes: `safeInvoke` returns `Result<T, RecoveryError>` for inline
  handling; `safeInvokeOrAdvise` emits a proactive advisory chat message
  via `useChatStore.appendAdvisory` and returns `T | null`.
- `RecoveryError` Rust type in `core::error`; mirror in `core-types`.
  Every `#[tauri::command]` returns `Result<T, RecoveryError>` (24 commands
  migrated). `chatStore.appendAdvisory(err)` reuses the proactive variant.
- ESLint flat config bans direct `invoke` imports from `@tauri-apps/api/core`
  (`@typescript-eslint/no-restricted-imports` with `allowTypeImports: true`).
  Pre-commit hook runs lint, typecheck, tests, and 80% coverage gate.
- Render-time `<ErrorBoundary>` wraps `<App>` in `main.tsx` with `role="alert"`
  + "Get help" reload button; logs to console.
- `QueryClientProvider` lives in `main.tsx` above `<ErrorBoundary>`'s
  children — App's body hooks (`useOnboardingEngine` → `useInvalidateSidebar`)
  consume the provider context.

**Component behavior matrix + a11y framework (T-061, T-063):**
- `apps/desktop/src/__tests__/MATRIX.md` is the canonical inventory of
  React component requirements. Covers `TransactionCard` (4 states),
  `ChatThread` (10 message kinds, separators, scroll, infinite history),
  `InputBar` (slash palette, chip strip, auto-grow). New components MUST
  add a row.
- `apps/desktop/src/test/axe.ts` exposes `checkA11y` + `expectNoA11yViolations`.
  Every matrix-listed surface has at least one axe-wrapped render.
- Live a11y audit deferred to beta self-testing; framework ready in
  `docs/superpowers/a11y-2026-04.md`. Three structural items filed as
  Phase 2 issues: native widgets (#126), streaming live region (#127),
  full keyboard map (#128).
- Doc-discipline rule (T-065) added: every `feat:` PR landing ticket work
  updates this Implementation status section. See CONTRIBUTING.md.

**AI optimization (T-066–T-070):**
- T-070 (tool definition): `proposal_tool()` JSON trimmed of restating-the-property-name descriptions; the test
  `tool_definition_under_token_budget` fails CI if the serialized JSON exceeds 1400 chars (≈ 350 tokens via
  the same `chars/4` rule used in `ai::prompt::approx_tokens`). Current: ~625 chars (~156 tokens).
- T-066 (prompt caching): system prompt is sent as a content-block array with `cache_control: { type: "ephemeral" }`
  on the BASE+SNAPSHOT chunk and on the tool definition. Anthropic returns ~10% billing for cached input on
  repeat calls within the 5-minute TTL. Cache hits surface via `usage.cache_read_input_tokens > 0`.
- T-069 (token tracking): migration 0010 adds `transactions.ai_input_tokens / ai_output_tokens / ai_cache_hit`.
  `AiAdapter::propose` now returns `ProposeResult { proposal, usage: AiUsage }`. The orchestrator embeds the
  usage in `MessageResponse::Proposal.ai_usage`; the frontend echoes it back to `commit_proposal`, which
  calls `commands::stamp_ai_usage` to write the columns. Best-effort: a stamp failure does not roll back the
  ledger commit.
- T-067 (history compression): `PROMPT_HISTORY_LIMIT` dropped from 20 → 10. Older messages compress to a
  deterministic one-line-per-pair summary (`compress_history_pairs`) which is prepended to the prompt
  (`[Earlier conversation summary: ...]`) and persisted via `store_summary_async` for future reads. No second
  Claude call — the compressor is local and free.
- T-068 (intent-scoped loading): `SnapshotScope::{Full, QueryBalance}` decides whether to load envelope rows.
  Record intents get the full snapshot; everything else skips envelopes. Payee memory hints are only loaded
  for Record intents (which is also the only path that uses them).

**Proactive engine (T-050–T-054):**
- `core::insight` (T-053) is the gate + dedup layer for every proactive
  message. `InsightKind`, `Sensitivity`, `should_emit`, `log_if_new`. Migration
  0008 adds `insight_log` with two partial unique indexes — one for
  entity-scoped kinds (envelope/duplicate), one for singletons (briefing).
  `day_bucket_ms` uses household-local midnight via chrono-tz so dedup
  respects the user's calendar day.
- Sensitivity (T-054): migration 0009 adds `households.sensitivity` (default
  `normal`) and `households.last_briefing_at_day`. Tauri commands
  `get_sensitivity` / `set_sensitivity`. `/sensitivity quiet|normal|proactive`
  slash command. Quiet blocks all kinds, normal allows alerts only,
  proactive allows everything (including morning briefing).
- Producers in `core::triggers`: `envelope::evaluate` (T-051) emits
  `EnvelopeOver` (>budget) or `EnvelopeApproaching` (≥85%); `duplicate::check`
  (T-052) emits `PossibleDuplicate` for same-household, same-day, same-memo,
  same-amount matches; `briefing::assemble` (T-050) builds a 4-item
  morning summary (cash on hand, top envelope, over-count, recent activity).
- Wiring: `commit_proposal` runs envelope triggers post-commit and returns
  `proactive_insights[]` in the `Committed` outcome. `ai::orchestrator`
  runs the duplicate trigger pre-commit and attaches insights to the
  `Proposal` response. New `session_open` Tauri command returns the
  briefing (or `null`) and updates `last_briefing_at_day`.
- Frontend: `useSessionOpen` hook fires once per mount after onboarding
  completes. `chatStore.appendInsight` writes a proactive message; reuses
  the backend ULID so within-session duplicates are no-ops.
  `ProactiveMessage` gains a `category` prop (`alert` | `insight` |
  `briefing`) with corresponding aria-label and left-border accent
  matching spec §6.2.
- Test surfaces: 12 unit tests in `core::insight`, 7 in `triggers::envelope`,
  7 in `triggers::duplicate`, 2 in `triggers::briefing`. Vitest covers
  `useSessionOpen`, `/sensitivity` dispatch, `ProactiveMessage` categories.
  Playwright spec `proactive.spec.ts` covers briefing render, envelope-over
  alert from commit, and `/sensitivity` flow.

**Playwright E2E + /undo wiring (T-062):**
- `apps/desktop/e2e/` holds the Playwright suite, driven by a mocked
  `window.__TAURI_INTERNALS__.invoke` injected via `addInitScript` in
  `e2e/setup.ts`. Specs cover fresh-start onboarding, entry (text +
  proposal Confirm/Discard + validation rejection), `/fix` palette and
  dispatch, and `/undo` happy + no-history paths. Migration / hledger /
  GnuCash import flows are out of scope for T-062 (GnuCash already covered
  by `src-tauri/tests/gnucash_import_integration.rs`).
- New rule of thumb: use `.last()` on text matchers — React.StrictMode
  double-fires effects in dev and many prompts repeat across the flow.
- Drift guard: `src-tauri/tests/orchestrator_e2e_contract.rs` exercises
  `submit_message → commit_proposal → undo_last_transaction` against a
  real encrypted SQLite + a `FixedProposalAdapter`, asserting the JSON
  shapes the E2E mocks return still round-trip through the real backend.
- `/undo` is no longer a stub. `commands::undo_last_transaction` now calls
  `core::correction::undo_last_transaction` and maps `CorrectionError`
  to `RecoveryError`. The frontend dispatcher (`useSlashDispatch`) also
  invalidates the sidebar after a successful undo.
- New CI job `e2e` runs Playwright on chromium against the Vite dev server
  in parallel with `test`, `typecheck`, and `rust-test`. Browser binaries
  are cached by `apps/desktop/package.json` hash.
- E2E coverage rules live in `apps/desktop/e2e/MATRIX.md` — same discipline
  as the React component MATRIX.

**Phase 2 Tier 1 — security hardening (#142, #143):**
- CSPRNG (#142): `commands::create_household` now uses
  `crate::crypto::generate_salt` (rand crate's thread RNG, CSPRNG-quality)
  instead of the prior `DefaultHasher`-of-(time,pid) construction. Fix is a
  4-line redirect in `commands/mod.rs` — the proper helper was already
  exported from `crypto::key_derivation` but never called by anything.
- audit_log writes (#143): new `core::audit` module exposes a single
  `write(tx, household_id, table_name, row_id, action, payload, now_ms)`
  helper that runs inside the caller's open SQL transaction. Wired into
  every ledger mutation: `core::ledger::commit_proposal`,
  `core::ledger::create_opening_balance`, `core::correction::void_and_reverse`
  (one update + one insert), `core::correction::correct_transaction`
  (replacement insert), and `commands::set_opening_balance` (which also
  gained an explicit SQL transaction wrapper).
- Payload shape: full proposal JSON (#143 design call). Disk is cheap;
  joins to reconstruct payloads later are not.
- Atomicity contract: audit failure aborts the whole SQL transaction.
  No quiet compliance holes — an audit row that's missing for a real
  ledger mutation is a bug we want to find via test/CI, not via silent
  drop.
- Test coverage: `core::audit` has 3 unit tests (write+roundtrip, rollback
  with outer txn, all three actions). The orchestrator contract test
  (`orchestrator_e2e_contract.rs`) asserts both the commit-side audit row
  and the void+reversal pair from undo round-trip end-to-end.

## Phase 2 stubs (TODO(phase2) in code)

- Full hledger CoA mapping (`import_hledger` command). [#145]
- Persistent AI defaults table (`get_ai_defaults` command). [#144]
