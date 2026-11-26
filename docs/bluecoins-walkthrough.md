# Bluecoins runtime walkthrough and next-build plan

## Method and safety

- Device: physical <android-device>, serial `<device-serial>`, Android 13.
- Artifact: installed Bluecoins `13.1.149` (`33215`); pulled APK SHA-256 matches the analyzed local APK.
- The app was a fresh install. No real financial data existed.
- Onboarding was completed, INR was selected, and Bluecoins' built-in **Run Demo** was used to create simulated sample data.
- REA screen captures were used after each major navigation step. Raw ADB taps/swipes were used only because REA's ADB surface does not expose a general UI-input tool.
- The app remains in **Demo** mode. No reset, delete, purchase, sync, or real-data import was performed.

Detailed static findings remain in [`bluecoins-parity.md`](bluecoins-parity.md).

## Runtime flow observed

| Area | What the demo showed | Product implication |
| --- | --- | --- |
| Onboarding | Welcome screen, language selector, currency picker, “Add First Transaction”, and “Run Demo”. The initial currency defaulted to GBP before INR was selected. | A first-run setup flow and explicit currency choice matter. Our milestone intentionally stays INR-only until totals are currency-aware. |
| Main dashboard | Daily Summary bar chart with 7/30-day expense averages; Budget Summary donut and progress; calendar card; cash-flow comparison; Net Worth trend; Favorite Accounts; Balance Sheet shortcut. | The home screen is a composable dashboard, not just a balance list. Cards can be added incrementally. |
| Transactions | Date-grouped list with daily totals, category icons, reconciled checkmarks, account running balances, and two rows for a transfer pair. Search, bulk edit, and print actions are present. | A real ledger needs grouping, drill-down, and paired transfer display. |
| Expense detail | Name, date/time, signed amount, parent/child category, account type/name, reconciled status, similar transactions, duplicate, print, delete, and edit. | Detail and edit/delete/duplicate are high-value parity features. |
| Transaction editor | Name, date/time, schedule, amount with calculator, currency, category, account, split, status, label, note, attachment, expense/income/transfer mode, and save. | The transaction form is the central workflow; fields can be introduced in stages. |
| Transfer editor | From/to account rows, swap control, transfer fee, status, label, note, and transfer mode. | Transfer fees and explicit direction are separate from basic atomic transfers. |
| Transfer detail | Bank Checking → Credit Card Credit Card, ₹68.91, reconciled, duplicate/edit/print/delete. The list shows both the outgoing and incoming legs. | Our two-leg transfer model matches the visible paired behavior. |
| Reminders | Scheduled/recurring entries grouped by due date, “Due tomorrow/in 2 days”, recurrence text, labels, account balances, and daily totals. | Recurring transactions deserve a first-class schedule model and reminder queue. |
| Balance Sheet | Timeline date selector, prior/current columns, assets and liabilities grouped by account type, percentage contribution, expandable groups, mortgages, and net worth. | Account types and liability semantics are prerequisites for a useful balance sheet. |
| Budget | Monthly period, overall expense budget, hierarchical category budgets, spent/remaining/over amounts, progress bars, and “today” marker. | Budgets should support parent rollups and category-level limits. |
| Net Earnings | Expense, income, and net earnings sections with prior/current month comparison, category hierarchy, percentages, and trend arrows. | Reports need reusable aggregation and period comparison. |
| Items Summary | Ranked income and expense item names with proportional bars for the selected month. | Item/payee aggregation is a lightweight report to add after categories. |
| Labels | Period-based label totals including “No Labels”, with income/expense bars. | Labels are independent of categories and useful for cross-category reporting. |
| Calendar | Month grid with colored transaction dots, selected day, and Net Earnings / Balance Sheet / Transactions / Reminders views. | A calendar can reuse existing ledger and report queries. |
| Accounts | Asset and liability account groups with totals: Bank, Cash, CryptoCurrencies, Foreign Assets, Investments, Other Assets, Properties, Receivables, Virtual Accounts, Credit Card, Foreign Liabilities, Loans, Mortgages, Other Liabilities, Payables. | Account type/group metadata is foundational for balance sheet and account pickers. |
| New/Edit account | Name, note, starting balance, calculator, currency, opening date, account type, hide-from-selection/reports toggles. Credit-card accounts add credit limit, cutoff day, due date, latest balance, and transaction-list shortcut. | Account editing is richer than our current create-only form. |
| Categories | Expense and income hierarchies with icons and monthly budget amounts. Category drill-down shows period total, transaction table, chart toggle, edit/filter/print, and add-transaction action. | Categories should be modeled before budgets or category reports. |
| Filters | Search item/payee/notes, amount from/to, account, labels, status, reset/save/open saved filter. | A reusable filter specification will serve transactions and reports. |
| Settings / data | Localization, currency/date, appearance, data management, online sync, transaction setup, calendar, notifications, password/fingerprint. Data Management offers Phone Storage, CSV import, QIF import, and reset. | Import/export and security are later milestones; CSV parity should reuse the documented import fields. |
| Navigation | Main, Calendar, Accounts, Categories, Labels, Trash, Settings, QuickSync, Travel Mode, feedback/support links. | The app has a broad management surface beyond the dashboard tabs. |

## Recommended next milestone: categories and transaction management

This is the smallest next milestone that unlocks budgets, reports, reminders, and meaningful Bluecoins parity.

### Agreed Paisa decisions

| # | Decision | Agreed behavior |
| --- | --- | --- |
| 1 | Starter categories | Start with a comprehensive, editable set of income and expense categories that works out of the box for everyday and heavy users. Add subcategories where they make a meaningful distinction. Use specific, non-overlapping names so users can tell where an entry belongs. |
| 2 | Category assignment | Categorization is optional. Show uncategorized income and expenses as **Uncategorized**. Transfers have no category. |
| 3 | Category removal | Archive categories that have transactions. Keep their historical assignments and totals, and hide archived categories from selection for new entries. |
| 4 | Transaction deletion | Delete transactions permanently after explicit confirmation. Deleting a transfer removes both legs in one database transaction. |
| 5 | Transfer display | Show one combined transfer row in the main ledger. In each account’s history, show only the side that affects that account. |
| 6 | Reconciliation | Use a manual **Reconciled** toggle. |
| 7 | Saved views | Start with named ledger filters. Save dashboard and report preferences in a later milestone. |
| 8 | Recurring transactions | Remind the user when an entry is due and wait for approval before recording it. |
| 9 | Transfer fees | When fees are supported, record the fee as a separate expense on the paying account. |
| 10 | Data export and agent access | Prioritize CSV export so users can share a ledger extract with their agents. Then build a Paisa MCP server for agents to inspect the ledger through the app. |

**Database decision remains open.** SQLite is the current backend. Evaluate whether to keep it or switch databases based on where Paisa and its agents will run, whether they share one app instance, and what concurrent use will look like. Keep the future read-only MCP queries behind the Rust application layer so the agent integration does not dictate the database choice.

### Scope

1. **Category model and CRUD**
   - `income` or `expense` category.
   - Optional parent category for a two-level hierarchy.
   - Seed the starter catalog and let users customize names, colors/icons, and sort order.
   - Prevent cycles. Archive referenced categories while preserving their historical use.

2. **Transaction category assignment**
   - Optional category selector for income and expenses, with **Uncategorized** as the fallback.
   - No category for transfers.
   - Show parent/child category in lists and detail.

3. **Ledger management**
   - Date-grouped transaction list with daily totals.
   - Detail view with edit, duplicate, and permanent deletion after confirmation.
   - Notes and a manual **Reconciled** toggle.
   - One transfer row in the main ledger and the relevant side in each account’s history.
   - Preserve atomic two-leg behavior when editing/deleting transfers.

4. **Search and filters**
   - Text search across transaction name and note.
   - Filter by account, category, status, amount range, and date range.
   - Save and reopen named ledger filters.

### Acceptance criteria

**Status: implemented and verified on 2026-11-26.** See the root [`README.md`](../README.md) for commands and results. The repeatable browser test is `frontend/e2e/milestone2.spec.ts`, run by `./frontend/scripts/e2e.sh`.

- On a fresh ledger, find suitable starter categories for a dining expense and salary income without creating custom categories.
- Record a ₹250 dining expense and ₹1,000 salary entry.
- See correct account balances and category totals.
- Leave an income or expense entry uncategorized and confirm it appears as **Uncategorized**.
- Archive a referenced category and verify it remains on existing entries and disappears from the picker for new ones.
- Edit the expense to ₹300 and verify balances/totals update.
- Duplicate it and verify a second entry is created.
- Confirm permanent deletion of the duplicate and verify balances/totals return to the expected values.
- Verify a transfer appears once in the main ledger and with the correct direction in each account’s history. Edit and delete it to verify that both accounts update together without affecting income or expense totals.
- Toggle **Reconciled**, then save and reopen a filter by category, account, or status.

## Following milestones

1. **Account types and groups** — assets/liabilities, account grouping, hide toggles, credit-card limit/cutoff/due date.
2. **Budgets** — overall and hierarchical category budgets, monthly periods, rollups, remaining/over states.
3. **Reports and balance sheet** — net earnings, item summary, labels report, prior-period comparison, net worth.
4. **Calendar and reminders** — calendar dots/day drill-down and recurring reminders that require approval before posting.
5. **Splits and attachments** — split editor, receipt image/attachment metadata.
6. **Multi-currency and fees** — per-currency reports, transfer conversion, and a separate expense on the account paying the transfer fee.
7. **Data portability and agents** — CSV export first, followed by a Paisa MCP server for agent inspection. CSV/QIF import and backup/restore remain separate backlog items.
8. **Sync, security, and polish** — QuickSync/Travel Mode, biometric lock, notifications, dashboard customization.

## Open Bluecoins research questions for later passes

- How does Bluecoins handle category deletion when transactions already reference it?
- What exact recurrence rules and timezone behavior does the reminder engine use?
- How are transfer fees represented in reports and account balances?
- Which fields are included in CSV/QIF export, and how are transfer pairs serialized?
- How do saved filters, dashboard card customization, and report date ranges persist?
- What is the normal-onboarding default data set versus demo-only sample data?

## Key REA Evidence IDs

- Demo dashboard: `ev_8269de905f88f8fab436a4e8107b3b87f1b2266f574efedbb35b1aabe2bc1240`
- Dashboard reports/accounts: `ev_0d477a1b88481926b75ea4f42de9b4753fdff63b0e5248506ef0bd937359288f`
- Transactions list: `ev_73532576a8346bc5f00e675218bb511bf385d8c286c56a54ff3a18f7a2839b23`
- Expense detail: `ev_6899edb1607af8db8a2fa278874064cbbb6d83847f54e9a88de9182c771247a8`
- Transaction editor: `ev_fd998da37cf8c851736f35e01742e2e9df923b71afc6ee1b4992bc8079d516bf`
- Add transaction: `ev_ff176749910819f70a44f5bbe93a629eca467171d885c6cc00741b635c39a09b`
- Transfer editor: `ev_59dd523ad3e5fb9536733bc03eb02a5f85f2759c452b10b26135f7cda4c471f0`
- Transfer detail: `ev_a436c0786ca79edbbd3499dfa35345b3873c0e3689fea5ec822fee504e0463e2`
- Reminders: `ev_d4165679e505e112e0c4855838cd352666d070a3b5e7a9f8a5439e50fcd72e4f`
- Balance sheet: `ev_bad76aaf8ada5f7720f32d61d30af0d495ae82a0d8c336bd32fcb35690cb99b3`, `ev_c4ce5907ef86624f81c30970fbf6206f115b7c250f81335ed7451ceda9d691fd`
- Budget: `ev_b74344134895415a55abf8acad5798dbf4e15ea27a19be0f0ee5ba42fc73c10b`
- Net earnings: `ev_5fbfc8a55ae779d6e2aa98625411a0271d376b0fe38a98e6bd3620edaf58f0c1`, `ev_62861f06b2471bd139708217786a2e0940eb7ec444c130991bfd99642019938e`
- Items summary / labels: `ev_ac759e8e92fb2d797c6cc720d0cc6f844e60dcef7d65d03ebc89a95b32e3bfec`, `ev_f4a96dab46081184264bdbfb246b4ec0138920decccc0086ee1f65ff1f621498`
- Accounts: `ev_1cfeea2b6806fd7d6b69af19e22c20ab7362430e25e2326c204b11664c7af2fc`, `ev_111f51f1152db958162aeac02884d10e7ebb567369e13924fde3c2deb16603be`
- Account editor: `ev_172e201c5b150ff67b1bffec5d9579be49cce21e9a904af39d5c5ab9a91fd1b7`, `ev_ebc86a89211c8047bf54e09002ee7aa62143baff2b3cc4c9db8781a0f994a6ef`
- Categories: `ev_13090e375c25fbf9bda28a5febfe355d70849d3fed08892902645dd589707cbc`, `ev_6b4bd0847fe28ee32add60e6acb714b955d9122c428b1831ada0134fae64286c`
- Calendar: `ev_bb9ca9d787f66726b0e4390fb35ef1a8476b4903410cb64f791eebb16db55885`
- Settings/data management: `ev_b5713416023720c28609fcee960471892ab37734d4ec6d0293990df59776da9d`, `ev_9adcada9a895a0e8c66228cd32f1d42ec7012c835f4fce7e0f5fcbb812c9aae6`
