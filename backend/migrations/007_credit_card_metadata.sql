PRAGMA foreign_keys = ON;

ALTER TABLE accounts
    ADD COLUMN credit_limit_minor INTEGER
    CHECK (credit_limit_minor IS NULL OR credit_limit_minor > 0);
ALTER TABLE accounts
    ADD COLUMN cutoff_day INTEGER
    CHECK (cutoff_day IS NULL OR cutoff_day BETWEEN 1 AND 31);
ALTER TABLE accounts
    ADD COLUMN due_day INTEGER
    CHECK (due_day IS NULL OR due_day BETWEEN 1 AND 31);
