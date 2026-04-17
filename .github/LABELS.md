# GitHub Labels

This document defines all labels used in the Tally.ai repository. Create these in GitHub settings under **Labels**.

## Type Labels

| Label | Color | Description |
|-------|-------|-------------|
| `bug` | `#d73a4a` | Something is broken |
| `enhancement` | `#a2eeef` | New feature or improvement |
| `documentation` | `#0075ca` | Docs, guides, README updates |
| `refactor` | `#fbca04` | Code quality, cleanup (no behavior change) |
| `performance` | `#ffd700` | Speed, memory, efficiency improvement |
| `security` | `#8b0000` | Security vulnerability or hardening |
| `test` | `#cccccc` | Test coverage, test improvements |
| `chore` | `#f0f0f0` | Deps, CI, tooling, not user-facing |

## Status Labels

| Label | Color | Description |
|-------|-------|-------------|
| `triage` | `#d4c5f9` | Needs review and prioritization |
| `blocked` | `#ee0701` | Waiting on something else (comment why) |
| `help-wanted` | `#008672` | Good for contributors, needs hands |
| `good-first-issue` | `#7057ff` | Entry point for new contributors |
| `in-progress` | `#ffd700` | Someone is actively working on it |
| `review` | `#fbca04` | PR or code ready for review |
| `wontfix` | `#ffffff` | Not planned / intentional decision |

## Domain Labels

| Label | Color | Description |
|-------|-------|-------------|
| `frontend` | `#c2e0c6` | React / UI / TypeScript |
| `backend` | `#ffeaa7` | Rust / Tauri / database |
| `ai` | `#dda15e` | Claude AI integration |
| `crypto` | `#bc6c25` | Encryption, key derivation, security |
| `database` | `#d8bfd8` | SQLCipher, migrations, schema |

## Phase Labels

| Label | Color | Description |
|-------|-------|-------------|
| `phase-1` | `#b7e4c7` | Phase 1 scope (now) |
| `phase-2` | `#a8dadc` | Phase 2 scope (future) |
| `phase-3` | `#f1faee` | Phase 3 scope (future) |

## Discussion Labels

| Label | Color | Description |
|-------|-------|-------------|
| `idea` | `#e799ff` | Feature idea or RFE |
| `question` | `#d4af37` | Question / how-to |
| `discussion` | `#c5def5` | Architectural or design discussion |

## Dependency Labels

| Label | Color | Description |
|-------|-------|-------------|
| `dependencies` | `#0366d6` | Dependabot: npm/Cargo/Action updates |
| `dependencies:npm` | `#cb2431` | npm dependency update |
| `dependencies:cargo` | `#ff7f50` | Cargo (Rust) dependency update |
| `ci` | `#1d76db` | CI/CD, GitHub Actions |

---

## How to Create Labels

### Via GitHub CLI

```bash
# Create a single label
gh label create bug -c d73a4a -d "Something is broken"

# Create all at once (using gh CLI with loop)
# See script below
```

### Via Script

```bash
#!/bin/bash
# Save as scripts/create-labels.sh

REPO="rmwarriner/tally-ai"

labels=(
  "bug|d73a4a|Something is broken"
  "enhancement|a2eeef|New feature or improvement"
  "documentation|0075ca|Docs, guides, README updates"
  "refactor|fbca04|Code quality, cleanup"
  "performance|ffd700|Speed, memory, efficiency"
  "security|8b0000|Security vulnerability or hardening"
  "test|cccccc|Test coverage improvements"
  "chore|f0f0f0|Deps, CI, tooling"
  "triage|d4c5f9|Needs review and prioritization"
  "blocked|ee0701|Waiting on something else"
  "help-wanted|008672|Good for contributors"
  "good-first-issue|7057ff|Entry point for new contributors"
  "in-progress|ffd700|Someone is actively working"
  "review|fbca04|Ready for review"
  "wontfix|ffffff|Not planned"
  "frontend|c2e0c6|React / UI / TypeScript"
  "backend|ffeaa7|Rust / Tauri / database"
  "ai|dda15e|Claude AI integration"
  "crypto|bc6c25|Encryption and security"
  "database|d8bfd8|SQLCipher, migrations, schema"
  "phase-1|b7e4c7|Phase 1 scope"
  "phase-2|a8dadc|Phase 2 scope"
  "phase-3|f1faee|Phase 3 scope"
  "idea|e799ff|Feature idea or RFE"
  "question|d4af37|Question or how-to"
  "discussion|c5def5|Architectural discussion"
  "dependencies|0366d6|Dependabot updates"
  "dependencies:npm|cb2431|npm dependency update"
  "dependencies:cargo|ff7f50|Cargo dependency update"
  "ci|1d76db|CI/CD and GitHub Actions"
)

for label in "${labels[@]}"; do
  IFS='|' read -r name color desc <<< "$label"
  gh label create "$name" -c "$color" -d "$desc" -R "$REPO" 2>/dev/null && echo "✓ Created: $name" || echo "✗ Failed or exists: $name"
done
```

Run it:
```bash
chmod +x scripts/create-labels.sh
./scripts/create-labels.sh
```

### Via GitHub Web UI

1. Go to **Settings** → **Labels**
2. Click **New label**
3. Enter name, color, description
4. Repeat for each label

---

## Label Usage in Workflows

### Auto-label by file change

See `.github/workflows/auto-label.yml` for automatic labeling based on which files change.

### Manual labeling

When creating/triaging issues, add these labels:
- **One type label** (`bug`, `enhancement`, `documentation`, etc.)
- **Zero or more domain labels** (`frontend`, `backend`, `ai`, `database`, etc.)
- **One phase label** (`phase-1`, `phase-2`, `phase-3`)
- **Status labels as needed** (`triage`, `blocked`, `help-wanted`, etc.)

---

## Example Label Combinations

**Bug in frontend (Phase 1):**
- `bug`, `frontend`, `phase-1`, `triage`

**Feature idea for Phase 2:**
- `enhancement`, `phase-2` (keep as discussion until approved)

**Good first task:**
- `good-first-issue`, `help-wanted`, `frontend` or `backend`

**Security issue:**
- `bug`, `security`, `blocked` (assign fix)
