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

## Command Center

Command Center is the default home screen. It turns a typed or dictated description of a day's
activity into reviewed ledger entries with one Save. See
[`docs/command-center-plan.md`](docs/command-center-plan.md) for the product behavior and
[`docs/command-center-api-contract.md`](docs/command-center-api-contract.md) for the API shapes.

- **Draft** — one private draft per household member survives refreshes and syncs with that
  member's access. **Prepare** appends a prompt; **Add details** adds a follow-up; **Add manually**
  and **Add accounts** create rows directly. Follow-ups target stable row IDs and keep unrelated
  manual edits. **Prompt history** is collapsed by default, and **Discard** clears the draft.
- **Review** — rows are grouped by local date with descriptions, amounts, endpoints, and explicit
  text issues. Select rows with unresolved accounts and assign one eligible account to all of them.
  Every incomplete row must be resolved before **Save**. Dates resolve against the prompt's local
  day; an itemized total mismatch, ambiguous account, or unclear balance must be clarified.
- **Atomic save** — the reviewed revision is revalidated and committed in one `BEGIN IMMEDIATE`
  transaction: new accounts first, then expenses, income, and same-owner transfers. A forced
  failure rolls everything back and keeps the draft. A repeated Save of the same member and
  revision returns the same receipt and posts nothing. New accounts created from a stated current
  balance store the opening balance calculated by walking back the confirmed activity.
- **Receipt** — counts saved accounts and entries, lists pending transfer requests separately,
  and links to the saved activity in Ledger.
- **Household** — every account has one owner. The creator owns new accounts; legacy accounts are
  assigned by the designated member after a review step, and confirmed ownership cannot change.
  Both members can read all saved accounts and activity; writes to another member's account are
  rejected by the ordinary API and the draft save path alike. Transcripts, nicknames, and drafts
  are private to their member.
- **Transfer requests** — a cross-member transfer in a batch becomes a pending request with no
  balance or ledger effect. The receiving owner approves or declines; the sender can cancel.
  Approval rechecks the endpoints and posts exactly two legs under the requested date in one
  transaction, recording the approval time separately. Posted cross-member transfers cannot be
  edited directly; corrections use an approved reversal in the opposite direction.
- **Assistant** — `backend/src/assistant.rs` sends only the submitted text, local date, unsaved
  rows, and a small allowlist of relevant account/category names. It never reads ledger history or
  balances. The default route is `deepseek/deepseek-v4.1-flash-1017` pinned to the
  `sail-research/fp4` endpoint with fallbacks disabled and strict JSON Schema required. No app or
  analytics attribution headers/metadata are sent. Without a key the assistant is disabled and
  manual capture still works; `PAISA_ASSISTANT_MODE=stub` provides deterministic offline pragmas
  for tests.
- **Assistant cost history** — Command Center has a collapsed read-only total and per-day history
  sourced only from OpenRouter's actual reported `usage.cost` in USD. Costs use exact decimal
  strings, distinguish reported zero from missing cost, and are scoped to the signed-in member.
  See [`docs/command-center-openrouter.md`](docs/command-center-openrouter.md).

### Household identity configuration

Household mode is opt-in. Without it, existing callers resolve to a single default member and all
legacy flows are unchanged. To enable it, set these on the backend (the shared token is a
placeholder boundary until a deployment supplies verified sign-in). The backend also reads an
allowlisted set of these keys from Git-ignored `backend/.env`; process environment values win.

| Variable | Purpose |
| --- | --- |
| `PAISA_HOUSEHOLD_ENABLED` | `true` or `1` requires a verified member on every request. |
| `PAISA_MEMBER_TOKEN` | Shared token required in `x-paisa-member-token`. |
| `PAISA_DEFAULT_MEMBER_ID` | Fallback member while household mode is off (default `1`). |
| `OPENROUTER_API_KEY` | Optional assistant key; kept only in Git-ignored `backend/.env`. |
| `OPENROUTER_MODEL`, `OPENROUTER_URL` | Optional deployment model/endpoint overrides; defaults are the pinned DeepSeek route above. |
| `PAISA_ASSISTANT_MODE` | `stub` for offline deterministic proposals; `off` disables. |
| `PAISA_PORT` | Optional API port for isolated runs; defaults to `3000`. |

The frontend sends `x-paisa-member` and `x-paisa-member-token` only when
`VITE_PAISA_HOUSEHOLD_ENABLED === "true"`; member id and token come from browser storage.

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
- credit-card cutoff statements, due-day rollover/short-month clamping, card-side payments, separate credits, visibility, missing settings, invalid legacy dates, before/inside/after-cutoff opening-baseline gating, and checked-paise overflow;
- one combined transfer row and relevant account-history sides;
- transfer edit/delete updating both accounts without changing totals;
- rollback when either transfer-leg insert or update fails;
- legacy-shaped rows remaining uncategorized with unchanged balances;
- INR-only account validation and strict amount handling;
- household membership and owner-scoped writes, designated-member assignment review, and
  read-only other-member views;
- atomic batch save with new accounts, receipt replay, unresolved-row rejection, rollback on a
  forced mid-batch failure, and backwards-reconciled current balances;
- private prompt/nickname isolation, assistant allowlist, stub proposals, and manual completion
  while the assistant is unavailable;
- pending transfer requests, single approval with both legs, decline/cancel, posted-transfer
  protection, and separately approved reversals.

The frontend unit suite covers exact paise parsing, Western/Indian grouping, malformed separator rejection, fractional paise, negative transaction amounts, negative opening balances, safe-integer limits, INR formatting, and Command Center draft grouping/receipt helpers.

The browser suite also covers the Command Center flow: a prompt batch becomes reviewable rows,
account assignment resolves them, Save posts to Ledger with a receipt, a new account and its
expense save together, a draft survives reload, and a failed prompt keeps its text while manual
save still succeeds.

## API

All amounts are integer minor units.

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/api/health` | Health check |
| `GET` | `/api/state` | Accounts, totals, categories, category totals, saved filters, recent combined ledger |
| `POST` | `/api/accounts` | Create an INR account with opening balance |
| `GET` | `/api/accounts/:id/history` | Paginated account-specific ledger with canonical ledger filters, relevant transfer side, filtered count, and full-day totals |
| `GET` | `/api/ledger` | Paginated filtered combined ledger with full-day totals; supports `limit` and `offset` |
| `GET` | `/api/ledger/invalid-date-expenses` | Paginated expenses whose stored timestamps fail the shared date-validity rules |
| `GET` | `/api/ledger/export` | UTF-8 CSV of every transaction matching the same filter query parameters; ignores `limit` and `offset` |
| `GET` | `/api/reports/items-summary?month=YYYY-MM` | Selected-month income/expense totals and ranked exact-name item groups with counts |
| `GET` | `/api/reports/daily-summary?anchor_date=YYYY-MM-DD` | Seven-day expense chart values and full-window 7/30-day average expenses |
| `GET` | `/api/reports/balance-sheet?month=YYYY-MM` | Prior/selected month-end signed balances by asset, liability, and unclassified group |
| `GET` | `/api/reports/credit-card?account_id=:id&anchor_date=YYYY-MM-DD` | Selected card's signed current balance, ledger-derived cutoff statement, card-directed payments, advances, charges, and non-transfer credits |
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
| `GET/POST` | `/api/labels` | List/create reusable labels |
| `PUT` | `/api/labels/:id` | Rename a reusable label |
| `GET/POST` | `/api/filters` | List/create named filters |
| `PUT/DELETE` | `/api/filters/:id` | Update/delete named filter |
| `GET` | `/api/draft` | Current private draft with rows, prompts, and revision |
| `GET` | `/api/assistant/usage` | Member-scoped exact reported USD total and per-day assistant cost history |
| `POST` | `/api/draft/prompt` | Append a prompt and apply an assistant proposal |
| `POST/PUT/DELETE` | `/api/draft/rows` or `/api/draft/rows/:row_id` | Edit draft rows manually |
| `POST` | `/api/draft/rows/assign-account` | Assign one account to selected rows |
| `POST` | `/api/draft/remember-alias` | Remember a personal account nickname |
| `POST` | `/api/draft/discard` | Clear the active draft |
| `POST` | `/api/draft/save` | Save the reviewed revision; idempotent per member/revision |
| `GET` | `/api/transfer-requests` | Incoming/outgoing requests and counts |
| `POST` | `/api/transfer-requests/:id/approve` | Approve an incoming request (recipient only) |
| `POST` | `/api/transfer-requests/:id/decline` | Decline an incoming request (recipient only) |
| `POST` | `/api/transfer-requests/:id/cancel` | Cancel a pending request (sender only) |
| `POST` | `/api/transfers/:id/reverse` | Start a reversal request from a posted transfer |
| `GET` | `/api/household/members` | Household member list |
| `GET` | `/api/household/accounts?member_id=` | Own or other-member read-only account view |
| `GET` | `/api/household/assignments` | Designated member's ownership review |
| `POST` | `/api/household/assignments/propose` | Propose account ownership assignments |
| `POST` | `/api/household/assignments/confirm` | Confirm assignments atomically |

Transaction and transfer requests accept optional `labels` arrays. Omitting the array preserves existing assignments on edit; an explicit empty array clears them. Labels are returned on ledger/detail/history entries, copied by duplication, and removed when their owning entry/transfer is deleted. F13 does not expose global label deletion, so historical assignments are preserved. A transfer carries one label set shown on both history sides. F13 intentionally does not add label filtering/reporting or change CSV columns and report totals; those remain F14/F15 work.

The invalid-date correction endpoint uses the same all-month expense semantics as the Budgets warning. It excludes valid dates, income, and transfers, returns authoritative rows/counts/pagination, and intentionally has no daily totals because the dates are unknown.

Account history accepts the same filter query parameters as `/api/ledger`. The path account is always pinned: `account_id` may be omitted or match the path ID; a conflicting value returns HTTP 400. Transfers still show only the side affecting that account.

Ledger, account history, saved filters, and CSV export share these input rules:

- `date_from` and `date_to`, when supplied, must be strict `YYYY-MM-DD` calendar dates. Empty, malformed, impossible, or inverted ranges return HTTP 400. Boundaries include the whole local calendar day for date-only, local date-time, and RFC3339 timestamps.
- Search is an ASCII case-insensitive literal substring of the name or note; non-ASCII characters match exactly. `%`, `_`, and `\` are ordinary characters, not wildcards or escapes; both operands use the same SQLite folding and search values are bound parameters for entries and transfers.
- With an active date filter, legacy rows whose stored timestamp is invalid are excluded. Unfiltered history still returns them, and unfiltered CSV keeps its existing blank `Date` serialization.
- Daily, category, and budget aggregates accumulate in wider integer paise and return HTTP 409 if a requested `i64` output cannot represent the total.

Items Summary groups transaction names exactly and case-sensitively; notes are not searched. Income and expenses rank separately by descending total, with exact-name order breaking display ties. Equal totals share a competition rank (`1, 2, 2, 4`). It uses the same report-month, account visibility, invalid-date, transfer, and opening-balance rules as Net Earnings.

Balance Sheet uses cumulative month-end snapshots for the selected month and the month immediately before it. Valid posted income, expenses, and both transfer legs before the next month starts are included. A known opening date gates only the opening baseline; posted history remains even if that metadata changes. Legacy accounts without an opening date retain their baseline in both snapshots, a documented historical limitation. Balances stay signed, and net worth equals assets plus signed liabilities plus unclassified balances. Report-hidden accounts and invalid legacy dated entries are excluded; picker-only hidden accounts remain included.

Daily Summary requires a strict local `anchor_date=YYYY-MM-DD` and includes that day. Its chart covers the anchor and previous six calendar days, including real zero-spend days. Both averages use their full 7- or 30-calendar-day window and round to the nearest integer paise, with exact halves rounded up. Anchors before `0000-01-30` cannot provide a complete non-negative 30-day window and return HTTP 400. Only posted expenses count; report visibility, invalid-date, transfer, and opening-balance rules match the other reports.

## Using Card Summary

1. Open **Card Summary** and choose a report-visible Credit Card account.
2. Choose a cutoff-date anchor. The anchor selects the calendar month whose clamped cutoff closes the statement period; it is not an issuer statement date.
3. Read the current signed balance separately from the statement balance at cutoff, charges, advances, non-transfer credits, card-directed payments, due date, and post-cutoff activity.

Paisa uses these explicit ledger-derived rules:

- The statement starts the day after the previous month's clamped cutoff and ends after the selected cutoff day, so the cutoff day is included and the next day is excluded. Boundaries use the shared local calendar-date prefix and half-open date comparisons.
- The due date is the first clamped due-day strictly after cutoff. A due-day equal to the cutoff rolls to the next month. Day 31 clamps to February 28 or 29 as appropriate.
- Canonical signed delta is `income = +amount`, `expense = -amount`, card `transfer_in = +amount`, and card `transfer_out = -amount`. A bank/asset-account → card transfer is the card's incoming `transfer_in` leg: it reduces debt and is a card-directed payment. A card → bank/asset-account `transfer_out` is an advance/withdrawal: it increases debt and is never a payment. Non-transfer income is a separate income/refund credit.
- A valid dated opening baseline activates before the period only when `opening_date < period_start`, inside the period when `period_start <= opening_date < cutoff_date + 1 day`, and after cutoff in neither balance. A baseline on the cutoff date is inside; one on the following day is excluded. Balance before the period includes only the before-period baseline plus valid canonical deltas before the period starts. Statement balance at cutoff reconciles as `balance before period + opening baseline activated inside period − charges − advances + non-transfer credits + card-directed statement payments`.
- Statement-period and post-cutoff windows separately count charges (`expense`), advances (`transfer_out`), card-directed payments (`transfer_in`), and non-transfer credits (`income`). The post-cutoff payment window is **after cutoff through the due date inclusive**. A payment on cutoff is included in the statement payment total and cutoff balance, never in the post-cutoff payment total. The counterpart transfer leg is never counted again.
- A known opening date gates only the opening baseline; posted history and historical transaction dates are retained. A missing opening date keeps the legacy baseline before the period and is disclosed; an invalid opening date returns an explicit error. The anchor selects a cutoff cycle, not an as-of date, so valid ledger-dated activity in the selected windows is included even if it is later than the anchor or today.
- Current balance uses the canonical signed account balance: opening balance plus all posted income, expenses, and card-side transfer legs, including later activity and invalid legacy dates. Invalid dates are counted and excluded from the cutoff statement. Available limit is `credit_limit + current_balance`, calculated with checked integer paise and labeled ledger-based.
- Negative balances are **Amount owed**, positive balances are **Credit balance**, and zero is **Settled**. Missing cutoff or due settings leave the statement unavailable while keeping the current summary visible. Invalid anchors/settings and invalid opening dates for an available statement return explicit errors.
- Report-hidden cards are excluded; picker-only hidden cards remain included. Paisa has no account archive field. Archived categories keep their historical assignments and do not alter card totals.

This view is **not an issuer-provided bill, minimum due, payment allocation, or reminder schedule**. The saved Bluecoins Demo evidence establishes a Credit Card Summary entry and labels, not those calculation rules. On 2026-11-27 the Demo showed cutoff day 18, due day 2, credit limit ₹14,000.00, Previous Balance −₹235.03, Purchases/Advances ₹0.00, Payments/Credit +₹68.91, Outstanding Balance −₹166.12, Not billed yet ₹0.00, and Available Credit Limit ₹13,833.88 (`ev_8f200602d8bf609b63445f978598f233409a31aea4f43c8420fd2ce26d3c0b92`, `ev_049eb5a73d397d9f729c13f1b9f2880fd325503e1959615486a4d7891652789d`). Its internal cutoff inclusion, rollover, allocation, and minimum-due behavior remain unverified.

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

Command Center verification: Rust fmt/check, **108 Rust tests** (including household isolation,
atomic batch rollback/retry, transfer approval/reversal, and assistant stub/allowlist suites),
**21 frontend unit tests**, the production build, and `npm audit` (0 vulnerabilities) pass. The
four new `command-center` browser tests pass against the disposable SQLite runner. With the suite
serialized on its single shared database, the current tree is **34 passed / 10 failed** and the
untouched baseline is **30 passed / 10 failed**: the same ten legacy failures (shared-database
ordering and strict selector behavior in this environment), with no new regressions from Command
Center.

Historical baseline verified on 2026-11-26:

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
