# GitHub Project Board Setup

Tally.ai uses GitHub Projects (new format) as the single source of truth for issue tracking and planning. This replaces local markdown backlogs.

## Why GitHub Projects?

- **Centralized**: Everything lives on GitHub, no separate docs to maintain
- **Integrated**: Linked to issues, PRs, discussions
- **Flexible**: Drag-and-drop columns, custom fields, automation
- **Visible**: Team can see progress at a glance
- **Durable**: History is preserved, decisions are auditable

## How to Create the Project

### Step 1: Create a new project

1. Go to your repo → **Projects** tab
2. Click **New project**
3. Choose **Table** layout (more flexible than board for this use case)
4. Name: **Tally.ai Backlog**
5. Create it

### Step 2: Add custom fields

Add these fields to track status:

| Field Name | Type | Values |
|-----------|------|--------|
| Status | Single select | `Backlog` `Ready` `In Progress` `Review` `Done` |
| Priority | Single select | `Critical` `High` `Medium` `Low` |
| Phase | Single select | `Phase 1` `Phase 2` `Phase 3` |
| Effort | Single select | `1pt` `2pt` `3pt` `5pt` `8pt` |

### Step 3: Set automation

Go to **Project settings** → **Automation**:

- **Auto-add PRs**: When PRs are created, add to project (Status: `Review`)
- **Auto-add issues**: When issues are labeled `phase-1`, add to project (Status: `Backlog`)
- **Auto-close**: When PR merged, move Status to `Done`

### Step 4: Populate initial items

Start with:
- Backlog of known bugs from Phase 1 CLAUDE.md
- Planned Phase 1 features
- Phase 2 placeholder items (for reference)

## Workflow: From Idea to Done

### 1. **Idea** (GitHub Discussion)
- Posted in Discussions as "idea"
- Community discusses
- No project item yet

### 2. **Approved → Issue**
- Create an **Issue** in the repo
- Label: Type (`bug`, `enhancement`, `documentation`)
- Label: Phase (`phase-1`, `phase-2`, `phase-3`)
- Label: Domain (`frontend`, `backend`, `ai`, `database`, `crypto`)
- Label: Status (`triage`)

### 3. **Triaged → Backlog**
- Maintainer reviews and prioritizes
- Remove `triage`, add appropriate status labels
- Add to project board with Status: `Backlog`
- Set Priority and Effort estimates

### 4. **Ready to Work → Assigned**
- Issue Status: `in-progress` label
- Move to `Ready` or `In Progress` column
- Assign to someone (or open for volunteers)
- Create feature branch: `feature/issue-123-description`

### 5. **Working → PR**
- Open PR against `main`
- Link PR to issue: Type `Fixes #123` in PR body
- Project auto-adds PR, Status: `Review`
- Code review happens

### 6. **Approved → Merged**
- Approve and merge
- PR auto-closes linked issue
- Project auto-moves to `Done`

### 7. **Done**
- PR merged
- Issue closed
- Project shows as complete
- Shipped in next release

## Project View Examples

### Board View (by Status)

```
Backlog              Ready                In Progress          Review               Done
─────────────────────────────────────────────────────────────────────────────────────
[Bug] Fix decimal    [Enh] Multi-currency [Bug] Encrypt key    [Enh] Add account    [Feat] Migrations
rounding (P:High)    support (P:Medium)   derivation (P:High)  types (P:Medium)     ✓ (Phase 1)

[Enh] Categories     [Bug] Handle edge    [Refactor] Error     [Fix] Session        [Bug] Fix crash
(P:Medium)           cases (P:Low)        handling (P:Medium)  timeout (P:High)     on empty DB ✓
```

### Filtered View (Phase 1 only)

Show only Phase 1 items, grouped by Priority:

```
Critical (5 items)   High (8 items)       Medium (12 items)    Low (3 items)
────────────────────────────────────────────────────────────────────────
Database encryption  AI tool use          Categories           Nice-to-haves
Session handling     Transaction proposal Recurring trans.     Future refactors
Audit log setup      Error recovery       UI polish            Documentation
```

## Example Labels for Issues

**Bug in backend, Phase 1, High priority:**
```
Labels: bug, backend, phase-1
Priority: High
Status: In Progress (if assigned) or Backlog (if not)
```

**Feature idea for Phase 2:**
```
Keep as Discussion with label: idea
Once approved: Create Issue with labels: enhancement, phase-2, [domain]
Status: Backlog (for Phase 2 planning)
```

**Good task for new contributor:**
```
Labels: good-first-issue, help-wanted, phase-1
Priority: Medium or Low (not critical)
Effort: 2pt or 3pt (not 8pt!)
Status: Ready
```

## Milestones (Optional)

Create milestones for releases:

- **v0.1.0 (Phase 1 MVP)** — Core features, encryption, audit log
- **v0.2.0 (Phase 2)** — Multi-user, sync, SimpleFIN
- **v1.0.0** — Production release

Assign issues to milestones to track release readiness.

## Reports & Insights

Use GitHub's built-in insights:

- **Burndown**: Track progress by Effort
- **Velocity**: Issues closed per sprint
- **Cycle time**: Time from backlog to done
- **Distribution**: By Phase, Domain, Priority

---

## Quick Commands (GitHub CLI)

Create and assign an issue:
```bash
gh issue create -t "Fix: [description]" -l "bug,backend,phase-1" -a rmwarriner
```

Link a PR to an issue:
```bash
# In PR body: Fixes #123
```

Add to project:
```bash
gh project item-add [project-id] --owner=rmwarriner --repo=tally-ai
```

---

## Next Steps

1. Create the project board using steps above
2. Enable automation in project settings
3. Add initial backlog of Phase 1 issues
4. Update this repo's README to link to the project
5. Move all local backlog items to Issues if any exist
