-- Chat message persistence (T-045)
-- Stores every message rendered in the chat thread so history survives restart.
-- Payload is JSON matching the discriminated union in `src/components/chat/chatTypes.ts`;
-- kind-specific fields live inside payload rather than as columns so the schema
-- stays stable as new message variants are added.

CREATE TABLE chat_messages (
    id           TEXT PRIMARY KEY NOT NULL,  -- ULID, also the client-side message id
    household_id TEXT NOT NULL REFERENCES households(id),
    kind         TEXT NOT NULL CHECK(kind IN (
                   'user','ai','proactive','system',
                   'transaction','artifact','setup_card','handoff'
                 )),
    payload      TEXT NOT NULL,              -- JSON; shape matches ChatMessage variant for `kind`
    ts           INTEGER NOT NULL,           -- unix ms; the user-visible timestamp on the bubble
    created_at   INTEGER NOT NULL            -- unix ms; insert time (may differ from ts for backfills)
);

CREATE INDEX idx_chat_messages_household_ts ON chat_messages(household_id, ts);
CREATE INDEX idx_chat_messages_household_created ON chat_messages(household_id, created_at);
