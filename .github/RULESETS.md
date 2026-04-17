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

### 2. Protect Critical Files

**What:** CLAUDE.md, DECISIONS.md, SECURITY.md, LICENSE can only be changed via PR (no direct pushes)

**Files:**
- `CLAUDE.md` (architectural rules)
- `DECISIONS.md` (decision log)
- `SECURITY.md` (security policy)
- `LICENSE` (license file)
- `.github/CODEOWNERS` (code owners)

**Enforcement:** Require PR review before merging

**Why:** These files define project fundamentals; changes should be intentional and reviewed

---

### 3. Require Linear History

**What:** Only allow squash or rebase merges; no merge commits

**Applies to:** `main` branch

**Why:** Keeps git log clean, makes history readable, easier to bisect

---

## Setup Options

### Option A: Web UI (Easiest)

1. Go to **Settings** → **Rules** → **Rulesets**
2. Click **New ruleset** → **New branch ruleset**
3. For each ruleset below, create one

### Option B: GraphQL API

Use the GraphQL mutations in `RULESETS_API.md` to create all three at once.

```bash
gh api graphql -f query=@rulesets.graphql
```

### Option C: Terraform

See `terraform/rulesets.tf` for infrastructure-as-code setup.

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

### Ruleset 2: Protect Critical Files

1. **New ruleset** → **New branch ruleset**
2. **Name:** `Protect Critical Files`
3. **Enforcement level:** **Active**
4. **Target branches:** All branches
5. **Add rules:**
   - **Restrict file changes**
   - Files: `CLAUDE.md`, `DECISIONS.md`, `SECURITY.md`, `LICENSE`, `.github/CODEOWNERS`
   - Restriction: **Pull request required**
   - Check: **Enforce for everyone**
6. Click **Create**

### Ruleset 3: Linear History

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

### Test file protection:

```bash
# This should be rejected:
echo "new rule" >> CLAUDE.md
git commit -m "fix: update rules"
git push origin main  # ❌ Rejected (must use PR)

# This should work:
git checkout -b docs/update
echo "new rule" >> CLAUDE.md
git commit -m "docs: update rules"
git push origin docs/update
# Create PR, merge
```

### Test linear history:

```bash
# On main, after merging a PR:
git log --oneline main
# Should see: —————→ (straight line)
# NOT:        \ / (merge commit)
```

---

## How Rulesets Work With Branch Protection

| Feature | Branch Protection | Ruleset | Result |
|---------|-------------------|---------|--------|
| Require PR | ✅ Yes | ✅ Yes | Both required (cumulative) |
| Status checks | ✅ Yes | ✅ Yes | Both checked |
| Linear history | ❌ No | ✅ Yes | Only ruleset enforces |
| Commit messages | ❌ No | ✅ Yes | Only ruleset enforces |
| File protection | ❌ No | ✅ Yes | Only ruleset enforces |

**No conflicts.** Rulesets add new enforcement; branch protection still handles PR reviews.

---

## Troubleshooting

### "Commit rejected: message doesn't match pattern"

Your commit message doesn't follow conventional commits. Fix it:

```bash
git commit --amend -m "feat: your message here"
git push --force-with-lease origin your-branch
```

### "Can't push to main: file changes require PR"

You're trying to edit a protected file directly. Use a PR instead:

```bash
git checkout -b fix/update-claude
# Edit CLAUDE.md
git commit -m "docs: clarify encryption rules"
git push origin fix/update-claude
# Create PR on GitHub
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

1. ✅ Create ruleset 1: Conventional commits (no bypass needed)
2. ✅ Create ruleset 2: File protection (admin bypass for emergencies)
3. ✅ Create ruleset 3: Linear history (admin bypass)
4. Test each ruleset
5. Document in team wiki/handbook

Then your repo enforces:
- ✅ Clean commit messages
- ✅ Protected architectural docs
- ✅ Linear, readable history
- ✅ Code reviews (via branch protection)
- ✅ Status checks (via branch protection)

**All committed to code quality. ✨**
