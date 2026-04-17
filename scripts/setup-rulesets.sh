#!/bin/bash

# Setup GitHub Rulesets for tally-ai
# Provides options for Web UI, GraphQL API, or Terraform

set -e

REPO="rmwarriner/tally-ai"

cat << 'EOF'
╔════════════════════════════════════════════════════════════════╗
║          GitHub Rulesets Setup for tally-ai                   ║
║                                                                ║
║  Creating 2 rulesets for public repos:                        ║
║  1. Conventional Commits (required format)                    ║
║  2. Linear History (clean git log)                            ║
║                                                                ║
║  Note: File protection uses CODEOWNERS + branch protection   ║
║        (push rules unavailable for public repos)              ║
║                                                                ║
║  Setup options:                                               ║
║  1. Web UI (easiest, manual)                                   ║
║  2. Terraform (infrastructure-as-code)                         ║
╚════════════════════════════════════════════════════════════════╝

Choose your method:
EOF

echo ""
echo "1) Web UI Setup"
echo "2) Terraform"
echo "0) Exit"
echo ""
read -p "Choose (0-2): " choice

case $choice in
  1)
    cat << 'WEBUI'

📋 WEB UI SETUP

Go to: https://github.com/rmwarriner/tally-ai/settings/rules/rulesets

Create two rulesets:

1️⃣ CONVENTIONAL COMMITS
   Name: Enforce Conventional Commits
   Target: All branches
   Rule: Commit message pattern
   Pattern: ^(feat|fix|test|docs|chore|refactor|perf|style|ci)(\(.+\))?!?: .+
   Enforcement: Active
   Bypass: None

2️⃣ LINEAR HISTORY
   Name: Linear History for main
   Target: main branch only
   Rules:
     - Required linear history
     - Required pull request (1 approval)
     - Require status checks: test, typecheck, rust-test
   Enforcement: Active
   Bypass: None

📌 File Protection:
   Use CODEOWNERS + branch protection instead (already configured)
   See: .github/BRANCH_PROTECTION_SETUP.md

Full details: .github/RULESETS.md

WEBUI
    ;;

  2)
    cat << 'TERRAFORM'

🏗️ TERRAFORM SETUP

Requires:
  - Terraform ≥ 1.0
  - GitHub provider ≥ 6.0
  - GitHub token with admin:repo_hook scope

Setup:

1. Initialize Terraform:
   cd terraform
   terraform init

2. Set your GitHub token:
   export GITHUB_TOKEN=$(gh auth token)

3. Plan the rulesets:
   terraform plan -target=github_repository_ruleset.conventional_commits

4. Apply:
   terraform apply -target=github_repository_ruleset.conventional_commits
   terraform apply -target=github_repository_ruleset.protect_critical_files
   terraform apply -target=github_repository_ruleset.linear_history

Or apply all at once:
   terraform apply

Verify:
   terraform show

Destroy (if needed):
   terraform destroy

Full config: terraform/rulesets.tf

TERRAFORM
    ;;

  0)
    echo "Exiting."
    exit 0
    ;;

  *)
    echo "Invalid choice."
    exit 1
    ;;
esac

echo ""
echo "📖 Full Documentation: .github/RULESETS.md"
echo "🔒 File Protection: .github/BRANCH_PROTECTION_SETUP.md (CODEOWNERS)"
echo "✅ Don't forget to test your rulesets after creation!"
