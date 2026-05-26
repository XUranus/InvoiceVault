-- Add UUID column to agent_sessions
ALTER TABLE agent_sessions ADD COLUMN uuid TEXT NOT NULL DEFAULT '';

-- Backfill existing sessions with UUIDs
UPDATE agent_sessions SET uuid = lower(hex(randomblob(4))) || '-' ||
    lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' ||
    substr('89ab',abs(random())%4+1,1) || substr(lower(hex(randomblob(2))),2) || '-' ||
    lower(hex(randomblob(6)))
WHERE uuid = '';
