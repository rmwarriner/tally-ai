# Branch Protection Configuration

This document describes the recommended branch protection settings for the `main` branch.

## How to Apply

1. Go to your repository **Settings** → **Branches**
2. Under **Branch protection rules**, click **Add rule**
3. Enter `main` as the branch name pattern
4. Apply the settings below

## Recommended Settings

### Status Checks

- ✅ **Require status checks to pass before merging**
  - ✅ Require branches to be up to date before merging
  - Select these checks:
    - `test` (Node.js tests)
    - `typecheck` (TypeScript type checking)
    - `rust-test` (Rust tests and clippy)

### Pull Request Reviews

- ✅ **Require pull request reviews before merging**
  - Approvals required: **1**
  - Dismiss stale pull request approvals when new commits are pushed
  - Require code review from Code Owners (optional, if you set up CODEOWNERS)

### Enforce Administrators

- ✅ **Enforce all the above rules for administrators too**

### Dismiss Stale Reviews

- ✅ **Dismiss stale pull request approvals when new commits are pushed**

### Require CODEOWNERS Review

- Optional: Set up [CODEOWNERS](.github/CODEOWNERS) and require their review for matching files

---

## Optional Enhancements

- **Require commit messages to follow conventional commit format** — Use GitHub Actions for this if needed
- **Require signed commits** — If you enforce GPG or S/MIME signing
- **Restrict push to administrators** — If you want to enforce PR-only merging strictly
- **Automatically delete head branches** — Clean up PR branches after merge

## Automation

These settings can be managed programmatically via:
- GitHub API (REST or GraphQL)
- Terraform (with `github` provider)
- GitHub CLI (`gh api`)

Example with GitHub CLI:
```bash
gh repo edit --enable-auto-merge --enable-delete-branch-on-merge
```
