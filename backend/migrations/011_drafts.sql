PRAGMA foreign_keys = ON;

-- One private active draft per member. Rows carry stable member-scoped IDs so
-- assistant follow-ups can target a single row without disturbing manual edits.
CREATE TABLE IF NOT EXISTS drafts (
    member_id INTEGER PRIMARY KEY REFERENCES members(id),
    revision INTEGER NOT NULL DEFAULT 0,
    local_date TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS draft_rows (
    member_id INTEGER NOT NULL REFERENCES members(id),
    row_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('expense', 'income', 'transfer', 'new_account')),
    data_json TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'ready',
    updated_at TEXT NOT NULL,
    PRIMARY KEY (member_id, row_id)
);

CREATE TABLE IF NOT EXISTS draft_prompts (
    member_id INTEGER NOT NULL REFERENCES members(id),
    submission_id TEXT NOT NULL,
    seq INTEGER NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('member', 'assistant', 'system')),
    text TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (member_id, submission_id, seq)
);

-- Receipts survive draft clearance and make repeated Saves idempotent.
CREATE TABLE IF NOT EXISTS save_receipts (
    save_key TEXT PRIMARY KEY,
    member_id INTEGER NOT NULL REFERENCES members(id),
    revision INTEGER NOT NULL,
    result_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

-- Personal account nicknames are remembered only after an explicit choice.
CREATE TABLE IF NOT EXISTS member_aliases (
    member_id INTEGER NOT NULL REFERENCES members(id),
    alias TEXT NOT NULL,
    account_id INTEGER REFERENCES accounts(id),
    proposed_row_id TEXT,
    created_at TEXT NOT NULL,
    PRIMARY KEY (member_id, alias),
    CHECK (account_id IS NOT NULL OR proposed_row_id IS NOT NULL)
);
