# Paisa — first working milestone

An original, local-first budgeting web app. This milestone implements accounts, opening balances, income, expenses, and atomic transfers. Bluecoins is the long-term parity target; static research is in [`docs/bluecoins-parity.md`](docs/bluecoins-parity.md) and the runtime walkthrough/next-build plan is in [`docs/bluecoins-walkthrough.md`](docs/bluecoins-walkthrough.md).

## Stack

- **Backend:** Rust, Axum, Tokio, SQLx, bundled SQLite, SQL migrations
- **Frontend:** React, TypeScript, Vite
- **Money:** signed 64-bit integer minor units (paise); the browser converts decimal text exactly before sending it to the API
- **Currency:** INR only in this milestone, enforced by both UI and API; multi-currency reporting is deferred

## Prerequisites

- Rust toolchain with Cargo (rustup is fine)
- Node.js 20+ and npm
- A C compiler for SQLx's bundled SQLite build

If `cargo` is not on your shell's PATH, add rustup's bin directory first:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

## Exact startup commands

Open a terminal at the repository root and start the API:

```bash
cd backend
cargo run
```

The API listens on **http://127.0.0.1:3000**. It creates `backend/budget.db` and runs migrations automatically. To use another database:

```bash
DATABASE_URL='sqlite:///absolute/path/to/budget.db?mode=rwc' cargo run
```

Open a second terminal at the repository root and start Vite:

```bash
cd frontend
npm install
npm run dev
```

Open **http://127.0.0.1:5173**. Vite proxies `/api` to `http://127.0.0.1:3000`.

Stop either process with `Ctrl+C`.

## Acceptance flow

1. Create **Cash** with opening balance `1000`.
2. Create **Wallet** with opening balance `200`.
3. Transfer `250` from Cash to Wallet.
4. Reload the browser.
5. Cash shows **₹750.00**, Wallet shows **₹450.00**, total balance is **₹1,200.00**, and income/expense totals remain **₹0.00**.

Income and expense totals also remain unchanged when nonzero: the backend test records ₹100 income and ₹40 expense, transfers ₹250, and verifies the totals stay ₹100/₹40 while balances move correctly.

## Tests and production build

Backend:

```bash
cd backend
cargo test
```

Frontend:

```bash
cd frontend
npm test
npm run build
```

The backend suite covers:

- the exact ₹1,000 / ₹200 → ₹750 / ₹450 acceptance balances;
- persistence through a fresh state read;
- two explicit transfer legs sharing one transfer ID;
- unchanged nonzero income/expense totals;
- invalid same-account, zero-amount, and missing-account transfers;
- rollback of the transfer header and both legs when the second leg fails;
- rejection of non-INR accounts before any row is written.

The frontend suite covers exact decimal-to-paise parsing, valid Western and Indian comma grouping, rejection of malformed separators such as `12,50` and `1,,000`, fractional paise, negative transaction amounts, negative opening balances, safe-integer range checks, and INR formatting.

## API

All request and response amounts are integer minor units.

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/api/health` | Health check |
| `GET` | `/api/state` | Accounts with balances, totals, and recent entries |
| `POST` | `/api/accounts` | Create an account with an opening balance |
| `POST` | `/api/transactions` | Record income or expense |
| `POST` | `/api/transfers` | Atomically transfer between two accounts |

Example transfer:

```json
{
  "from_account_id": 1,
  "to_account_id": 2,
  "amount_minor": 25000,
  "note": "Move to wallet"
}
```

## Data model

- `accounts`: name, currency, opening balance, creation timestamp.
- `transfers`: source account, destination account, positive amount, note, timestamps.
- `entries`: `income`, `expense`, `transfer_in`, or `transfer_out`; positive amount; account; optional transfer link.

A transfer writes one `transfers` row and two `entries` rows in one transaction. Balances are derived from the opening balance plus ledger entries; there is no mutable cached balance to drift out of sync.

## Resetting local data

Stop the backend, then from the repository root:

```bash
rm -f backend/budget.db backend/budget.db-wal backend/budget.db-shm
```

Restart with the startup command above.

## Verified milestone result

Verified on 2026-11-26:

- `cargo test`: **5 passed**
- `npm test`: **7 passed**
- `npm run build`: **passed**
- `npm audit`: **0 vulnerabilities**
- Controlled Chromium browser flow: created Cash ₹1,000 and Wallet ₹200 through the UI, transferred ₹250, reloaded, and observed Cash ₹750.00 / Wallet ₹450.00 / income ₹0.00 / expenses ₹0.00. REA browser Evidence: `ev_3ff990f96ca7b0417957bd07eb0196b64badc29aa255110075ac6aea2647446e`.
- Direct API state after reload: balances `75000` and `45000`; totals `0` and `0`; one transfer header and two transfer legs.
- Feedback regressions: USD account creation returns HTTP 400; malformed `12,50` is blocked in the browser with no ledger entries or transfers written. REA browser Evidence: `ev_a26dd72860d4414e7de28f8329f31c13deacf9f5203c9eda00b5287011d6159b`.

## Next-milestone backlog

**Recommended next milestone:** categories and transaction management — category hierarchy/CRUD, category assignment, date-grouped ledger, detail/edit/delete/duplicate, notes/status, and reusable search/filter predicates. Full scope and acceptance criteria are in [`docs/bluecoins-walkthrough.md`](docs/bluecoins-walkthrough.md).

Following milestones:

1. Account types/groups, liabilities, and credit-card account fields
2. Overall and hierarchical category budgets
3. Net earnings, items/labels reports, balance sheet, and calendar
4. Recurring transactions and reminders
5. Splits and attachments
6. Multi-currency totals, conversion, and transfer fees
7. CSV/QIF import, export, backup/restore, and trash/undo
8. Sync, travel mode, biometric security, notifications, and dashboard customization
