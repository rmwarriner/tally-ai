# Issue Triage Guide

This guide is for maintainers triaging new issues.

## Goals

- **Clarify intent** — Ensure the issue is clear and actionable
- **Label properly** — Type, phase, domain, priority
- **Prioritize** — Set priority and estimate effort
- **Route correctly** — Assign to backlog, milestone, or maintainer
- **Close duplicates** — Merge with existing issues when applicable

## Triage Checklist

When a new issue comes in:

- [ ] **Read the issue** — Understand what's being reported/requested
- [ ] **Check for duplicates** — Search existing issues
- [ ] **Clarify if needed** — Comment with questions (don't close)
- [ ] **Add Type label** — `bug`, `enhancement`, `documentation`, `refactor`
- [ ] **Add Domain label** — `frontend`, `backend`, `ai`, `database`, `crypto`
- [ ] **Add Phase label** — `phase-1`, `phase-2`, `phase-3`
- [ ] **Set Priority** — Critical, High, Medium, Low
- [ ] **Estimate Effort** — 1pt, 2pt, 3pt, 5pt, 8pt (if applicable)
- [ ] **Add to Project** — Move to project board
- [ ] **Decide** — Accept, reject, or request more info

## By Issue Type

### Bug Report

**Questions to ask:**
- Can I reproduce it?
- What's the impact? (UI, data loss, crash?)
- Does it affect Phase 1 or Phase 2?
- Is there a workaround?

**Labels:**
- `bug` (always)
- Domain: `frontend`, `backend`, `ai`, `crypto`, `database`
- Phase: `phase-1` (critical bugs), `phase-2` or later (edge cases)
- Status: `triage` → accepted labels
- Priority: Critical (data loss, crash), High (broken feature), Medium (workaround exists), Low (edge case)

**Decision:**
- **Accept** (for Phase 1): Add to project, estimate effort, label `help-wanted` if it's a good entry point
- **Defer** (to Phase 2): Keep issue open, label `phase-2`
- **Reject** (if not reproducible or by design): Close with explanation

### Feature Request

**Questions to ask:**
- Does it fit Phase 1 scope?
- Is it a new feature or improvement to existing?
- What's the user value?
- How complex is it?

**Labels:**
- `enhancement` (always)
- Domain: `frontend`, `backend`, `ai`, `database`
- Phase: `phase-1` (in scope now), `phase-2` (future)
- Status: Start as `triage`, only move to backlog if approved for this phase

**Decision:**
- **Backlog** (Phase 1 or 2): Add to project, estimate effort, label `good-first-issue` if small enough
- **Discussion** (needs refinement): Close issue, suggest moving to Discussions for further community input
- **Wontfix** (out of scope): Close with `wontfix` label and explanation

### Documentation

**Questions to ask:**
- What docs are missing?
- Who's the audience?
- Is this critical or nice-to-have?

**Labels:**
- `documentation` (always)
- Phase: Usually `phase-1`
- Priority: Usually Medium or Low
- Effort: Usually 1-2pt

**Decision:**
- **Accept** (if clear): Add to project
- **Clarify** (if vague): Comment with examples of what's needed
- **Defer** (if it depends on Phase 2 features): Label `phase-2`

## Priority Guidelines

### Critical
- Data loss or corruption
- Security vulnerability
- Crash on common operation
- Complete feature broken
- **Action:** Fix immediately, don't defer to Phase 2

### High
- Feature partially broken
- Poor UX that blocks workflow
- Performance regression
- **Effort:** Typically 3-8pt
- **Phase:** Try to include in current phase, but okay to defer

### Medium
- Feature works but has issues
- Edge cases or rare bugs
- Nice-to-have improvements
- **Effort:** Typically 1-5pt
- **Phase:** Can be Phase 2 if not core to Phase 1

### Low
- Polish or cosmetic issues
- Future-proofing
- "Nice to have" features
- **Effort:** Typically 1-3pt
- **Phase:** Often Phase 2+

## Effort Estimation

Use Story Points (Fibonacci):

- **1pt** — Trivial (typo fix, single line, well-defined)
- **2pt** — Small (simple fix, minor feature, isolated)
- **3pt** — Medium (moderate change, affects multiple areas)
- **5pt** — Large (significant feature, multiple files)
- **8pt** — Very Large (major refactor, unclear scope, needs planning)

If you estimate 8pt, it might need to be split into smaller tasks.

## Good First Issue

Mark as `good-first-issue` if:
- **Effort is 1-2pt** (not 5-8pt)
- **Isolated** (doesn't require learning half the codebase)
- **Has clear acceptance criteria**
- **Doesn't block other work**
- **Includes links to relevant code**

Add comment:
```
Great for first-time contributors! Here's what needs to happen:

1. [Specific task]
2. [Specific task]
3. Run tests with `pnpm test`
4. Follow [CONTRIBUTING.md](CONTRIBUTING.md) for PR format

Let us know if you have questions!
```

## Duplicate Issues

When you find duplicates:

1. **Add label** `duplicate`
2. **Comment** linking to the original
3. **Close** with reference: "Duplicate of #123"

If the duplicate has more context, comment on the original with the additional info before closing.

## Blocking Issues

If an issue is blocked by another:

1. **Add label** `blocked`
2. **Add comment** explaining what it's blocked by
3. **Link** to the blocking issue
4. **Move from Backlog to Blocked** column (if using project board)

Example:
```
Blocked by #456 (need encryption setup first).
Will unblock once that PR merges.
```

## When in Doubt

Ask in the issue:
- "Can you provide a minimal reproduction?"
- "Does this affect Phase 1 or Phase 2?"
- "What's the priority if we can only do one of these?"
- "Would you like to work on this?"

Better to clarify upfront than commit to something unclear.

## Common Rejections

**Not reproducible**
```
Thanks for reporting! I wasn't able to reproduce this. 
Could you provide:
- Steps to reproduce
- OS and app version
- Any error messages from the logs

Once we can reproduce it, we'll reopen.
```

**Out of Phase 1 scope**
```
Thanks for the suggestion! This is a great Phase 2 feature.
We're keeping Phase 1 focused on [core features].
We'll revisit this after Phase 1 ships. Feel free to discuss in Discussions!
```

**Needs design/discussion**
```
This needs more design thinking. Please start a Discussion
so the community can help flesh out the approach.
Once we have consensus, we can convert this to an issue.
```

---

## Tools

- **GitHub Insights** — See issues by label, milestone, assignee
- **GitHub CLI** — `gh issue list`, `gh issue edit`, bulk label operations
- **Milestones** — Group issues by release
- **Projects** — Visual backlog management

---

## Triage Cadence

- **Daily** (if possible) — Check and label new issues
- **Weekly** — Priority and roadmap review
- **Before release** — Milestone cleanup, close stale issues
