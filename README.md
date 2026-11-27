# Paisa — transactions, categories, CSV export, and monthly budgets

An original, local-first budgeting web app. It includes a seeded category catalog, date-grouped ledger, transaction management, reconciliation, saved filters, account history, atomic transfers, Rust-generated CSV export, and month-specific category budgets. Bluecoins remains the long-term parity reference; see [`docs/bluecoins-parity.md`](docs/bluecoins-parity.md) and [`docs/bluecoins-walkthrough.md`](docs/bluecoins-walkthrough.md).

## Stack

- **Backend:** Rust, Axum, Tokio, SQLx, bundled SQLite, SQL migrations
- **Frontend:** React, TypeScript, Vite, Playwright
- **Money:** signed 64-bit integer minor units (paise); decimal text is validated and converted exactly in the browser
- **Currency:** INR only, enforced by both UI and API until per-currency reporting exists

## Project layout and agent guides

- [`backend/AGENTS.md`](backend/AGENTS.md) documents backend folders, where to add routes/store/models/tests, required checks, and invariants for paise, atomic transfers, dates, CSV, and budget transactions.
- [`frontend/AGENTS.md`](frontend/AGENTS.md) documents feature folders, shared helpers, month-specific request handling, stable browser selectors, and frontend checks.
- Frontend screens live under `frontend/src/features/`; shared API, money, date, and types live under `frontend/src/shared/`.
- Backend HTTP, store, model, and test code is grouped under `backend/src/routes/`, `store/`, `models/`, and `tests/`; `main.rs` contains startup/wiring only.

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
- New hierarchy changes are limited to category → subcategory; parents must be active top-level categories of the same type.
- Categories with subcategories cannot be moved until those subcategories are moved first.
- Optional categories with **Uncategorized** fallback; transfers remain uncategorized.
- Referenced or budgeted categories archive instead of disappearing, preserving history, limits, and totals while leaving new-entry pickers.
- Date-grouped ledger with daily income, expense, and transfer totals.
- Stable 200-entry pages with **Load more** for older matching entries in both the main ledger and account history.
- Daily totals cover all matching transactions for that day, including entries not yet loaded.
- Search by name/note and filters for date, account, category, reconciliation status, amount range, and uncategorized entries.
- Named filters persist in SQLite and can be reopened after reload.
- **Download CSV** exports all transactions matching the applied filters, including rows beyond the first 200 and filters reopened from a saved view.
- **Budgets** provides independent monthly limits for expense categories and subcategories, parent rollups, progress, remaining/over states, and historical archived-category visibility.
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
11. With 201 same-day ₹1 expenses, verify the first page shows a ₹201 daily total and Load more reaches all 201 entries without gaps or duplicates.
12. Download the filtered ledger while only 200 rows are loaded and verify the CSV contains all 201; reopen a saved filter and verify its export, one-row transfers, Uncategorized fallback, special-character quoting, and spreadsheet-formula protection.
13. Set monthly parent/subcategory budgets, verify ₹2,800 parent spending and ₹300 child overage, switch months, reload persisted limits, and retain archived category history.

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
- category CRUD, duplicate prevention, cycle prevention, and two-level hierarchy enforcement;
- moving childless categories without changing transaction assignments;
- search/date/account/category/status/amount filters;
- stable ledger/account-history pagination and full-day totals for the 201-entry regression;
- CSV export of all 201 filtered rows despite `limit`/`offset`, exact INR formatting, quoting, formula protection, archived category names, Uncategorized fallback, and one combined transfer row;
- transaction/transfer date validation with no writes on rejection, plus blank CSV dates for invalid legacy timestamps;
- monthly budget validation, parent/child rollups and limit invariants, category-move checks, budget-aware archiving/type changes, overall de-duplication, Uncategorized inclusion, invalid-date warnings, month/leap-day boundaries, and edit/delete recalculation;
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
| `GET` | `/api/accounts/:id/history` | Paginated account-specific ledger with canonical ledger filters, relevant transfer side, filtered count, and full-day totals |
| `GET` | `/api/ledger` | Paginated filtered combined ledger with full-day totals; supports `limit` and `offset` |
| `GET` | `/api/ledger/export` | UTF-8 CSV of every transaction matching the same filter query parameters; ignores `limit` and `offset` |
| `GET` | `/api/reports/items-summary?month=YYYY-MM` | Selected-month income/expense totals and ranked exact-name item groups with counts |
| `POST` | `/api/transactions` | Create income/expense |
| `GET/PUT/DELETE` | `/api/transactions/:id` | Detail/edit/permanent delete |
| `POST` | `/api/transactions/:id/duplicate` | Duplicate transaction |
| `PATCH` | `/api/transactions/:id/reconciliation` | Toggle reconciled |
| `POST` | `/api/transfers` | Atomic transfer create |
| `GET/PUT/DELETE` | `/api/transfers/:id` | Detail/edit/delete both legs |
| `POST` | `/api/transfers/:id/duplicate` | Duplicate transfer |
| `PATCH` | `/api/transfers/:id/reconciliation` | Reconcile both legs |
| `GET/POST` | `/api/categories` | List/create categories |
| `PUT/DELETE` | `/api/categories/:id` | Edit; archive referenced/budgeted categories or delete unused categories |
| `GET` | `/api/category-budgets?month=YYYY-MM` | Monthly expense spending, category rollups, limits, remaining/over amounts, and archived categories |
| `PUT` | `/api/category-budgets/:category_id?month=YYYY-MM` | Create/update a monthly category limit from `{ "amount_minor": 500000 }` |
| `DELETE` | `/api/category-budgets/:category_id?month=YYYY-MM` | Remove a monthly category limit |
| `GET/POST` | `/api/filters` | List/create named filters |
| `PUT/DELETE` | `/api/filters/:id` | Update/delete named filter |

Account history accepts the same filter query parameters as `/api/ledger`. The path account is always pinned: `account_id` may be omitted or match the path ID; a conflicting value returns HTTP 400. Transfers still show only the side affecting that account.

Ledger, account history, saved filters, and CSV export share these input rules:

- `date_from` and `date_to`, when supplied, must be strict `YYYY-MM-DD` calendar dates. Empty, malformed, impossible, or inverted ranges return HTTP 400. Boundaries include the whole local calendar day for date-only, local date-time, and RFC3339 timestamps.
- Search is an ASCII case-insensitive literal substring of the name or note; non-ASCII characters match exactly. `%`, `_`, and `\` are ordinary characters, not wildcards or escapes; both operands use the same SQLite folding and search values are bound parameters for entries and transfers.
- With an active date filter, legacy rows whose stored timestamp is invalid are excluded. Unfiltered history still returns them, and unfiltered CSV keeps its existing blank `Date` serialization.
- Daily, category, and budget aggregates accumulate in wider integer paise and return HTTP 409 if a requested `i64` output cannot represent the total.

Items Summary groups transaction names exactly and case-sensitively; notes are not searched. Income and expenses rank separately by descending total, with exact-name order breaking display ties. Equal totals share a competition rank (`1, 2, 2, 4`). It uses the same report-month, account visibility, invalid-date, transfer, and opening-balance rules as Net Earnings.

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

## Using monthly budgets

1. Open **Budgets**. The selected month defaults to the current local calendar month.
2. Use the month picker or arrow buttons to view another month.
3. Click **Set limit** on an expense category or subcategory, enter a positive INR amount, and save. **Edit limit** changes it; **Remove** deletes only that month's limit.
4. Each row shows its limit, spending, remaining or over amount, percentage used, and a progress bar.

While a month is loading, its figures and edit/remove controls are unavailable. Late responses for a previously selected month, including save/remove responses, cannot replace the selected month's figures.

Budget rules:

- Limits are stored independently per `(category, YYYY-MM)` in integer paise and are not copied into future months.
- A parent row's spending includes expenses assigned directly to the parent plus expenses assigned to all of its subcategories. Each subcategory keeps its own independent limit.
- A subcategory limit cannot exceed its parent's limit for the same month, and a parent limit cannot be lowered below an existing child limit. Moving a subcategory under a new parent checks every overlapping budget month.
- **Monthly spending** sums each expense exactly once. It is not the sum of parent and child rows. Uncategorized expenses are included; income and transfers are excluded.
- Expenses with invalid stored dates cannot be assigned to a month. Budgets excludes them from monthly totals and displays a warning with their count and amount so they can be corrected in Ledger.
- Categories with saved budgets archive instead of being permanently deleted, and their type cannot be changed while any budget exists.
- Archived categories remain visible with their historical limits and spending. Existing archived limits can be edited or removed, but a new limit cannot be added to an archived category.
- Month boundaries use validated calendar dates and a half-open range, so the last instant of one month and the first of the next are not double-counted. Transaction edits, deletions, and category hierarchy changes are reflected when budgets are loaded.
- Progress is capped visually at 100%, while the percentage and **over** amount continue to show the true excess.
- Budget saves and removals commit only with a successful checked snapshot. If an aggregate cannot fit the API integer range, the write rolls back and returns HTTP 409.

For example, when **Dining Out** is a child of **Food & Drink**, a ₹5,000 Food & Drink limit and a ₹1,500 Dining Out limit with ₹1,000 groceries and ₹1,800 dining show:

- Food & Drink: **₹2,800.00 spent**, **₹2,200.00 left**.
- Dining Out: **₹1,800.00 spent**, **₹300.00 over**.
- Overall monthly spending: **₹2,800.00**, not ₹4,600.00.

## Downloading CSV

1. Open **Ledger**.
2. Apply the desired search, date, account, category, status, amount, or Uncategorized filter. Reopening a saved filter also updates the applied filter.
3. Click **Download CSV**. The browser saves `paisa-ledger.csv`.

The export uses the **applied** filter, not unfinished edits in the filter form. It includes every matching transaction, even when the on-screen ledger has loaded only its first 200 rows. The button is available on the main ledger, not account history. CSV export itself requires no schema changes.

The Rust backend returns `text/csv; charset=utf-8` with an attachment filename. The file has this stable header order:

```text
Date,Type,Name,Account,Source Account,Destination Account,Category,Subcategory,Amount,Currency,Status,Note
```

| Column | Meaning |
| --- | --- |
| `Date` | Transaction date in `YYYY-MM-DD` form; blank if a legacy stored timestamp is invalid. |
| `Type` | `Income`, `Expense`, or `Transfer`. |
| `Name` | Transaction/transfer name. |
| `Account` | Account for income/expenses; blank for transfers. |
| `Source Account` | Transfer source; blank for income/expenses. |
| `Destination Account` | Transfer destination; blank for income/expenses. |
| `Category` | Parent category, top-level category, or `Uncategorized`; blank for transfers. |
| `Subcategory` | Child category when assigned; otherwise blank. |
| `Amount` | Positive INR magnitude with exactly two decimal places and no thousands separator. Direction comes from `Type`. |
| `Currency` | Always `INR`. |
| `Status` | `Reconciled` or `Unreconciled`. |
| `Note` | Transaction/transfer note. |

Each transfer is exported **once**, with both account endpoints, so rows of type `Transfer` can be excluded from income and spending totals. Archived categories retain their stored names on historical entries. Income and expenses without a category use `Uncategorized`.

Commas, quotes, and line breaks are CSV-quoted and round-trip correctly. User-controlled text that begins with a spreadsheet formula marker (`=`, `+`, `-`, or `@`, including after leading whitespace) receives a leading apostrophe. This is intentional spreadsheet-injection protection; for example, `=SUM(1,2)` exports as `'=SUM(1,2)`.

Transaction and transfer saves validate `occurred_at` before writing. Accepted forms are `YYYY-MM-DD`, local ISO date-times such as `2026-11-20T12:00:00` (optional fractional seconds), and RFC3339 date-times with `Z` or an offset. Invalid values return HTTP 400 and leave existing data unchanged. Invalid legacy timestamps are not rewritten; their CSV `Date` cell is blank so arbitrary text cannot break the date format or become a spreadsheet formula.

CSV import is not implemented.

## Data model and migration safety

- `accounts`: name, INR currency, opening balance, creation timestamp.
- `categories`: income/expense type, optional parent, color/icon/sort order, archive timestamp.
- `entries`: income, expense, transfer_in, or transfer_out; positive amount; name/note; optional category; reconciled flag; optional transfer link.
- `transfers`: source/destination, positive amount, name/note, reconciled flag, timestamps.
- `saved_filters`: named JSON filter definitions.
- `category_budgets`: one positive integer-paise limit per category and calendar month, with a composite uniqueness constraint.

Migration `002_categories_and_ledger.sql` is additive: it preserves all existing accounts, entries, transfers, amounts, and links. Existing entries receive an empty/default name, null category, and unreconciled status; existing transfer names default from their notes. Balances continue to derive from opening balances plus ledger entries.

Migration `003_fix_dining_category.sql` moves the existing **Dining Out** category from **Food & Drink** to **Entertainment** to match the observed Bluecoins taxonomy. It changes only the parent pointer, so historical transaction assignments and category totals remain intact.

Migration `004_category_budgets.sql` is additive. It creates only the monthly budget table and index, leaves all ledger/account/category data untouched, and cascades budget rows only when an otherwise unreferenced category is permanently deleted.

A manual upgrade check migrated a Milestone-1-shaped database containing two accounts, income, expense, and a transfer. Before/after values were preserved exactly: balances `82500` and `45000`, income `10000`, expense `2500`, one combined transfer row, and legacy entries shown as Uncategorized.

## Resetting local data

Stop the backend, then from the repository root:

```bash
rm -f backend/budget.db backend/budget.db-wal backend/budget.db-shm
```

Restart with the startup command above.

## Verified result

Verified on 2026-11-26:

- `cargo test`: **31 passed; 0 failed**
- `npm test`: **7 passed; 0 failed**
- `npm run build`: **passed**
- `npm audit`: **0 vulnerabilities**
- `./frontend/scripts/e2e.sh`: **6 passed** — hierarchy editor rules, full Milestone 2 flow, CSV regressions, monthly budget persistence/rollup/archive behavior, and stale GET/PUT/DELETE month-response regressions
- Bluecoins runtime export: **628 demo rows** observed through ADB; workflow and fields recorded in [`docs/bluecoins-walkthrough.md`](docs/bluecoins-walkthrough.md)
- Bluecoins runtime budget table: parent/child limits, spent, remaining, over-budget, progress, and Last Month switching observed through ADB
- Manual Milestone-1 → Milestone-2 migration check: balances, totals, and transfer legs preserved

## Later milestones / explicit deferrals

1. Account types/groups, liabilities, and credit-card account fields
2. Net earnings, items/labels reports, balance sheet, and calendar
3. **Recurring reminders** — due reminders that wait for approval before posting
4. Splits and attachments
5. Multi-currency totals/conversion and **transfer fees** as separate expenses
6. **CSV import**, QIF import, and backup/restore work
7. **Paisa MCP** read-only agent access behind the Rust application layer
8. Evaluate any **database switch** only after deployment/concurrency requirements are known
9. Sync, travel mode, biometric security, notifications, and dashboard customization
