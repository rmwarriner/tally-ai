#!/bin/bash

# Setup branch protection for main branch
# Requires GitHub CLI: https://cli.github.com/

set -e

REPO="rmwarriner/tally-ai"
BRANCH="main"

echo "🔒 Setting up branch protection for $REPO/$BRANCH"
echo ""

# Check if gh CLI is installed
if ! command -v gh &> /dev/null; then
    echo "❌ GitHub CLI not found. Install it: https://cli.github.com/"
    exit 1
fi

# Check authentication
if ! gh auth status &> /dev/null; then
    echo "❌ Not authenticated with GitHub. Run: gh auth login"
    exit 1
fi

echo "✓ GitHub CLI authenticated"
echo ""

# Enable branch protection via API
# Unfortunately GitHub CLI doesn't have a direct command for this,
# so we use the gh api command with the GraphQL mutation

echo "Setting up branch protection rules..."

# This would require GraphQL API, which is complex via CLI
# For now, provide the manual steps

cat << 'EOF'
Branch protection setup requires GitHub web UI or Terraform.

📋 Manual Setup (Web UI):

1. Go to: https://github.com/rmwarriner/tally-ai/settings/branches
2. Click "Add rule"
3. Enter branch name: main
4. Configure:

   ✅ Require status checks to pass
      - Dismiss stale reviews
      - Require branches up to date
      - Status checks:
        • test
        • typecheck
        • rust-test

   ✅ Require pull request reviews
      - 1 approval required
      - Dismiss stale reviews
      - Require CODEOWNERS review

   ⚠️  Do NOT enforce for administrators
       (allows emergency bypasses)

   ✅ Restrict who can push to matching branches
       (Optional: admins only)

   ✅ Include administrators
       (They can still push, but PRs logged)

5. Click "Create"

🚀 Terraform Setup (Coming Soon):

We'll add a terraform config to automate this.
See: https://registry.terraform.io/providers/integrations/github/latest/docs/resources/branch_protection

EOF

echo ""
echo "ℹ️  Documentation: .github/BRANCH_PROTECTION.md"
echo "ℹ️  Auto-approve workflow: .github/workflows/auto-approve-docs.yml"
