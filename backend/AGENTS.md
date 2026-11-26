# Backend agent guide

## Folder map

```text
backend/
├── migrations/                 Additive SQLite schema changes
└── src/
    ├── main.rs                 Tracing, pool/migrations, bind/startup only
    ├── app.rs                  AppState, CORS, route table, router wiring
    ├── csv.rs                  Stable ledger CSV format and spreadsheet safety
    ├── date.rs                 Shared timestamp/month validation and bounds
    ├── error.rs                AppError and HTTP/JSON error mapping
    ├── models/                 Serde/SQLx DTOs grouped by feature
    ├── routes/                 Thin HTTP handlers grouped by feature
    ├── store/
    │   ├── mod.rs              Store, shared validation helpers, aggregate state
    │   ├── accounts.rs
    │   ├── transactions.rs
    │   ├── transfers.rs
    │   ├── ledger.rs
    │   ├── categories.rs
    │   ├── budget_rules.rs     Shared parent/child budget and move checks
    │   ├── budgets.rs
    │   └── filters.rs
    └── tests/                  Feature regression suites; mod.rs is the harness
```

## Where to add code

- **New endpoint:** add a thin handler in the matching `routes/*` module, register the unchanged path/method in `app.rs`, and call a `Store` method.
- **New query or mutation:** add it to the matching `store/*` module. Reuse shared amount/name/note/date helpers in `store/mod.rs` and `date.rs`.
- **New request/response type:** add it to the matching `models/*` module; `models/mod.rs` re-exports the existing API surface.
- **Schema change:** add a new numbered migration. Never rewrite an applied migration.
- **Regression:** add a test to the matching `tests/*` module. Shared fixtures live in `tests/mod.rs` and create temporary migrated SQLite databases.

## Commands

From `backend/`:

```bash
cargo fmt --check
cargo check --locked
cargo test --locked
cargo run
```

From the repository root, run the full browser suite against a disposable database:

```bash
./frontend/scripts/e2e.sh
```

`cargo run` listens on `127.0.0.1:3000`, uses `sqlite://budget.db?mode=rwc` by default, and runs migrations automatically. Override `DATABASE_URL` for another SQLite file.

## Rules to preserve

- **Money:** store and calculate signed 64-bit integer paise. Never use floating-point money. INR remains the only supported currency.
- **Transfers:** create/update/delete the transfer header and both legs atomically in one SQLite transaction. Transfers never affect income or expense totals.
- **Ledger/CSV:** pagination and CSV must share the same filter and ordering logic. CSV exports every match, not only the first page, and keeps its stable headers, UTF-8 quoting, two-decimal INR amounts, one-row transfers, and formula-prefix protection.
- **Dates:** validate transaction/transfer timestamps through `date.rs`. Monthly queries use validated calendar dates and half-open month bounds. Invalid legacy dates stay out of monthly totals and are reported for correction.
- **Budgets:** store one positive integer-paise limit per category/month. Count only expenses. Parent spending includes direct and descendant expenses; overall spending counts each expense once and includes Uncategorized expenses.
- **Budget concurrency:** hierarchy validation and budget upsert must stay in the same `BEGIN IMMEDIATE` transaction. Category move validation and update use the same immediate-write transaction. Do not weaken the transaction start/read/validate/write/commit ordering.
- **Budget hierarchy:** a child limit cannot exceed its parent limit for the same month; a parent cannot be lowered below a child. Moving a child checks every overlapping month.
- **Category safety:** categories with transactions, children, or saved budgets archive instead of being deleted. Budgeted categories cannot change type. Archived categories remain visible in historical budget reads.
- **Compatibility:** keep API paths, JSON shapes, status codes, CSV columns, dependencies, startup commands, and existing migrations unchanged unless the task explicitly requires a change.
