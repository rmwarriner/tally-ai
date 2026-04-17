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
