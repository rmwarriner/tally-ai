// Proactive engine triggers (T-050–T-052).
//
// Each submodule produces `InsightDraft` values for a specific event class.
// The caller (commands::commit_proposal, ai::orchestrator, commands::session_open)
// is responsible for sensitivity gating + dedup persistence via
// `core::insight::should_emit` and `log_if_new`.

pub mod briefing;
pub mod duplicate;
pub mod envelope;
