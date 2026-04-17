# GitHub Rulesets

Rulesets enforce code quality patterns at the git level. Unlike branch protection (which is PR-focused), rulesets prevent bad commits/pushes from happening in the first place.

**Note:** These complement branch protection; they don't conflict.

## Rulesets to Create

### 1. Enforce Conventional Commits

**What:** Commit messages must start with `feat:`, `fix:`, `test:`, `docs:`, `chore:`, or `refactor:`

**Pattern:** `^(feat|fix|test|docs|chore|refactor|perf|style|ci)(\(.+\))?!?: .+`

**Applies to:** All branches, all users (including admins)

**Why:** Keeps git history queryable and release notes auto-generatable

---

### 2. Require Linear History

**What:** Only allow squash or rebase merges; no merge commits

**Applies to:** `main` branch

**Why:** Keeps git log clean, makes history readable, easier to bisect

---

## Protecting Critical Files (CODEOWNERS Alternative)

**Note:** GitHub push rules (file restrictions) are **not available for public repos**—only org-owned repos.

Instead, use **CODEOWNERS + Branch Protection**:

- [.github/CODEOWNERS](./CODEOWNERS) already lists critical files
- Enable "Require code owner review" in branch protection
- Changes to CLAUDE.md, DECISIONS.md, SECURITY.md, LICENSE, CODEOWNERS require your approval

This approach:
- ✅ Works on public repos
- ✅ Requires PR (branch protection)
- ✅ Requires your review (CODEOWNERS)
- ✅ Files can't be merged without your sign-off

See [Branch Protection Setup](.github/BRANCH_PROTECTION_SETUP.md) — it's already configured.

---

## Setup Options

### Option A: Web UI (Easiest)

1. Go to **Settings** → **Rules** → **Rulesets**
2. Click **New ruleset** → **New branch ruleset**
3. Create the two rulesets below (skip file protection—not available for public repos)

---

## Ruleset 1: Conventional Commits

**Name:** `Enforce Conventional Commits`

**Target:** 
- All branches
- All users

**Rules:**
- ✅ **Commit message pattern**
  - Pattern: `^(feat|fix|test|docs|chore|refactor|perf|style|ci)(\(.+\))?!?: .+`
  - Example valid: `feat: add transaction categories`, `fix(db): handle null amounts`
  - Example invalid: `updated stuff`, `WIP`, `asdf`

**Bypass:**
- Allow admins to bypass (for emergency hotfixes, but discourage)

**Enforcement:**
- Enforce for everyone (including admins)
- This prevents bad commits upfront

---

## Ruleset 2: Protect Critical Files

**Name:** `Protect Critical Files`

**Target:**
- All branches
- All users

**Rules:**
- ✅ **Restrict file changes**
  - Files: `CLAUDE.md`, `DECISIONS.md`, `SECURITY.md`, `LICENSE`, `.github/CODEOWNERS`
  - Restriction: Require pull request
  - Allow dismissal: No (changes must go through PR)

**Bypass:**
- Admins only (for emergency fixes)

**Enforcement:**
- Can't edit these files directly; must PR

**How it works:**
```bash
# This will be BLOCKED:
echo "new rule" >> CLAUDE.md
git commit -m "chore: update CLAUDE.md"
git push origin main  # ❌ Rejected by ruleset

# This will SUCCEED:
git checkout -b docs/update-claude
echo "new rule" >> CLAUDE.md
git commit -m "docs: update CLAUDE.md"
git push origin docs/update-claude
# Then create PR, get review, merge
```

---

## Ruleset 3: Linear History (main branch)

**Name:** `Linear History for main`

**Target:**
- Branch: `main` only
- All users

**Rules:**
- ✅ **Require pull request before merging**
  - 1 approval required
- ✅ **Require branches to be up to date before merging**
- ✅ **Require status checks to pass**
  - `test`, `typecheck`, `rust-test`
- ✅ **Require merge method**
  - Disallow: Merge commits
  - Allow: Squash or rebase

**Bypass:**
- Admins only (for emergency hotfixes)

**Why squash/rebase?**
- Merge commits create a messy tree: `\ / \ / \ /`
- Squash/rebase keep a straight line: `—————→`
- Easier to read history, easier to bisect

---

## Web UI Setup Instructions

### Ruleset 1: Conventional Commits

1. Go to **Settings** → **Rules** → **Rulesets**
2. Click **New ruleset** → **New branch ruleset**
3. **Name:** `Enforce Conventional Commits`
4. **Enforcement level:** **Active**
5. **Target branches:** 
   - Click **Add target** → `Include default branch (main)`
6. **Add rules:**
   - Click **Add rules**
   - Find **Commit message pattern**
   - Pattern: `^(feat|fix|test|docs|chore|refactor|perf|style|ci)(\(.+\))?!?: .+`
   - Description: `Conventional commit format required`
   - Check: **Enforce for everyone**
7. Click **Create**

### Ruleset 2: Linear History

1. **New ruleset** → **New branch ruleset**
2. **Name:** `Linear History for main`
3. **Enforcement level:** **Active**
4. **Target branches:** `main` only
5. **Add rules:**
   - **Require merge method**
     - Disallow: Merge commits
     - Allow: Squash and rebase
   - **Require pull request before merging**
     - 1 approval required
   - **Require branches to be up to date before merging**
   - **Require status checks to pass**
     - `test`, `typecheck`, `rust-test`
6. Click **Create**

---

## Verification

After creating rulesets, test them:

### Test conventional commits:

```bash
# This should be rejected:
git commit -m "fixed stuff"
git push origin main  # ❌ Rejected

# This should work:
git commit -m "fix: correct decimal rounding"
git push origin main  # ✅ Allowed
```

### Test linear history:

```bash
# On main, after merging a PR:
git log --oneline main
# Should see: —————→ (straight line)
# NOT:        \ / (merge commit)
```

### Verify file protection (CODEOWNERS):

File protection uses CODEOWNERS + branch protection:
- Try editing `CLAUDE.md` directly on GitHub
- You must create a PR
- PR requires your approval (CODEOWNERS review)
- This works because branch protection requires "Require CODEOWNERS review"

See: `.github/BRANCH_PROTECTION_SETUP.md`

---

## How Rulesets Work With Branch Protection

| Feature | Branch Protection | Ruleset | Result |
|---------|-------------------|---------|--------|
| Require PR | ✅ Yes | ✅ Yes | Both required (cumulative) |
| Status checks | ✅ Yes | ✅ Yes | Both checked |
| Linear history | ❌ No | ✅ Yes | Only ruleset enforces |
| Commit messages | ❌ No | ✅ Yes | Only ruleset enforces |
| File protection | ✅ CODEOWNERS | ❌ Not public repos | Branch protection enforces via CODEOWNERS |

**No conflicts.** Rulesets add new enforcement; branch protection still handles PR reviews.

**Note on file protection:** Push rules aren't available for public repos. Use CODEOWNERS + branch protection instead (already configured).

---

## Troubleshooting

### "Commit rejected: message doesn't match pattern"

Your commit message doesn't follow conventional commits. Fix it:

```bash
git commit --amend -m "feat: your message here"
git push --force-with-lease origin your-branch
```

### "Commit rejected: only squash/rebase allowed"

You're trying to merge with a merge commit. GitHub enforces squash/rebase on main.
When merging a PR, select **Squash and merge** instead of **Merge pull request**.

---

## Enforcement Levels

**Active** — Rules are enforced; violations block commits/merges

**Evaluate** — Test mode; violations logged but not blocked (useful before enabling)

**Disabled** — Not enforced

Start with **Evaluate** if you want to test before enforcing.

---

## Bypass Policies

Each ruleset can have a bypass list:

- **No one** — Enforced for everyone (including admins)
- **Admins** — Admins can bypass (for emergencies)
- **Specific users/teams** — Designate who can bypass

**Recommended:**
- Conventional commits: Enforce for everyone (no bypasses)
- File protection: Allow admin bypass (rare emergencies)
- Linear history: Allow admin bypass (rare merges)

---

## Next Steps

1. ✅ Create ruleset 1: Conventional commits
2. ✅ Create ruleset 2: Linear history
3. ✅ Verify branch protection has "Require CODEOWNERS review" enabled
4. Test each ruleset
5. Document in team wiki/handbook

Then your repo enforces:
- ✅ Clean commit messages (ruleset)
- ✅ Protected architectural docs (CODEOWNERS + branch protection)
- ✅ Linear, readable history (ruleset)
- ✅ Code reviews (branch protection)
- ✅ Status checks (branch protection)

**All committed to code quality. ✨**
