-- AI token tracking (T-069).
--
-- Three new columns on `transactions` for per-call usage feedback. Populated
-- on commit_proposal from the Anthropic API response `usage` object passed
-- back through MessageResponse::Proposal:
--   - input_tokens          → ai_input_tokens
--   - output_tokens         → ai_output_tokens
--   - cache_read_input_tokens > 0 → ai_cache_hit = 1
--
-- Used for spend tracking, prompt-layer cost analysis, and a future
-- "tokens drifted >1800/req" alert.

ALTER TABLE transactions ADD COLUMN ai_input_tokens  INTEGER;
ALTER TABLE transactions ADD COLUMN ai_output_tokens INTEGER;
ALTER TABLE transactions ADD COLUMN ai_cache_hit     INTEGER NOT NULL DEFAULT 0;
