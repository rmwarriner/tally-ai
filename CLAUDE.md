# Tally.ai — CLAUDE.md

## Project identity

Tally.ai is a conversational household finance app built with Tauri 2 (Rust backend),
React/TypeScript frontend, and Codex AI. The user interacts exclusively through
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

- Codex API: always use tool use for TransactionProposal output.

- Never parse free-form text to extract transaction data.

- Prompt assembly order: BASE > SNAPSHOT > INTENT > HISTORY > MEMORY.

- BASE and SNAPSHOT are never trimmed. Others trim under token budget.

- Memory writes are always async — never block the response path.

## Phase 1 scope

- Desktop only (Tauri). No mobile, no sync, no multi-user.

- Codex backend only. No GPT, Gemini, or Ollama yet.

- Manual entry only. No SimpleFIN, no file import, no folder watch.

- No scheduled/recurring transactions yet.

- Stub Phase 2 extension points with clear TODO(phase2) comments.

## Implementation status (as of 2026-04-26)

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

## Phase 2 stubs (TODO(phase2) in code)

- Full hledger CoA mapping (`import_hledger` command).
- GAAP undo via `core::correction` (`undo_last_transaction` command).
- Persistent AI defaults table (`get_ai_defaults` command).
- Proper CSPRNG for salt generation (currently `DefaultHasher` + time + pid).
- Pre-existing `audit_log` write gap — no production code populates it yet
  (flagged during T-072 review; out of Phase 1 scope).
