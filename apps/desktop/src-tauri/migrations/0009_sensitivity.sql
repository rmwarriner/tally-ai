-- Sensitivity setting (T-054).
--
-- Per-household gate that determines which proactive engine kinds surface
-- to the user. core::insight::should_emit reads this; producers MUST check
-- before emitting, since dedup happens after gating.
--
-- 'normal' is the safe default — hard alerts only, no morning briefing or
-- approaching-limit insights.

ALTER TABLE households ADD COLUMN sensitivity TEXT NOT NULL DEFAULT 'normal';

-- Tracks the most recent household-local day on which a morning briefing
-- was emitted; session_open compares against today's day_bucket to gate
-- the trigger. Stored as unix-ms of household-local midnight, same shape
-- as insight_log.day_bucket.
ALTER TABLE households ADD COLUMN last_briefing_at_day INTEGER;
