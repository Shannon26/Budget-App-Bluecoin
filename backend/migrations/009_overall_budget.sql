PRAGMA foreign_keys = ON;

CREATE TABLE overall_budgets (
    month TEXT NOT NULL PRIMARY KEY CHECK (month GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]'),
    amount_minor INTEGER NOT NULL CHECK (amount_minor > 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
