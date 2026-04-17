# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in Tally.ai, please report it responsibly by emailing **rmwarriner@icloud.com** instead of using the public issue tracker.

In your report, please include:

- Description of the vulnerability
- Steps to reproduce (if applicable)
- Potential impact
- Suggested fix (if you have one)

We will acknowledge your report within 48 hours and aim to provide updates on remediation progress within one week.

## Security Principles

Tally.ai follows these security principles:

### Money as Integers

All monetary amounts are stored as **integer cents**, never as floats. This eliminates floating-point precision errors that could cause financial discrepancies.

### Encrypted at Rest

The SQLite database is encrypted with **SQLCipher**, using encryption keys derived from user passphrases via **Argon2id** (memory-hard KDF). Database files are unreadable without the correct passphrase.

### Audit Trail

All changes to transactions are logged immutably in the `audit_log` table. The audit log is INSERT-only—no UPDATE or DELETE operations allowed. This creates an authoritative ledger of all modifications.

### Validated Writes

The AI layer never writes directly to the database. Instead, it submits `TransactionProposal` objects. The Rust core validates these proposals against business rules before committing. This boundary is enforced at compile time.

### Plain Language Errors

Error messages shown to users are in plain English without error codes or field names. Internal error codes and stack traces are logged but never exposed to users.

## Known Limitations (Phase 1)

- **No multi-user support** — Each instance is single-user; no sync between devices
- **Manual entry only** — No automated transaction import or external integrations
- **Local desktop only** — No cloud backup; data lives on the user's machine
- **Claude API dependency** — Requires active Claude API key; API availability is a dependency

These limitations are intentional for Phase 1. Future phases will address them.

## Compliance

Tally.ai is designed for Phase 1 as a local-first, single-user finance app. It does not currently meet standards required for multi-user, cloud-hosted, or regulatory (GDPR, PCI-DSS) deployments. If you plan a Phase 2 that supports these, a security audit is recommended.

## Security Advisories

We use GitHub Security Advisories to track and communicate security issues. Check the [Security tab](https://github.com/yourusername/tally-ai/security/advisories) for published advisories.

---

Thank you for helping keep Tally.ai secure.
