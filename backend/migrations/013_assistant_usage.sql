PRAGMA foreign_keys = ON;

-- Local operational accounting only. Submission IDs are hashed before they
-- reach this table; prompts, model output, credentials, and headers never do.
CREATE TABLE assistant_usage (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    member_id INTEGER NOT NULL REFERENCES members(id),
    submission_key TEXT NOT NULL,
    generation_id TEXT,
    status TEXT NOT NULL CHECK (status IN ('pending', 'completed', 'unavailable', 'duplicate')),
    model TEXT NOT NULL,
    provider TEXT NOT NULL,
    cost_reported INTEGER NOT NULL DEFAULT 0 CHECK (cost_reported IN (0, 1)),
    cost_usd TEXT,
    created_at TEXT NOT NULL,
    completed_at TEXT,
    UNIQUE (member_id, submission_key),
    CHECK ((cost_reported = 1 AND cost_usd IS NOT NULL) OR (cost_reported = 0 AND cost_usd IS NULL))
);

CREATE UNIQUE INDEX idx_assistant_usage_generation
    ON assistant_usage(generation_id)
    WHERE generation_id IS NOT NULL;
CREATE INDEX idx_assistant_usage_member_created
    ON assistant_usage(member_id, created_at DESC, id DESC);
