PRAGMA foreign_keys = ON;

CREATE TABLE labels (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    name_key TEXT NOT NULL UNIQUE
);

CREATE TABLE entry_labels (
    entry_id INTEGER NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    label_id INTEGER NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
    PRIMARY KEY (entry_id, label_id)
);

CREATE INDEX idx_entry_labels_label ON entry_labels(label_id);

CREATE TABLE transfer_labels (
    transfer_id INTEGER NOT NULL REFERENCES transfers(id) ON DELETE CASCADE,
    label_id INTEGER NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
    PRIMARY KEY (transfer_id, label_id)
);

CREATE INDEX idx_transfer_labels_label ON transfer_labels(label_id);
