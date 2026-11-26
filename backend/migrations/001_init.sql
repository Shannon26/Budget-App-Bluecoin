PRAGMA foreign_keys = ON;

CREATE TABLE accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 80),
    currency TEXT NOT NULL CHECK (currency GLOB '[A-Z][A-Z][A-Z]'),
    opening_balance_minor INTEGER NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE transfers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    from_account_id INTEGER NOT NULL REFERENCES accounts(id),
    to_account_id INTEGER NOT NULL REFERENCES accounts(id),
    amount_minor INTEGER NOT NULL CHECK (amount_minor > 0),
    note TEXT NOT NULL DEFAULT '',
    occurred_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    CHECK (from_account_id <> to_account_id)
);

CREATE TABLE entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL CHECK (kind IN ('income', 'expense', 'transfer_in', 'transfer_out')),
    account_id INTEGER NOT NULL REFERENCES accounts(id),
    amount_minor INTEGER NOT NULL CHECK (amount_minor > 0),
    note TEXT NOT NULL DEFAULT '',
    occurred_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    transfer_id INTEGER REFERENCES transfers(id) ON DELETE CASCADE,
    CHECK (
        (kind IN ('income', 'expense') AND transfer_id IS NULL)
        OR
        (kind IN ('transfer_in', 'transfer_out') AND transfer_id IS NOT NULL)
    )
);

CREATE INDEX idx_entries_account_id ON entries(account_id);
CREATE INDEX idx_entries_transfer_id ON entries(transfer_id);
CREATE INDEX idx_entries_occurred_at ON entries(occurred_at);
CREATE INDEX idx_transfers_occurred_at ON transfers(occurred_at);
