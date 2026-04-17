# Contributing to Tally.ai

Thanks for your interest in contributing! Tally.ai is still in early phase, but we welcome contributions to improve the codebase.

## Getting Started

1. **Fork the repo** and clone your fork
2. **Install dependencies** — `pnpm install`
3. **Create a feature branch** — Never commit directly to `main`
4. **Read CLAUDE.md** — Understand our conventions and architectural rules

## Code Conventions

### Commits

Use [conventional commits](https://www.conventionalcommits.org/):

```
feat: add transaction categories
fix: correct decimal rounding in calculations
test: add tests for validation logic
docs: clarify database migration process
chore: update dependencies
```

### TypeScript

- **Strict mode** — No `any` types
- **No class components** — Use functional components
- **Zustand** for UI state, **TanStack Query** for server/DB state
- **Import from core-types** — Use the shared type package for common types

### Rust

- **Use `thiserror`** for error types
- **No `unwrap()`** in production paths
- **Clippy clean** — `cargo clippy --all -- -D warnings`
- **Tests first** — Write tests before implementation (TDD)

### Money & Precision

- **Always integer cents** — Never REAL or FLOAT for amounts
- **Side field encodes direction** — Use `debit` or `credit`, never negative amounts
- **Milliseconds UTC** — All dates stored as unix milliseconds at UTC midnight

### Error Handling

- **Plain language errors** — Users see clear messages, no error codes or field names
- **Recovery actions** — Every error must include `NonEmpty<RecoveryAction>`
  - `CreateMissing`, `UseSuggested`, `EditField`, `PostAnyway`, `Discard`, `ShowHelp`
- **Logs for details** — Internal codes and stack traces go to logs only

### AI Boundary

- **AI submits proposals** — Never writes directly to the database
- **Rust core validates** — `TransactionProposal` → `ValidationResult` → commit or reject
- **Tool use only** — Claude API should use tool use, never free-form text parsing

## Testing

Tests are required before merging:

```bash
# TypeScript tests (Vitest)
pnpm test

# Type checking
pnpm typecheck

# Rust tests
cargo test --all

# Coverage target: 80%+
```

## Pull Request Process

1. **Branch off `main`** with a descriptive name
2. **Test locally** — Ensure tests pass and types are clean
3. **Write a clear PR description**
   - What does it do?
   - Why is it needed?
   - Any tradeoffs or decisions?
4. **Address feedback** — We may ask for changes
5. **One approval** — At least one maintainer must review before merge

See [PULL_REQUEST_TEMPLATE.md](.github/PULL_REQUEST_TEMPLATE.md) for the checklist.

## Architecture Rules (Non-Negotiable)

Read [CLAUDE.md](CLAUDE.md) for the full list. Key ones:

- Money is **always integer cents**
- AI **never writes** to the database directly
- `audit_log` is **INSERT-only**
- `journal_lines.amount` is **always positive** (side field encodes direction)
- Every error must have a `RecoveryAction`

## What We're Looking For

- Bug fixes with tests
- Improved documentation
- Better error messages
- Performance improvements (with benchmarks)
- Accessibility improvements
- Code quality & refactoring (that respects the rules above)

## What We're Not Looking For (Phase 1)

- Mobile support (coming Phase 2)
- Multi-user or cloud sync (coming Phase 2)
- Automated transaction import (coming Phase 2)
- New AI backends (Claude only in Phase 1)
- Database migrations to different engines

## Questions?

- Check [CLAUDE.md](CLAUDE.md) for detailed conventions
- Open a discussion issue
- Email rmwarriner@icloud.com

---

Thank you for contributing to Tally.ai!
