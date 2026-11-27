PRAGMA foreign_keys = ON;

-- Cross-member transfer requests. Ledger and report queries read only posted
-- transfers, so pending requests have no balance or total effect.
CREATE TABLE IF NOT EXISTS transfer_requests (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_account_id INTEGER NOT NULL REFERENCES accounts(id),
    destination_account_id INTEGER NOT NULL REFERENCES accounts(id),
    amount_minor INTEGER NOT NULL CHECK (amount_minor > 0),
    name TEXT NOT NULL DEFAULT '',
    note TEXT NOT NULL DEFAULT '',
    requested_date TEXT NOT NULL,
    sender_member_id INTEGER NOT NULL REFERENCES members(id),
    recipient_member_id INTEGER NOT NULL REFERENCES members(id),
    status TEXT NOT NULL CHECK (status IN ('pending', 'approved', 'declined', 'canceled')),
    source_batch_id TEXT,
    original_transfer_id INTEGER REFERENCES transfers(id),
    created_at TEXT NOT NULL,
    decided_at TEXT,
    decided_by_member_id INTEGER REFERENCES members(id),
    posted_transfer_id INTEGER REFERENCES transfers(id),
    CHECK (source_account_id <> destination_account_id),
    CHECK (sender_member_id <> recipient_member_id)
);

CREATE INDEX IF NOT EXISTS idx_transfer_requests_sender_status
    ON transfer_requests(sender_member_id, status);
CREATE INDEX IF NOT EXISTS idx_transfer_requests_recipient_status
    ON transfer_requests(recipient_member_id, status);

-- Only one approved reversal may exist for an original transfer.
CREATE UNIQUE INDEX IF NOT EXISTS idx_transfer_requests_single_approved_reversal
    ON transfer_requests(original_transfer_id)
    WHERE original_transfer_id IS NOT NULL AND status = 'approved';
