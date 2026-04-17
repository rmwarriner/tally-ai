#!/bin/bash

# Setup GitHub Rulesets for tally-ai
# Provides options for Web UI, GraphQL API, or Terraform

set -e

REPO="rmwarriner/tally-ai"

cat << 'EOF'
╔════════════════════════════════════════════════════════════════╗
║          GitHub Rulesets Setup for tally-ai                   ║
║                                                                ║
║  1. Web UI (easiest, manual)                                   ║
║  2. GraphQL API (programmatic)                                 ║
║  3. Terraform (infrastructure-as-code)                         ║
╚════════════════════════════════════════════════════════════════╝

Choose your method:
EOF

echo ""
echo "1) Web UI Setup"
echo "2) GraphQL API"
echo "3) Terraform"
echo "0) Exit"
echo ""
read -p "Choose (0-3): " choice

case $choice in
  1)
    cat << 'WEBUI'

📋 WEB UI SETUP

Go to: https://github.com/rmwarriner/tally-ai/settings/rules/rulesets

Create three rulesets:

1️⃣ CONVENTIONAL COMMITS
   Name: Enforce Conventional Commits
   Target: All branches
   Rule: Commit message pattern
   Pattern: ^(feat|fix|test|docs|chore|refactor|perf|style|ci)(\(.+\))?!?: .+
   Enforcement: Active
   Bypass: None

2️⃣ PROTECT CRITICAL FILES
   Name: Protect Critical Files
   Target: All branches
   Rule: File path restriction
   Files: CLAUDE.md, DECISIONS.md, SECURITY.md, LICENSE, .github/CODEOWNERS
   Enforcement: Active
   Bypass: None

3️⃣ LINEAR HISTORY
   Name: Linear History for main
   Target: main branch only
   Rules:
     - Required linear history
     - Required pull request (1 approval)
     - Require status checks: test, typecheck, rust-test
   Enforcement: Active
   Bypass: None

Full details: .github/RULESETS.md

WEBUI
    ;;

  2)
    cat << 'GRAPHQL'

🔗 GRAPHQL API SETUP

Requires: GitHub CLI (gh auth login)

The GraphQL mutations are in: .github/rulesets.graphql

However, creating rulesets via GraphQL requires:
1. Repository ID (not the name)
2. Complex mutation syntax

Easier option:
  • Use Terraform (option 3) for IaC
  • Or use Web UI (option 1) for simplicity

To get repository ID:
  gh api repos/rmwarriner/tally-ai --jq '.id'

Then modify rulesets.graphql with the ID and run:
  gh api graphql --input .github/rulesets.graphql

GRAPHQL
    ;;

  3)
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
echo "✅ Don't forget to test your rulesets after creation!"
