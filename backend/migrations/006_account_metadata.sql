PRAGMA foreign_keys = ON;

ALTER TABLE accounts ADD COLUMN note TEXT NOT NULL DEFAULT '';
ALTER TABLE accounts ADD COLUMN opening_date TEXT;
ALTER TABLE accounts
    ADD COLUMN hide_from_selection INTEGER NOT NULL DEFAULT 0
    CHECK (hide_from_selection IN (0, 1));
ALTER TABLE accounts
    ADD COLUMN hide_from_reports INTEGER NOT NULL DEFAULT 0
    CHECK (hide_from_reports IN (0, 1));
