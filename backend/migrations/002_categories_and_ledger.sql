PRAGMA foreign_keys = ON;

CREATE TABLE categories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 80),
    kind TEXT NOT NULL CHECK (kind IN ('income', 'expense')),
    parent_id INTEGER REFERENCES categories(id) ON DELETE SET NULL,
    color TEXT NOT NULL DEFAULT '#426b8a',
    icon TEXT NOT NULL DEFAULT '•',
    sort_order INTEGER NOT NULL DEFAULT 0,
    archived_at TEXT,
    created_at TEXT NOT NULL
);

ALTER TABLE entries ADD COLUMN name TEXT NOT NULL DEFAULT '';
ALTER TABLE entries ADD COLUMN category_id INTEGER REFERENCES categories(id);
ALTER TABLE entries ADD COLUMN reconciled INTEGER NOT NULL DEFAULT 0;

UPDATE entries SET name = note WHERE name = '';

ALTER TABLE transfers ADD COLUMN name TEXT NOT NULL DEFAULT '';
ALTER TABLE transfers ADD COLUMN reconciled INTEGER NOT NULL DEFAULT 0;
UPDATE transfers SET name = note WHERE name = '';

CREATE TABLE saved_filters (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 80),
    definition_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_categories_kind_parent ON categories(kind, parent_id, sort_order);
CREATE INDEX idx_entries_category_id ON entries(category_id);
CREATE INDEX idx_entries_name ON entries(name);
CREATE INDEX idx_entries_reconciled ON entries(reconciled);

-- Expense parents
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at) VALUES
('Food & Drink', 'expense', NULL, '#b65d35', '🍽', 10, '2026-01-01T00:00:00Z'),
('Transport', 'expense', NULL, '#426b8a', '🚗', 20, '2026-01-01T00:00:00Z'),
('Home & Utilities', 'expense', NULL, '#7b5a86', '🏠', 30, '2026-01-01T00:00:00Z'),
('Shopping', 'expense', NULL, '#c28a2c', '🛍', 40, '2026-01-01T00:00:00Z'),
('Health & Fitness', 'expense', NULL, '#23735b', '❤', 50, '2026-01-01T00:00:00Z'),
('Entertainment', 'expense', NULL, '#8b5f9e', '🎬', 60, '2026-01-01T00:00:00Z'),
('Travel', 'expense', NULL, '#3d7f8f', '✈', 70, '2026-01-01T00:00:00Z'),
('Family & Education', 'expense', NULL, '#a56b45', '🎓', 80, '2026-01-01T00:00:00Z'),
('Financial & Taxes', 'expense', NULL, '#5d6d7e', '₹', 90, '2026-01-01T00:00:00Z'),
('Other Expenses', 'expense', NULL, '#6f7b73', '•••', 100, '2026-01-01T00:00:00Z');

-- Expense children
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Dining Out', 'expense', id, '#b65d35', '🍴', 11, '2026-01-01T00:00:00Z' FROM categories WHERE name='Food & Drink' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Groceries', 'expense', id, '#b65d35', '🛒', 12, '2026-01-01T00:00:00Z' FROM categories WHERE name='Food & Drink' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Coffee & Tea', 'expense', id, '#b65d35', '☕', 13, '2026-01-01T00:00:00Z' FROM categories WHERE name='Food & Drink' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Food Delivery', 'expense', id, '#b65d35', '🥡', 14, '2026-01-01T00:00:00Z' FROM categories WHERE name='Food & Drink' AND kind='expense';

INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Fuel', 'expense', id, '#426b8a', '⛽', 21, '2026-01-01T00:00:00Z' FROM categories WHERE name='Transport' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Public Transport', 'expense', id, '#426b8a', '🚌', 22, '2026-01-01T00:00:00Z' FROM categories WHERE name='Transport' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Taxi & Ride Share', 'expense', id, '#426b8a', '🚕', 23, '2026-01-01T00:00:00Z' FROM categories WHERE name='Transport' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Vehicle Maintenance', 'expense', id, '#426b8a', '🔧', 24, '2026-01-01T00:00:00Z' FROM categories WHERE name='Transport' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Parking & Tolls', 'expense', id, '#426b8a', '🅿', 25, '2026-01-01T00:00:00Z' FROM categories WHERE name='Transport' AND kind='expense';

INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Rent', 'expense', id, '#7b5a86', '🏠', 31, '2026-01-01T00:00:00Z' FROM categories WHERE name='Home & Utilities' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Electricity', 'expense', id, '#7b5a86', '💡', 32, '2026-01-01T00:00:00Z' FROM categories WHERE name='Home & Utilities' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Water', 'expense', id, '#7b5a86', '💧', 33, '2026-01-01T00:00:00Z' FROM categories WHERE name='Home & Utilities' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Internet & Phone', 'expense', id, '#7b5a86', '📶', 34, '2026-01-01T00:00:00Z' FROM categories WHERE name='Home & Utilities' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Household Supplies', 'expense', id, '#7b5a86', '🧴', 35, '2026-01-01T00:00:00Z' FROM categories WHERE name='Home & Utilities' AND kind='expense';

INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Clothing', 'expense', id, '#c28a2c', '👕', 41, '2026-01-01T00:00:00Z' FROM categories WHERE name='Shopping' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Electronics', 'expense', id, '#c28a2c', '💻', 42, '2026-01-01T00:00:00Z' FROM categories WHERE name='Shopping' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Personal Care', 'expense', id, '#c28a2c', '🧼', 43, '2026-01-01T00:00:00Z' FROM categories WHERE name='Shopping' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Gifts & Celebrations', 'expense', id, '#c28a2c', '🎁', 44, '2026-01-01T00:00:00Z' FROM categories WHERE name='Shopping' AND kind='expense';

INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Doctor & Hospital', 'expense', id, '#23735b', '🩺', 51, '2026-01-01T00:00:00Z' FROM categories WHERE name='Health & Fitness' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Medicines', 'expense', id, '#23735b', '💊', 52, '2026-01-01T00:00:00Z' FROM categories WHERE name='Health & Fitness' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Gym & Fitness', 'expense', id, '#23735b', '🏋', 53, '2026-01-01T00:00:00Z' FROM categories WHERE name='Health & Fitness' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Insurance', 'expense', id, '#23735b', '🛡', 54, '2026-01-01T00:00:00Z' FROM categories WHERE name='Health & Fitness' AND kind='expense';

INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Movies & Events', 'expense', id, '#8b5f9e', '🎬', 61, '2026-01-01T00:00:00Z' FROM categories WHERE name='Entertainment' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Subscriptions', 'expense', id, '#8b5f9e', '📺', 62, '2026-01-01T00:00:00Z' FROM categories WHERE name='Entertainment' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Hobbies', 'expense', id, '#8b5f9e', '🎨', 63, '2026-01-01T00:00:00Z' FROM categories WHERE name='Entertainment' AND kind='expense';

INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Flights', 'expense', id, '#3d7f8f', '✈', 71, '2026-01-01T00:00:00Z' FROM categories WHERE name='Travel' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Hotels & Stays', 'expense', id, '#3d7f8f', '🏨', 72, '2026-01-01T00:00:00Z' FROM categories WHERE name='Travel' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Vacation Activities', 'expense', id, '#3d7f8f', '🏖', 73, '2026-01-01T00:00:00Z' FROM categories WHERE name='Travel' AND kind='expense';

INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Childcare', 'expense', id, '#a56b45', '🧸', 81, '2026-01-01T00:00:00Z' FROM categories WHERE name='Family & Education' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'School & Tuition', 'expense', id, '#a56b45', '🏫', 82, '2026-01-01T00:00:00Z' FROM categories WHERE name='Family & Education' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Books & Courses', 'expense', id, '#a56b45', '📚', 83, '2026-01-01T00:00:00Z' FROM categories WHERE name='Family & Education' AND kind='expense';

INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Bank Fees', 'expense', id, '#5d6d7e', '🏦', 91, '2026-01-01T00:00:00Z' FROM categories WHERE name='Financial & Taxes' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Interest Paid', 'expense', id, '#5d6d7e', '%', 92, '2026-01-01T00:00:00Z' FROM categories WHERE name='Financial & Taxes' AND kind='expense';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Taxes', 'expense', id, '#5d6d7e', '₹', 93, '2026-01-01T00:00:00Z' FROM categories WHERE name='Financial & Taxes' AND kind='expense';

INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'General Expenses', 'expense', id, '#6f7b73', '•••', 101, '2026-01-01T00:00:00Z' FROM categories WHERE name='Other Expenses' AND kind='expense';

-- Income parents
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at) VALUES
('Employment', 'income', NULL, '#23735b', '💼', 10, '2026-01-01T00:00:00Z'),
('Business & Freelance', 'income', NULL, '#426b8a', '🏢', 20, '2026-01-01T00:00:00Z'),
('Investments', 'income', NULL, '#7b5a86', '📈', 30, '2026-01-01T00:00:00Z'),
('Property', 'income', NULL, '#a56b45', '🏠', 40, '2026-01-01T00:00:00Z'),
('Other Income', 'income', NULL, '#5d7b68', '•••', 50, '2026-01-01T00:00:00Z');

-- Income children
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Salary', 'income', id, '#23735b', '💰', 11, '2026-01-01T00:00:00Z' FROM categories WHERE name='Employment' AND kind='income';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Bonus', 'income', id, '#23735b', '🎉', 12, '2026-01-01T00:00:00Z' FROM categories WHERE name='Employment' AND kind='income';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Commission', 'income', id, '#23735b', '%', 13, '2026-01-01T00:00:00Z' FROM categories WHERE name='Employment' AND kind='income';

INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Client Payments', 'income', id, '#426b8a', '🧾', 21, '2026-01-01T00:00:00Z' FROM categories WHERE name='Business & Freelance' AND kind='income';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Sales', 'income', id, '#426b8a', '🛍', 22, '2026-01-01T00:00:00Z' FROM categories WHERE name='Business & Freelance' AND kind='income';

INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Dividends', 'income', id, '#7b5a86', '📈', 31, '2026-01-01T00:00:00Z' FROM categories WHERE name='Investments' AND kind='income';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Interest Income', 'income', id, '#7b5a86', '%', 32, '2026-01-01T00:00:00Z' FROM categories WHERE name='Investments' AND kind='income';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Capital Gains', 'income', id, '#7b5a86', '📊', 33, '2026-01-01T00:00:00Z' FROM categories WHERE name='Investments' AND kind='income';

INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Rent Received', 'income', id, '#a56b45', '🏠', 41, '2026-01-01T00:00:00Z' FROM categories WHERE name='Property' AND kind='income';

INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Gifts Received', 'income', id, '#5d7b68', '🎁', 51, '2026-01-01T00:00:00Z' FROM categories WHERE name='Other Income' AND kind='income';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Refunds', 'income', id, '#5d7b68', '↩', 52, '2026-01-01T00:00:00Z' FROM categories WHERE name='Other Income' AND kind='income';
INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, created_at)
SELECT 'Miscellaneous Income', 'income', id, '#5d7b68', '•••', 53, '2026-01-01T00:00:00Z' FROM categories WHERE name='Other Income' AND kind='income';
