# Branch Protection Setup

Tally.ai requires all changes to `main` to go through code review, except for documentation and config updates which are auto-approved.

## The Policy

| Change Type | Requires PR | Requires Review | Auto-Approved |
|------------|------------|-----------------|---------------|
| Code (.ts, .tsx, .rs) | ✅ Yes | ✅ Yes (1 approval) | ❌ No |
| Docs (.md) | ✅ Yes | ❌ No | ✅ Yes (auto-approved) |
| Config (.github/*, .npmrc, etc.) | ✅ Yes | ❌ No | ✅ Yes (auto-approved) |

**In practice:**
- Code changes → PR → Code review required → Merge
- Doc updates → PR → Auto-approved → Merge (no human wait)
- Package bumps → PR → Auto-approved → Merge

## Setup Instructions

### Step 1: Enable Branch Protection (Web UI)

1. Go to **Settings** → **Branches**
2. Click **Add rule**
3. Enter branch name: `main`

### Step 2: Configure the Rule

Copy these exact settings:

#### ✅ Require status checks to pass before merging
- Check: **Require branches to be up to date before merging**
- Status checks required:
  - `test`
  - `typecheck`
  - `rust-test`

#### ✅ Require pull request reviews before merging
- Required number of approvals: **1**
- Check: **Dismiss stale pull request approvals when new commits are pushed**
- Check: **Require code review from Code Owners**
  - (This enforces [.github/CODEOWNERS](.github/CODEOWNERS) reviews)

#### ⚠️ Enforce all the above settings for administrators too
- **Uncheck this**
  - Allows admin to push hotfixes in emergencies
  - Commits are still logged and auditable
  - Maintain accountability while keeping agility

#### ✅ Restrict who can push to matching branches
- (Optional) Allow only: admins, specific users, or teams

#### ✅ Include administrators
- Check: **Allow force pushes** → **No** (don't allow anyone to force push)
- Check: **Allow deletions** → **No** (don't allow branch deletion)

#### ✅ Automatically delete head branches
- Check: **Automatically delete head branches**
  - Cleans up PR branches after merge

### Step 3: Save

Click **Create** to enable the rule.

---

## Auto-Approve Workflow

**File:** `.github/workflows/auto-approve-docs.yml`

This workflow automatically approves PRs that only touch:

```
✅ Auto-approved files:
  - *.md (all markdown)
  - .github/** (workflows, templates, configs)
  - docs/**
  - .npmrc, .gitignore, .prettierrc*, .eslintrc*, tsconfig*.json
  - package.json, pnpm-lock.yaml, Cargo.lock
  - .husky/**, LICENSE

❌ Blocks auto-approval if any of these change:
  - apps/desktop/src/** (React)
  - apps/desktop/src-tauri/** (Rust)
  - packages/** (TypeScript)
  - Any .ts, .tsx, .rs file
```

**What happens:**
1. PR opens
2. Workflow checks changed files
3. If docs-only: ✅ auto-approves + labels `documentation`
4. If code-touching: ⏳ requires human review

---

## Verification

### Check if protection is enabled:

```bash
# View rule
gh repo view rmwarriner/tally-ai --json branchProtectionRules --jq '.branchProtectionRules'

# Or visit:
https://github.com/rmwarriner/tally-ai/settings/branches
```

### Test the auto-approve:

1. Create a PR that only edits `README.md`
2. You should see: **✅ Auto-approved: docs/config only**
3. The PR can be merged immediately (no human review needed)

### Test code protection:

1. Create a PR that touches `apps/desktop/src/App.tsx`
2. The PR will be blocked until someone approves it
3. Status check will show you need 1 approval

---

## Emergency Bypasses

If you need to bypass protection (critical hotfix):

```bash
# As admin, force-push (NOT RECOMMENDED)
git push --force-with-lease origin main:main
```

**⚠️ This is logged and auditable.** Use only for true emergencies, then document why in the issue.

Better approach:
1. Open PR immediately (even mid-development)
2. Request emergency review from team
3. Merge when approved

---

## Terraform Automation (Optional)

To manage branch protection as code:

```hcl
resource "github_branch_protection_v3" "main" {
  repository     = "tally-ai"
  branch         = "main"
  enforce_admins = false

  required_status_checks {
    strict   = true
    contexts = ["test", "typecheck", "rust-test"]
  }

  required_pull_request_reviews {
    required_approving_review_count = 1
    dismiss_stale_reviews           = true
    require_code_owner_reviews      = true
  }

  restrictions {
    users = ["rmwarriner"]
  }
}
```

See: https://registry.terraform.io/providers/integrations/github/latest/docs/resources/branch_protection_v3

---

## FAQ

**Q: Can I commit directly to main for urgent bugs?**
A: Only if you're an admin and branch protection allows it. But prefer opening a PR immediately and requesting emergency review.

**Q: Why auto-approve docs?**
A: Documentation and config are low-risk and change frequently. Auto-approval keeps iteration fast while protecting code.

**Q: What if my PR should be auto-approved but it's not?**
A: You changed a file not in the approved list. Either:
- Create a separate PR with only the doc changes
- Or request review in the PR (human will handle it)

**Q: Can I update the auto-approve list?**
A: Yes, edit `.github/workflows/auto-approve-docs.yml` and PR it. The patterns are in the `docPatterns` array.

**Q: What happens if status checks fail?**
A: You must fix them before merging. Re-run tests locally with `pnpm test && cargo test --all`.

---

## Checklist

- [ ] Branch protection rule created for `main`
- [ ] Status checks required: `test`, `typecheck`, `rust-test`
- [ ] PR reviews required: 1 approval
- [ ] CODEOWNERS review enabled
- [ ] Auto-approve workflow is active (visible in Actions)
- [ ] Tested with doc-only PR
- [ ] Tested with code-touching PR

Once done, your repo enforces quality while allowing fast iteration on docs.
