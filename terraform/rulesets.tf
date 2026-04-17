# GitHub Rulesets for tally-ai
# Apply with: terraform apply -target=github_repository_ruleset.conventional_commits

terraform {
  required_version = ">= 1.0"
  required_providers {
    github = {
      source  = "integrations/github"
      version = "~> 6.0"
    }
  }
}

variable "repository" {
  description = "Repository name"
  type        = string
  default     = "tally-ai"
}

variable "owner" {
  description = "Repository owner"
  type        = string
  default     = "rmwarriner"
}

data "github_repository" "main" {
  name  = var.repository
  owner = var.owner
}

# Ruleset 1: Enforce Conventional Commits
resource "github_repository_ruleset" "conventional_commits" {
  repository = data.github_repository.main.name
  name       = "Enforce Conventional Commits"
  target     = "branch"
  enforcement = "active"

  conditions {
    ref_name {
      include = ["refs/heads/*"]
      exclude = []
    }
  }

  rules {
    commit_message_pattern {
      operator = "starts_with"
      # Matches: feat:, fix:, test:, docs:, chore:, refactor:, perf:, style:, ci:
      # With optional scope: feat(db):
      pattern = "^(feat|fix|test|docs|chore|refactor|perf|style|ci)(\\(.+\\))?!?: .+"
    }
  }

  # Enforce for everyone (no bypasses)
  bypass_actors = []
}

# Ruleset 2: Protect Critical Files
resource "github_repository_ruleset" "protect_critical_files" {
  repository = data.github_repository.main.name
  name       = "Protect Critical Files"
  target     = "branch"
  enforcement = "active"

  conditions {
    ref_name {
      include = ["refs/heads/*"]
      exclude = []
    }
  }

  rules {
    # Files that require PR for changes
    file_path_restriction {
      restricted_file_paths = [
        "CLAUDE.md",
        "DECISIONS.md",
        "SECURITY.md",
        "LICENSE",
        ".github/CODEOWNERS"
      ]
    }
  }

  # Enforce for everyone (no bypasses)
  bypass_actors = []
}

# Ruleset 3: Linear History for main
resource "github_repository_ruleset" "linear_history" {
  repository = data.github_repository.main.name
  name       = "Linear History for main"
  target     = "branch"
  enforcement = "active"

  conditions {
    ref_name {
      include = ["refs/heads/main"]
      exclude = []
    }
  }

  rules {
    # Require squash/rebase only (no merge commits)
    required_linear_history = true

    # Require PR with 1 approval
    pull_request {
      required_approving_review_count   = 1
      dismiss_stale_reviews_on_push     = true
      require_code_owner_reviews        = false
      require_last_push_approval        = false
      require_deployment_environments   = false
    }

    # Require status checks to pass
    required_status_checks {
      required_check {
        context        = "test"
        integration_id = null
      }
      required_check {
        context        = "typecheck"
        integration_id = null
      }
      required_check {
        context        = "rust-test"
        integration_id = null
      }
      strict_required_status_checks_policy = true
    }

    # Require up-to-date branches
    update_allows_fetch_and_merge = true
  }

  # Enforce for everyone (no bypasses)
  bypass_actors = []
}

# Outputs
output "conventional_commits_ruleset_id" {
  value = github_repository_ruleset.conventional_commits.id
}

output "protect_critical_files_ruleset_id" {
  value = github_repository_ruleset.protect_critical_files.id
}

output "linear_history_ruleset_id" {
  value = github_repository_ruleset.linear_history.id
}
