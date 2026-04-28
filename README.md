# Tally.ai

A conversational household finance app that makes tracking money effortless. Just chat—no forms, no spreadsheets.

## Quick Start

### Prerequisites

- Node.js ≥ 22
- pnpm ≥ 10
- Rust (for the Tauri backend)
- macOS, Windows, or Linux

### Setup

```bash
# Install dependencies
pnpm install

# Run the full Tauri app (Rust backend + frontend)
pnpm tauri:dev

# OR run the frontend only (no Tauri IPC; useful for browser-based a11y tools)
pnpm dev

# Run all tests (Rust + TS)
pnpm test
cargo test --all

# Type-check + lint
pnpm typecheck
pnpm lint
```

## Architecture

Tally.ai is built as a Tauri desktop app with three layers:

- **Tauri Backend** (Rust) — SQLCipher database, transaction validation, audit logging
- **React Frontend** (TypeScript) — Chat interface, real-time updates via TanStack Query
- **Claude AI** — Natural language understanding via tool use, never writes directly to DB

```
┌─────────────────────────────────────────┐
│        React Chat Interface             │
│   (Tauri Webview / TanStack Query)      │
└────────────────┬────────────────────────┘
                 │
┌────────────────▼────────────────────────┐
│      Claude API (Tool Use)              │
│   Generates TransactionProposal objects │
└────────────────┬────────────────────────┘
                 │
┌────────────────▼────────────────────────┐
│      Tauri Command Handler              │
│   Validates & commits proposals         │
└────────────────┬────────────────────────┘
                 │
┌────────────────▼────────────────────────┐
│     SQLCipher Database (Encrypted)      │
│    Money stored as INTEGER cents        │
└─────────────────────────────────────────┘
```

## Key Principles

- **Money as integers** — All amounts stored as cents (no floats)
- **Validated writes** — AI proposes, Rust core validates and commits
- **Encrypted at rest** — SQLCipher with Argon2id key derivation
- **Audit trail** — All changes logged immutably
- **Plain language** — User-facing messages, never error codes

## Project Structure

```
tally.ai/
├── apps/
│   └── desktop/           # Tauri app & React frontend
│       ├── src/           # React components & hooks
│       ├── src-tauri/     # Rust backend
│       │   └── src/
│       │       ├── db/    # SQLCipher schema & migrations
│       │       └── ai/    # Claude API orchestration
│       └── vite.config.ts
├── packages/
│   └── core-types/        # Shared TypeScript types
├── CLAUDE.md              # Detailed dev conventions
├── package.json           # pnpm workspace root
└── Cargo.toml             # Rust workspace
```

## Development

### Conventions

- **Commits** — Conventional commits (`feat:`, `fix:`, `test:`, `docs:`)
- **Branches** — Feature branches off `main`, no direct commits
- **Tests** — TDD-first, 80% coverage enforced pre-commit (Vitest + Rust tests)
- **Types** — TypeScript strict mode, no `any`
- **State** — Zustand for UI, TanStack Query for server state

See [CLAUDE.md](CLAUDE.md) for full conventions.

### Running Tests

```bash
# TypeScript + React
pnpm test

# Rust
cargo test --all

# All with coverage
pnpm test
```

### Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidance on PRs, commit style, and code review expectations.

## Security

Found a security issue? Please see [SECURITY.md](SECURITY.md) for responsible disclosure.

## Getting Involved

### Report a Bug

Found an issue? [Open a bug report](../../issues/new?template=bug_report.md).

### Suggest a Feature

Have an idea? [Start a discussion](../../discussions/new?category=Ideas) or [request a feature](../../issues/new?template=feature_request.md).

### Ask a Question

Need help? [Ask in Discussions](../../discussions/new?category=Q-and-A) — it helps future users too.

### Track Our Work

See what we're working on in the [GitHub Project Board](../../projects) (backlog, in-progress, review, done).

### Contribute Code

See [CONTRIBUTING.md](CONTRIBUTING.md) for conventions, testing, and how to submit PRs.

## License

MIT — see [LICENSE](LICENSE)

## Phase 1 Scope

- Desktop only (Tauri); mobile and sync coming later
- Manual entry only; SimpleFIN & file import in Phase 2
- Claude API only; other models planned for Phase 2

---

Built with [Tauri](https://tauri.app), [React](https://react.dev), and [Claude AI](https://claude.ai).
