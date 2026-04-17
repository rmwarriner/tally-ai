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
