# Repository Navigation Guide

Quick reference for where to find things in the Tally.ai repository.

## For Users

- **[README.md](../README.md)** — Start here. Project overview, quick-start, architecture.
- **[Issues](../../issues)** — Report bugs or request features.
- **[Discussions](../../discussions)** — Ask questions, share ideas, discuss architecture.
- **[Project Board](../../projects)** — See what we're working on.
- **[Releases](../../releases)** — Download the app or see what's new.

## For Contributors

### Getting Started

1. **[CONTRIBUTING.md](../CONTRIBUTING.md)** — How to contribute, conventions, testing
2. **[CLAUDE.md](../CLAUDE.md)** — Detailed dev rules and architecture
3. **[DECISIONS.md](../DECISIONS.md)** — Why we made certain choices

### Code & Tests

- **[apps/desktop/src/](../apps/desktop/src/)** — React components and frontend
- **[apps/desktop/src-tauri/](../apps/desktop/src-tauri/)** — Rust backend
- **[packages/core-types/](../packages/core-types/)** — Shared TypeScript types

### Development

- **[.github/workflows/ci.yml](.github/workflows/ci.yml)** — CI/CD pipeline
- **[.github/BRANCH_PROTECTION.md](./BRANCH_PROTECTION.md)** — How branch protection works

## For Maintainers

### Issue & Project Management

- **[.github/TRIAGE.md](./TRIAGE.md)** — How to triage new issues
- **[.github/LABELS.md](./LABELS.md)** — Label definitions and how to create them
- **[.github/PROJECT_BOARD.md](./PROJECT_BOARD.md)** — Project board setup and workflow

### GitHub Setup

- **[.github/CODEOWNERS](./CODEOWNERS)** — Code ownership assignments
- **[.github/dependabot.yml](./dependabot.yml)** — Dependency update automation
- **[.github/workflows/](./workflows/)** — All automation workflows

### Documentation & Policies

- **[SECURITY.md](../SECURITY.md)** — Security reporting, principles, compliance
- **[CODE_OF_CONDUCT.md](../CODE_OF_CONDUCT.md)** — Community standards
- **[LICENSE](../LICENSE)** — MIT license

## Directory Structure

```
tally.ai/
├── .github/
│   ├── workflows/              # GitHub Actions
│   │   ├── ci.yml              # Test, typecheck, Rust tests
│   │   └── auto-label.yml      # Auto-label by file changes
│   ├── ISSUE_TEMPLATE/         # Issue templates
│   ├── DISCUSSION_TEMPLATE/    # Discussion templates
│   ├── PULL_REQUEST_TEMPLATE.md
│   ├── CODEOWNERS
│   ├── dependabot.yml
│   ├── TRIAGE.md               # How to triage issues
│   ├── LABELS.md               # Label definitions
│   ├── PROJECT_BOARD.md        # Project board guide
│   └── BRANCH_PROTECTION.md    # Branch protection guide
│
├── apps/
│   └── desktop/
│       ├── src/                # React frontend
│       ├── src-tauri/          # Rust backend
│       └── vite.config.ts
│
├── packages/
│   └── core-types/             # Shared TypeScript types
│
├── docs/                        # Project documentation (future)
│
├── CLAUDE.md                    # Dev conventions (non-negotiable)
├── CONTRIBUTING.md             # How to contribute
├── DECISIONS.md                # Architectural decisions
├── SECURITY.md                 # Security policy
├── CODE_OF_CONDUCT.md          # Community guidelines
├── README.md                   # Project overview
├── LICENSE                     # MIT license
├── package.json                # npm workspace root
├── Cargo.toml                  # Rust workspace
└── pnpm-workspace.yaml         # pnpm workspace config
```

## Common Tasks

### "I want to report a bug"
→ [Issues](../../issues) → New issue → [Bug Report](.github/ISSUE_TEMPLATE/bug_report.md)

### "I have a feature idea"
→ [Discussions](../../discussions) → New discussion → Ideas
→ Or [Issues](../../issues) → New issue → [Feature Request](.github/ISSUE_TEMPLATE/feature_request.md)

### "I have a question"
→ [Discussions](../../discussions) → New discussion → Q&A

### "I want to contribute code"
→ Read [CONTRIBUTING.md](../CONTRIBUTING.md)
→ Read [CLAUDE.md](../CLAUDE.md) for rules
→ Pick an issue from [Project Board](../../projects) → Ready column
→ Create a feature branch, submit PR

### "I want to understand the architecture"
→ [README.md](../README.md) (overview)
→ [CLAUDE.md](../CLAUDE.md) (detailed rules)
→ [DECISIONS.md](../DECISIONS.md) (why choices were made)

### "I'm triaging a new issue"
→ Read [.github/TRIAGE.md](./TRIAGE.md)
→ Use [.github/LABELS.md](./LABELS.md) to label it
→ Add to [Project Board](../../projects)

### "I want to understand the workflow"
→ [CONTRIBUTING.md](../CONTRIBUTING.md) (Issue & Discussion Workflow section)
→ [.github/PROJECT_BOARD.md](./PROJECT_BOARD.md) (Workflow: From Idea to Done)

## External Links

- **GitHub**: https://github.com/rmwarriner/tally-ai
- **Issues**: https://github.com/rmwarriner/tally-ai/issues
- **Discussions**: https://github.com/rmwarriner/tally-ai/discussions
- **Project Board**: https://github.com/rmwarriner/tally-ai/projects

---

**Lost? Start with [README.md](../README.md), then ask in [Discussions](../../discussions).**
