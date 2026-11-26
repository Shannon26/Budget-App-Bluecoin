# Paisa — categories and transaction management

An original, local-first budgeting web app. Milestone 2 adds a seeded category catalog, date-grouped ledger, transaction detail/edit/duplicate/permanent delete, manual reconciliation, saved filters, account history, and atomic transfer management. Bluecoins remains the long-term parity reference; see [`docs/bluecoins-parity.md`](docs/bluecoins-parity.md) and [`docs/bluecoins-walkthrough.md`](docs/bluecoins-walkthrough.md).

## Stack

- **Backend:** Rust, Axum, Tokio, SQLx, bundled SQLite, SQL migrations
- **Frontend:** React, TypeScript, Vite, Playwright
- **Money:** signed 64-bit integer minor units (paise); decimal text is validated and converted exactly in the browser
- **Currency:** INR only, enforced by both UI and API until per-currency reporting exists

## Prerequisites

- Rust toolchain with Cargo
- Node.js 20+ and npm
- A C compiler for SQLx's bundled SQLite build
- `/usr/bin/google-chrome` for the repeatable browser test, or set `CHROME_PATH` to another Chromium executable

If `cargo` is not on your shell's PATH:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

## Exact startup commands

Terminal 1, from the repository root:

```bash
cd backend
cargo run
```

The API listens on **http://127.0.0.1:3000**, creates `backend/budget.db` if needed, and runs migrations automatically. To use another database:

```bash
DATABASE_URL='sqlite:///absolute/path/to/budget.db?mode=rwc' cargo run
```

Terminal 2, from the repository root:

```bash
cd frontend
npm install
npm run dev
```

Open **http://127.0.0.1:5173**. Vite proxies `/api` to `http://127.0.0.1:3000`.

Stop either process with `Ctrl+C`.

## Milestone 2 behavior

- Comprehensive editable income/expense catalog with meaningful two-level groupings.
- Optional categories with **Uncategorized** fallback; transfers remain uncategorized.
- Referenced categories archive instead of disappearing, preserving history and totals while leaving new-entry pickers.
- Date-grouped ledger with daily income, expense, and transfer totals.
- Search by name/note and filters for date, account, category, reconciliation status, amount range, and uncategorized entries.
- Named filters persist in SQLite and can be reopened after reload.
- Detail view with edit, duplicate, manual Reconciled toggle, and explicit permanent-delete confirmation.
- One combined transfer row in the main ledger; account history shows only the relevant transfer side.
- Transfer create/edit/delete updates both legs atomically and never changes income/expense totals.

## Acceptance flow

The repeatable browser test uses a clean database and verifies:

1. Create Checking with ₹5,000 and Savings with ₹1,000.
2. Record a ₹250 **Dining Out** expense and ₹1,000 **Salary** income.
3. Verify balances, income/expense totals, and category totals.
4. Edit the expense to ₹300 and verify balances/category totals update.
5. Duplicate the expense, then permanently delete the duplicate and verify prior values return.
6. Mark the expense reconciled, save a category/status filter, reload, and reopen it.
7. Record an uncategorized expense and verify the **Uncategorized** label.
8. Archive Dining Out and verify existing history remains while the new-entry picker hides it.
9. Transfer ₹500, verify one main-ledger row and correct account-history direction, edit it to ₹600, then delete it.
10. Verify both accounts update together and income/expense totals remain unchanged.

## Tests and production build

Backend:

```bash
cd backend
cargo test
```

Frontend unit tests and production build:

```bash
cd frontend
npm test
npm run build
npm audit
```

Repeatable full browser test from the repository root:

```bash
./frontend/scripts/e2e.sh
```

The script builds the backend, starts API/Vite on the required ports with a clean `/tmp/opencode/paisa-e2e.db`, runs Playwright, and stops both servers. To use another Chromium binary:

```bash
CHROME_PATH=/absolute/path/to/chromium ./frontend/scripts/e2e.sh
```

The backend suite covers:

- original Milestone 1 balances and transfer invariants;
- starter catalog availability for Dining Out and Salary;
- category totals and Uncategorized fallback;
- edit/duplicate/permanent delete balance updates;
- referenced-category archiving and historical preservation;
- editing an existing entry whose category is archived;
- category CRUD, duplicate prevention, and cycle prevention;
- search/date/account/category/status/amount filters;
- saved-filter persistence and reconciliation filtering;
- one combined transfer row and relevant account-history sides;
- transfer edit/delete updating both accounts without changing totals;
- rollback when either transfer-leg insert or update fails;
- legacy-shaped rows remaining uncategorized with unchanged balances;
- INR-only account validation and strict amount handling.

The frontend unit suite covers exact paise parsing, Western/Indian grouping, malformed separator rejection, fractional paise, negative transaction amounts, negative opening balances, safe-integer limits, and INR formatting.

## API

All amounts are integer minor units.

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/api/health` | Health check |
| `GET` | `/api/state` | Accounts, totals, categories, category totals, saved filters, recent combined ledger |
| `POST` | `/api/accounts` | Create an INR account with opening balance |
| `GET` | `/api/accounts/:id/history` | Account-specific ledger with relevant transfer side |
| `GET` | `/api/ledger` | Filtered combined ledger |
| `POST` | `/api/transactions` | Create income/expense |
| `GET/PUT/DELETE` | `/api/transactions/:id` | Detail/edit/permanent delete |
| `POST` | `/api/transactions/:id/duplicate` | Duplicate transaction |
| `PATCH` | `/api/transactions/:id/reconciliation` | Toggle reconciled |
| `POST` | `/api/transfers` | Atomic transfer create |
| `GET/PUT/DELETE` | `/api/transfers/:id` | Detail/edit/delete both legs |
| `POST` | `/api/transfers/:id/duplicate` | Duplicate transfer |
| `PATCH` | `/api/transfers/:id/reconciliation` | Reconcile both legs |
| `GET/POST` | `/api/categories` | List/create categories |
| `PUT/DELETE` | `/api/categories/:id` | Edit/archive or delete category |
| `GET/POST` | `/api/filters` | List/create named filters |
| `PUT/DELETE` | `/api/filters/:id` | Update/delete named filter |

Example transaction:

```json
{
  "kind": "expense",
  "account_id": 1,
  "amount_minor": 25000,
  "name": "Dinner",
  "category_id": 11,
  "reconciled": false,
  "note": "Dining out"
}
```

## Data model and migration safety

- `accounts`: name, INR currency, opening balance, creation timestamp.
- `categories`: income/expense type, optional parent, color/icon/sort order, archive timestamp.
- `entries`: income, expense, transfer_in, or transfer_out; positive amount; name/note; optional category; reconciled flag; optional transfer link.
- `transfers`: source/destination, positive amount, name/note, reconciled flag, timestamps.
- `saved_filters`: named JSON filter definitions.

Migration `002_categories_and_ledger.sql` is additive: it preserves all existing accounts, entries, transfers, amounts, and links. Existing entries receive an empty/default name, null category, and unreconciled status; existing transfer names default from their notes. Balances continue to derive from opening balances plus ledger entries.

A manual upgrade check migrated a Milestone-1-shaped database containing two accounts, income, expense, and a transfer. Before/after values were preserved exactly: balances `82500` and `45000`, income `10000`, expense `2500`, one combined transfer row, and legacy entries shown as Uncategorized.

## Resetting local data

Stop the backend, then from the repository root:

```bash
rm -f backend/budget.db backend/budget.db-wal backend/budget.db-shm
```

Restart with the startup command above.

## Verified Milestone 2 result

Verified on 2026-11-26:

- `cargo test`: **15 passed; 0 failed**
- `npm test`: **7 passed; 0 failed**
- `npm run build`: **passed**
- `npm audit`: **0 vulnerabilities**
- `./frontend/scripts/e2e.sh`: **1 passed** — full category, ledger, saved-filter, archive, Uncategorized, transfer edit/delete, account-history, and reload flow
- Manual Milestone-1 → Milestone-2 migration check: balances, totals, and transfer legs preserved

## Later milestones / explicit deferrals

1. Account types/groups, liabilities, and credit-card account fields
2. Overall and hierarchical category budgets
3. Net earnings, items/labels reports, balance sheet, and calendar
4. **Recurring reminders** — due reminders that wait for approval before posting
5. Splits and attachments
6. Multi-currency totals/conversion and **transfer fees** as separate expenses
7. **CSV export** first, then other import/export and backup work
8. **Paisa MCP** read-only agent access behind the Rust application layer
9. Evaluate any **database switch** only after deployment/concurrency requirements are known
10. Sync, travel mode, biometric security, notifications, and dashboard customization
