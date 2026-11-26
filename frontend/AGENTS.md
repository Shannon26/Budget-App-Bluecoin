# Frontend agent guide

## Folder map

```text
frontend/src/
├── App.tsx                    Navigation, screen composition, global state
├── main.tsx                  React root and global stylesheet entry
├── shared/
│   ├── api.ts                 Typed API facade and CSV download helper
│   ├── types.ts               Transport/domain types shared by features
│   ├── money.ts               Exact paise parsing and INR formatting
│   ├── money.test.ts
│   └── date.ts                Local calendar date and date-time helpers
├── styles.css                 Global feature stylesheet import order
├── features/
│   ├── overview/
│   │   ├── Overview.tsx
│   │   ├── activity.ts
│   │   ├── useOverviewForms.ts  Persistent controlled form state
│   │   └── overview.css
│   ├── accounts/AccountForm.tsx
│   ├── ledger/
│   │   ├── Ledger.tsx
│   │   ├── EntryEditor.tsx
│   │   ├── TransactionForm.tsx
│   │   ├── presentation.ts
│   │   └── ledger.css
│   ├── transfers/TransferForm.tsx
│   ├── categories/
│   │   ├── CategoryManager.tsx
│   │   └── categories.css
│   └── budgets/
│       ├── Budgets.tsx
│       └── budgets.css
└── styles/
    ├── base.css
    └── responsive.css
```

Browser regressions live in `frontend/e2e/milestone2.spec.ts`. The repeatable runner is `frontend/scripts/e2e.sh`.

## Where to add code

- **New screen:** create `features/<feature>/` and mount it from `App.tsx`. Keep `App.tsx` limited to navigation, composition, and global refresh/message state.
- **Overview forms:** keep controlled state in `useOverviewForms.ts`, which remains owned by `App` so drafts survive tab switches. Put markup in the relevant feature form component.
- **Ledger behavior:** keep filters/paging in `features/ledger/Ledger.tsx`, entry editing in `EntryEditor.tsx`, and presentation mapping in `presentation.ts`.
- **API call:** extend `shared/api.ts`; do not let shared utilities import feature components.
- **Money/date logic:** reuse `shared/money.ts` and `shared/date.ts`. Do not duplicate parsing or date formatting in features.
- **Styles:** add feature rules to that feature's CSS file. `styles.css` imports global and feature sheets in cascade order; preserve that order.

## Commands

From `frontend/`:

```bash
npm test
npm run build
npm audit
npm run dev
```

From the repository root:

```bash
./frontend/scripts/e2e.sh
```

The browser runner starts API/Vite against a clean disposable `/tmp/opencode/paisa-e2e.db` and stops both servers. Set `CHROME_PATH` if Chromium is not at `/usr/bin/google-chrome`.

## Rules to preserve

- **Stable selectors:** existing `data-testid` values are browser-test contracts. Do not rename or remove them without updating the regression suite.
- **Shared helpers:** use exact `parseMinorUnits`/`formatMinor` for integer-paise input and INR display. Use `localDateValue`/`dateTimeValue` for ledger-compatible dates.
- **Month-specific requests:** every budget request must capture its selected `YYYY-MM`, check the response month, and ignore stale requests. Preserve `requestSequence`, month-bound save/remove guards, and disabled edit/remove controls until the selected month loads.
- **Overview drafts:** keep the shared `submitting` flag and controlled form state behavior. Leaving and reopening Overview must not silently clear drafts.
- **Filters/export:** CSV download uses the applied ledger filter, including reopened saved filters, and exports all matches rather than only loaded rows.
- **Transfers:** the main ledger shows one combined transfer; account history shows the relevant side. Never fold transfers into income/spending totals.
- **Budgets:** keep parent rollups and independent child limits visible. Archived historical categories remain on screen. Invalid-date warnings must stay visible and actionable.
- **Compatibility:** preserve current markup, CSS behavior, API shapes, startup commands, dependencies, and browser flows unless the task explicitly requires a change.
