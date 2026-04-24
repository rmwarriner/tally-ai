# Architectural Decisions

This document tracks significant decisions made during Tally.ai development. Use this as a reference when considering future changes.

## Format

Each decision follows this format:

```
### [Date] — [Title]

**Decision**: What was decided.

**Rationale**: Why this choice was made.

**Status**: Accepted | Superseded | Deprecated

**Consequences**: What this enables or constrains.
```

---

## Active Decisions

### 2026-04-24 — Persist GnuCash GUID on imported accounts

**Decision**: Imported accounts carry their GnuCash GUID in a new `accounts.gnc_guid` column (migration 0007). The reconciler queries Tally rows by this GUID rather than matching on account name.

**Rationale**: Leaf-name matching was unsound — a GnuCash book legitimately can have two accounts with the same leaf name under different parents (e.g. `Assets:Cash:Savings` vs `Investments:Savings`). GUID is canonical and index-friendly.

**Status**: Accepted

**Consequences**:
- Reconcile performance is O(n log n) via the `idx_accounts_gnc_guid` partial index.
- Manual account creation (non-imported) leaves `gnc_guid = NULL`; the partial index keeps the table scan shape unchanged for those rows.
- If a future ticket adds "re-import" semantics, it has a clean key to match on.

---

### 2026-04-17 — Money as Integer Cents

**Decision**: All monetary amounts are stored as INTEGER cents in the database, never FLOAT or REAL.

**Rationale**: Floating-point arithmetic causes precision errors that compound in financial applications. Using integer cents (e.g., 12500 for $125.00) eliminates these issues entirely.

**Status**: Accepted (non-negotiable)

**Consequences**:
- All currency display must divide by 100
- All currency inputs must multiply by 100
- Impossible to represent sub-cent amounts (acceptable for USD, EUR, etc.)
- Calculations are always precise

---

### 2026-04-17 — AI Proposes, Rust Validates

**Decision**: The Claude AI layer submits `TransactionProposal` objects, and the Rust core validates before committing.

**Rationale**: This boundary prevents invalid data from reaching the database. The Rust compiler enforces the separation via type system. AI generates proposals, humans (or future rules) validate.

**Status**: Accepted (architectural boundary)

**Consequences**:
- AI can never directly mutate the database
- All business logic validation lives in Rust
- Proposals must round-trip through serialization
- Audit trail captures AI proposals and validation results separately

---

### 2026-04-17 — Audit Log is INSERT-only

**Decision**: The `audit_log` table permits only INSERT operations. No UPDATE or DELETE.

**Rationale**: An immutable audit trail is the single source of truth for what happened and when. Allowing mutations would undermine trust and compliance.

**Status**: Accepted (non-negotiable)

**Consequences**:
- Corrections are recorded as new entries, not overwrites
- Audit log grows monotonically (never shrinks)
- Storage costs increase over time
- Corrections are fully traceable

---

### 2026-04-17 — Tauri + React + Claude API (Phase 1)

**Decision**: Phase 1 uses Tauri (Rust), React (TypeScript), and Claude API only. No mobile, no other AI backends, no multi-user.

**Rationale**: MVP scope: prove the concept on desktop as a single-user, local-first app. Other backends and platforms can be added in Phase 2 once the core loop works.

**Status**: Accepted (Phase 1 constraint)

**Consequences**:
- No sync between devices
- No cloud backup (user's machine is the source of truth)
- No multi-user collaboration
- Strong desktop UX possible, future expansion clear

---

### 2026-04-17 — SQLCipher Encryption

**Decision**: SQLite database is encrypted with SQLCipher; encryption keys are derived from user passphrases via Argon2id.

**Rationale**: Sensitive financial data at rest must be encrypted. Argon2id is memory-hard, resisting brute-force attacks.

**Status**: Accepted

**Consequences**:
- Passphrase-protected access; lost passphrase = lost data
- No keychain integration (user holds the key)
- Encryption/decryption overhead (acceptable for desktop)
- No recovery mechanism for forgotten passphrases

---

### 2026-04-17 — Dates in UTC Milliseconds

**Decision**: All dates are stored as unix milliseconds (UTC) at midnight of the local date. Timezone conversion uses `household.timezone` (IANA identifier).

**Rationale**: Unix timestamps are unambiguous and portable. Storing at midnight allows date boundaries to be computed correctly even when the app crosses timezones. IANA identifiers handle DST changes.

**Status**: Accepted

**Consequences**:
- All date queries must account for timezone
- Dates are precise to milliseconds (overkill for daily finance, but harmless)
- Query complexity increases slightly (midnight boundary math)

---

### 2026-04-23 — Claude API Key in OS Keychain

**Decision**: The Claude API key is stored in the operating system's keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service) via the `keyring` crate. A `CLAUDE_API_KEY` env var is accepted as a development fallback. The SQLCipher-protected database is **not** used for this secret.

**Rationale**: Keychain access is gated by the OS user session, which is the right trust boundary for a secret the app needs to read on every chat turn. Keeping it out of the DB means the key is unaffected by passphrase rotation, DB corruption, or export/backup flows. This does not contradict the 2026-04-17 SQLCipher decision — that decision is about the DB passphrase (which the user holds), not other long-lived credentials.

**Status**: Accepted

**Consequences**:
- Adds `keyring` crate dependency
- Onboarding gains an "enter API key" step; the key is prompted once and persisted
- Losing keychain access (new machine, OS reinstall) requires re-entering the key, not restoring from backup
- Dev loop can use `CLAUDE_API_KEY` without touching the keychain

---

### 2026-04-23 — GnuCash Import via SQLite Backend Only

**Decision**: The Phase 1 GnuCash importer reads GnuCash files saved with the SQLite backend. The XML backend and CSV exports are not supported in Phase 1.

**Rationale**: GnuCash's SQLite schema is stable, documented, and trivially readable via `sqlx`. XML would require a dedicated parser and handling of gzip framing; CSV is lossy (no splits, no GUIDs, no commodities). The user's existing book is on the SQLite backend, so this is sufficient for beta testing.

**Status**: Accepted (Phase 1 constraint)

**Consequences**:
- Beta users must save their GnuCash book with "File → Save As → SQLite" before importing
- Idempotency is anchored on GnuCash transaction GUIDs — re-runs are safe
- XML-backed books are a Phase 2 concern if a beta user requests it

---

### 2026-04-24 — GnuCash Mapping Card as Top-Level Message Kind

**Decision**: The GnuCash CoA mapping preview is rendered as a new top-level chat message `kind: "gnucash_mapping"` dispatched from `MessageList.tsx`, not as an `artifact` variant via `ArtifactCard.tsx`.

**Rationale**: The existing `artifact` message kind's payload is `{ artifact_id, title, content?: string }` with `content` as plain text — it's not a discriminated-union-with-typed-payloads dispatcher. Existing rich renderers (`LedgerTable`, `BalanceReport`) sit outside that path unwired. Converting the `artifact` kind into a typed-payload union and retro-fitting the existing renderers was out of scope for T-072; the one-component-per-rich-kind pattern (already used by `TransactionCard`, `HandoffMessage`, `SetupCard`) is the more honest fit.

**Status**: Accepted

**Consequences**:
- Each rich chat artifact currently lives under its own message kind — `gnucash_mapping` joins `transaction_card`, `handoff`, `setup_card`.
- If a future ticket consolidates rich artifact rendering under a typed-payload `artifact` dispatcher, `GnuCashMappingCard`, `GnuCashReconcileCard` (T-074), `LedgerTable`, and `BalanceReport` would migrate together — a deliberate refactor, not an incremental change.
- The plan's sketched `reply.messages.some(m => m.kind === "artifact" && m.artifact === "gnucash_mapping")` test pattern was replaced with `addGnuCashMappingMessage` store-action assertions, consistent with how other onboarding-side-effect handlers are tested.

---

## Superseded Decisions

(None yet — first major decisions just made.)

---

## Deprecated Decisions

(None yet.)

---

## How to Add a Decision

When a significant architectural decision is made:

1. **Note the date** (YYYY-MM-DD)
2. **Write a clear decision statement** (what, not why)
3. **Explain the rationale** (constraints, tradeoffs)
4. **Mark the status** (Accepted, under discussion, etc.)
5. **List consequences** (what this enables or constrains)
6. **Get consensus** — Link to relevant PR or discussion

Push to a branch and include in the PR that implements the decision.
