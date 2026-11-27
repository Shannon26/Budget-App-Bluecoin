PRAGMA foreign_keys = ON;

-- Household membership. The two seeded members let a fresh database use the
-- Command Center immediately; production sign-in is a separate deployment
-- decision. Existing rows intentionally have no owner until a designated
-- member confirms assignments.
CREATE TABLE IF NOT EXISTS members (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 80),
    is_designated INTEGER NOT NULL DEFAULT 0 CHECK (is_designated IN (0, 1)),
    created_at TEXT NOT NULL
);

INSERT OR IGNORE INTO members (id, name, is_designated, created_at) VALUES
    (1, 'Member A', 1, '2026-01-01T00:00:00Z'),
    (2, 'Member B', 0, '2026-01-01T00:00:00Z');

ALTER TABLE accounts ADD COLUMN owner_member_id INTEGER REFERENCES members(id);
CREATE INDEX IF NOT EXISTS idx_accounts_owner_member_id ON accounts(owner_member_id);

-- Proposals are reviewed before a designated member confirms them atomically.
CREATE TABLE IF NOT EXISTS owner_assignments (
    account_id INTEGER PRIMARY KEY REFERENCES accounts(id),
    proposed_owner_member_id INTEGER REFERENCES members(id),
    confirmed_owner_member_id INTEGER REFERENCES members(id),
    updated_at TEXT NOT NULL
);
