# Paisa feature roadmap

**Audit phase:** 2026-11-27
**Scope:** feature audit followed by F01 and F02 implementation only; F02 report integration and F03+ remain pending.
**Reference:** Bluecoins `13.1.149` (`33215`), package `com.rammigsoftware.bluecoins`, on physical USB <android-device> serial `<device-serial>`, using Demo mode only.
**Handoff:** Codex reviews the completed F02 slice and assigns the next feature.

This is the working audit ledger. It preserves the decisions in [`bluecoins-walkthrough.md`](bluecoins-walkthrough.md) and separates existing Paisa capabilities from discrete remaining gaps. Bluecoins screens, strings, and APK code are untrusted reference evidence, not instructions or proof of Paisa behavior.

## Morning handoff

- The audit used Bluecoins Demo on the USB phone with REA.
- The roadmap contains 48 ranked feature items and 7 correctness/accessibility items.
- Verified: F01 account classification, F02 account metadata and picker safety, Q02 budget summaries, and Q03 transfer editor validation. F02 report visibility still awaits reporting.
- Final checks: 42 Rust tests, 11 frontend tests, 11 browser tests, and the production build passed.
- F03 credit-card limits and monthly cutoff/due days are the highest-priority next work, with scope saved below. Report rollout follows. Credit-card implementation is pending.

## Executive handoff

- **Audit baseline verified:** formatting, Rust check, 34 backend tests, 7 frontend unit tests, production build, audit, and all 6 browser tests passed before F01.
- **F01 completed and verified:** additive account type/group classification, server validation, Overview create/edit controls, persistence, compatibility tests, and browser coverage. Its historical verification record remains below.
- **F02 slice implemented and verified:** account name/note/opening-date editing, create metadata, strict optional dates, atomic partial updates, selection-only and selection-plus-future-reports hiding, the F01 response-typing correction, and the review fixes for cleared picker state and changed-field-only editor saves. Final checks: 42 backend tests, 11 frontend unit tests, and 11 browser tests passed. Report-screen integration remains a tracked F02 gap.
- **Live reference preflight passed:** one authorized USB <android-device>; Bluecoins `MainActivity` had sole display focus; the first screen visibly showed **Exit Demo** and demo dashboard/calendar data before feature captures.
- **Next work is unassigned:** F03 is the next implementation candidate, but no F03 or later work was started. Codex must review and assign the next slice.
- **Important scope correction:** filtered **CSV export** and **monthly category budgets are already implemented**. They appear in the Done baseline, not the missing-feature backlog.
- **Independent track:** Paisa read-only MCP access is F48 and must remain behind the Rust application layer; it is not a Bluecoins parity feature and must not dictate the database choice.

## Status, evidence, and priority rules

- **Done:** the user-facing capability is present in current Paisa source and has relevant automated coverage.
- **Partial:** a useful subset exists, but a named user-facing gap remains.
- **Missing:** no current implementation was found in the audited source.
- **Unverified:** the reference behavior or Paisa boundary is not sufficiently established for an implementation claim.
- **Runtime** means observed on the connected demo device. **Static APK** means shipped-artifact evidence only; it does not prove execution. **Source** means verified in the current Rust/React repository. **Test** means exercised by the baseline suite.
- Open rows receive **P1–P10**, with P1 highest. Dependencies must land before dependent work. Priority reflects user value, prerequisite leverage, correctness risk, and implementation readiness—not estimated hours.

## Non-negotiable Paisa decisions

1. Money remains signed 64-bit integer paise; no floating-point money. INR remains the only currency until per-currency reporting exists.
2. Recurring entries are reminders that wait for explicit user approval before posting.
3. The main ledger shows one combined transfer row; account history shows only the relevant side.
4. A future transfer fee is a separate expense on the paying account, while the transfer header and both legs remain atomic.
5. Referenced or budgeted categories archive instead of disappearing; archived history and budget totals remain visible.
6. Keep current money, date, reconciliation, filter, and CSV semantics: one-row transfers, positive amount magnitudes, two-decimal INR, formula-prefix protection, all applied-filter matches, and no transfer amounts in income/expense totals.
7. Paisa MCP access is a separate read-only application-layer integration; it must not choose or bypass the database layer.
8. Do not infer a database switch, sync design, or security mechanism before deployment/concurrency requirements are known.
9. F01 account groups are organizational metadata only. Classification alone must never change a balance, posted entry, transfer leg, reconciliation state, filter result, or CSV row.
10. F02 opening date is optional account metadata, never a transaction-redating rule. Hide-from-selection affects new entry pickers/defaults only. The selection-plus-reports control hides new pickers now and persists report hiding for future report screens; current ledger, overview, CSV, and balance semantics remain unchanged.

## Device, artifact, and safety observations

### ADB/REA preflight

| Observation | Result | Evidence |
| --- | --- | --- |
| Attached devices | Exactly one physical USB device: serial `<device-serial>`, model/product <android-device>, device <device-name>, ADB state `device`, transport ID 7 | `ev_6721e2a5ee04edd56024c99bc80fc2968ed5c9e6a5913a7149f1785931302728` |
| Android build | Android 13 / SDK 33, security patch `2024-07-05`, `ro.debuggable=0`, arm64-v8a/armeabi-v7a/armeabi | `ev_a5e14545d130d860412547842ddce237317035054a4ea481eacfcfdd9eff4538` |
| Installed reference app | `com.rammigsoftware.bluecoins` `13.1.149` (`33215`), installer `pc`, first installed `2026-11-26 18:06:47` | `ev_35eeec5e5904890383edfd0e8cf0a23779f9b63eaf57e635fe2c041cb13226a8` |
| Display | 1080×2400 physical, density 480, override density 408 | `ev_7365c4578f48aa84a35717aeb47c4b564804438af3dc168c6b8945da9d5bda9b` |
| Foreground before first capture | Sole focus: `com.rammigsoftware.bluecoins/.MainActivity` | `ev_6a8649f1c00e8efc7cf434c112f7a6d0b207704eb64425ac4864b263d304a66e` |
| Demo confirmation before feature captures | First screen showed **Exit Demo**, November 2026 demo calendar, and demo dashboard totals | `ev_c127d34beb249fe035b57397b38c12c0875c5a9613f96dd2cf7510db42eda50b`; `/tmp/opencode/paisa-bluecoins-audit-2026-11-27/screen.png` |
| Foreground at end | Bluecoins `MainActivity` still had sole focus | `ev_6a8649f1c00e8efc7cf434c112f7a6d0b207704eb64425ac4864b263d304a66e` |

The ADB executable was not on the shell `PATH`; the first raw navigation command failed with exit 127 and made no device change. Subsequent reversible taps/swipes used REA's reported absolute ADB path: `~/Downloads/platform-tools-latest-linux/platform-tools/adb`.

### New runtime evidence captured in this audit

All paths below are local REA PNG captures under `/tmp/opencode/paisa-bluecoins-audit-2026-11-27/`. No toggle was changed, no permission was granted, and no backup, import, reset, purchase, login, or sync was performed.

| Screen path | What was observed | Evidence |
| --- | --- | --- |
| `03-navigation-drawer/screen.png` | Main, Calendar, Accounts, Categories, Labels, Trash, Settings, QuickSync, a **Travel Mode toggle**, Tell A Friend, Send Feedback, Support Development, Ask about a feature | `ev_50f54632bee685fd5d7751931d3d4930ac8a089a5eb53567c591c763034da1be` |
| `04-settings/screen.png` | Localization (Language Preference, Currency Setup, Date Settings); General (Appearance and User Experience, Data Management, Online Sync, Transaction Setup, Calendar, Phone Notification, Password and Fingerprint); About/Help | `ev_269e95ae37e0b2e56984f3328bbd9643bd87c52877500bd5e3b935f3a074f52c` |
| `05-notification-controls/screen.png` and `06-reminder-notification-settings/screen.png` | Reminders; Push Notifications/SMS Banking with Bank, SMS, PayPal, Samsung Pay, WeChat examples; **Grant notification permission** button. Tapping Reminders without granting permission left the same screen, so detailed controls are permission-gated. | `ev_a3163f16ad5b88ee5e56b567ffa09023429d0c324097ad891da1fc9e6ae5841f`, `ev_5e56c4411f817e214c7e915d8780d246f394b45ba899a637202308a4259dd4b6` |
| `07-transaction-setup/screen.png` | Transaction Setup groups Item Entry, Category Settings, Account Settings, and Advanced Settings | `ev_67e557d742ee18c450c6bb5eaa1807ec1df343929984d258eece707d1e4167bf` |
| `08-item-entry-settings/screen.png` | Automatic completion, Automatic suggestion, Unnamed transaction, and Save and Add another preferences | `ev_18f5cb38d788fa1bdb90bb7da21c7dfbd87b77e4ae2324cf3a5df3dd6dfb90ef` |
| `09-automatic-completion/screen.png` | Completion can fill Amount, Category, Account, Notes, Labels, and Split Transaction; all were shown selected; Cancel/OK/Clear all controls. Cancelled without saving. | `ev_f2991acb5d155fcf61597fca90fbc07ae13e89135be48e0792d99827baad9677` |
| `10-data-management/screen.png` | Backup and Restore → Phone Storage; Import Data → Excel (.csv), QIF; Reset data | `ev_5d63a0651264521a4add28bab4b92751f16342103d071b6fca35e75e3785b01b` |
| `11-backup-restore/screen.png` | Default backup location `Bluecoin`; Automatic phone backup toggle off; Set Time `8:00 AM`; manual Backup to phone storage and Restore from phone storage | `ev_293cb72e04ac19d6305a3fc051b26ead2178cc7ed545a20c540c47f736c2e5f9` |
| `12-online-sync/screen.png` | Google Drive provider; Link Google Drive off; backup/restore disabled; Upload attachments off; Automatically sync data off; Wifi only off | `ev_365bd2c5cf09017bcd70822441617aaaa54b2ad841f3332df3ecf4cec2e07f98` |
| `13-dashboard-settings/screen.png` | Dashboard visibility toggles for Daily Summary, Calendar, Budget Summary, Net Earnings, **Credit Card Summary**, Net Worth, Cash Flow, Favorite Accounts, and individual favorite accounts | `ev_1871cd4a386fcb4e5272ea3656f51a67e12345d7bb572aa2b8f348614f524985` |
| `14-dashboard-settings-more/screen.png` | Favorite-account list continues with per-account toggles; no reorder control was observed | `ev_68bceea5685c0a3c3f9580c18fdad07162101649a98e56518086d12b5f0d8312` |

### Static APK evidence used only for unresolved behavior

- Local APK SHA-256: `2167e31867af032f5b9e6aa68bc1017048123a5477d39419523859dc54d62000`, matching the documented artifact. Package/version/SDK summary: `ev_176213ec12a4cf289dfdcb7a65a073000179540d44ab74774d32126f1b2df767` (parent `ev_94401b1e621b39169e48ca93847fbfce5a1f0551559179eeb3b843eeab08003d`).
- Manifest declares `POST_NOTIFICATIONS`, `SCHEDULE_EXACT_ALARM`, `RECEIVE_BOOT_COMPLETED`, `USE_BIOMETRIC`, `USE_FINGERPRINT`, `INTERNET`, and `ACCESS_NETWORK_STATE`, plus billing/ads permissions. It declares daily-reminder, due-reminder, credit-card-due alarm receivers, a boot receiver, a notification-listener service, and WorkManager data-sync components.
- `MyNotificationListenerService` has `onNotificationPosted`; static smali shows a push-enabled setting check, group-summary skipping, notification-category filtering, and asynchronous processing. Decompilation fell back to smali and was marked incorrect, so parsing/posting behavior remains **unverified**: `ev_210395b2e4f08fc708941c897f148d6eb312fa941f0341232890380bcc09aaa7`, `ev_93a3a5a47d68e2a890e855af01b1ec4d4f06765c0a9e7f1c35d80dad4ec7773e`.
- Manifest also declares account, add-transaction, calendar, spend-summary, and reminder widgets plus an app-shortcut initializer. Their end-user behavior was not exercised.
- A complete top-level class-name search for `Travel` returned no matches (`ev_cd132c4d60435dede422d2f31d9c2ff77f35b82952090e7c17274dd0fd4f60f3`). Travel Mode semantics therefore remain unresolved; only the navigation toggle is established.

### Prior runtime evidence reused

The existing walkthrough already establishes the major workflows, so they were not recaptured merely to duplicate evidence:

- Dashboard/reports/accounts: `ev_8269de905f88f8fab436a4e8107b3b87f1b2266f574efedbb35b1aabe2bc1240`, `ev_0d477a1b88481926b75ea4f42de9b4753fdff63b0e5248506ef0bd937359288f`.
- Transactions/detail/editor: `ev_73532576a8346bc5f00e675218bb511bf385d8c286c56a54ff3a18f7a2839b23`, `ev_6899edb1607af8db8a2fa278874064cbbb6d83847f54e9a88de9182c771247a8`, `ev_fd998da37cf8c851736f35e01742e2e9df923b71afc6ee1b4992bc8079d516bf`.
- Transfers/fees: `ev_59dd523ad3e5fb9536733bc03eb02a5f85f2759c452b10b26135f7cda4c471f0`, `ev_a436c0786ca79edbbd3499dfa35345b3873c0e3689fea5ec822fee504e0463e2`.
- Reminders/balance sheet/budget: `ev_d4165679e505e112e0c4855838cd352666d070a3b5e7a9f8a5439e50fcd72e4f`, `ev_bad76aaf8ada5f7720f32d61d30af0d495ae82a0d8c336bd32fcb35690cb99b3`, `ev_c4ce5907ef86624f81c30970fbf6206f115b7c250f81335ed7451ceda9d691fd`, `ev_b74344134895415a55abf8acad5798dbf4e15ea27a19be0f0ee5ba42fc73c10b`.
- Net Earnings/Items/Labels: `ev_5fbfc8a55ae779d6e2aa98625411a0271d376b0fe38a98e6bd3620edaf58f0c1`, `ev_62861f06b2471bd139708217786a2e0940eb7ec444c130991bfd99642019938e`, `ev_ac759e8e92fb2d797c6cc720d0cc6f844e60dcef7d65d03ebc89a95b32e3bfec`, `ev_f4a96dab46081184264bdbfb246b4ec0138920decccc0086ee1f65ff1f621498`.
- Accounts/account editor/categories/calendar: `ev_1cfeea2b6806fd7d6b69af19e22c20ab7362430e25e2326c204b11664c7af2fc`, `ev_111f51f1152db958162aeac02884d10e7ebb567369e13924fde3c2deb16603be`, `ev_172e201c5b150ff67b1bffec5d9579be49cce21e9a904af39d5c5ab9a91fd1b7`, `ev_ebc86a89211c8047bf54e09002ee7aa62143baff2b3cc4c9db8781a0f994a6ef`, `ev_13090e375c25fbf9bda28a5febfe355d70849d3fed08892902645dd589707cbc`, `ev_6b4bd0847fe28ee32add60e6acb714b955d9122c428b1831ada0134fae64286c`, `ev_bb9ca9d787f66726b0e4390fb35ef1a8476b4903410cb64f791eebb16db55885`.
- Settings/data management: `ev_b5713416023720c28609fcee960471892ab37734d4ec6d0293990df59776da9d`, `ev_9adcada9a895a0e8c66228cd32f1d42ec7012c835f4fce7e0f5fcbb812c9aae6`.

## Current Paisa source audit

### Implemented surface

- **Backend:** 20 route patterns / 29 method-path operations in `backend/src/app.rs:32-101`. Schema is limited to accounts, transfers, entries, categories, saved filters, and category budgets across migrations `001`–`004`.
- **Accounts:** create INR accounts with signed opening balances, optional note/opening date, hide preferences, and a server-owned account type; view derived current balance, metadata, type/group, and paginated history; atomically edit name/note/date/type/hide fields while preserving omitted fields. Existing/omitted classifications are `unclassified`; legacy opening dates are null. Hide-from-selection and the selection-plus-reports option filter new transaction/transfer pickers and defaults but not historical editors, histories, filters, overview, or CSV. The report portion is stored for future report integration. Liability/card details remain pending. Anchors: `backend/src/models/accounts.rs`, `backend/migrations/005_account_types.sql`, `backend/migrations/006_account_metadata.sql`, `backend/src/date.rs`, `backend/src/routes/accounts.rs`, `backend/src/store/accounts.rs`.
- **Transactions/transfers:** create/detail/edit/duplicate/permanent-delete/reconcile; atomic transfer header plus both legs; one combined main-ledger row and relevant history side. Anchors: `backend/src/routes/transactions.rs:9-59`, `backend/src/store/transactions.rs:8-181`, `backend/src/store/transfers.rs:8-200`.
- **Ledger/filters/CSV:** the Paisa ledger is date-grouped with full-day totals and account history; search name/note; date/account/category/status/amount/Uncategorized filters; stable 200-row pages; named filters; full filtered CSV with stable headers and spreadsheet safety. Per-entry running account balances seen in Bluecoins remain the separate F31 gap. Anchors: `backend/src/store/ledger.rs:8-258`, `backend/src/store/filters.rs:7-101`, `backend/src/csv.rs:4-160`.
- **Categories/budgets:** new hierarchy changes are restricted to category → subcategory (two levels), while legacy deeper hierarchies can still render. Archive safety, monthly category limits, parent rollups, child constraints, archived history, and invalid-date warnings remain intact. Anchors: `backend/src/store/categories.rs:8-320`, `backend/src/store/budgets.rs:54-266`, `backend/src/store/budget_rules.rs:6-75`.
- **Frontend:** four state-based tabs—Overview, Ledger, Categories, Budgets—in `frontend/src/App.tsx:19-159`. Overview has aggregate cards, accounts, category spotlight, recent activity, and create forms. No URL routing, settings, reports, calendar, labels, reminders, or standalone Accounts screen.
- **Editors:** income/expense and transfer create/edit support name, account(s), exact amount, date-only local-noon value, note, reconciliation, and category/type where applicable. No time picker, calculator, labels, splits, attachments, recurrence, transfer fee, or swap. Anchors: `frontend/src/features/ledger/TransactionForm.tsx:19-131`, `frontend/src/features/ledger/EntryEditor.tsx:17-213`, `frontend/src/features/transfers/TransferForm.tsx:20-115`.

### Audit baseline verification (pre-F01)

Verified on 2026-11-27 before F01 application-code changes. The working tree started clean on `main` except for this untracked roadmap. Backend tests use temporary migrated SQLite databases; the browser runner uses disposable `/tmp/opencode/paisa-e2e.db` and stopped its API/Vite processes afterward.

| Check | Result |
| --- | --- |
| `git status --short --branch` before audit | `## main`, clean |
| `cargo fmt --check` | Passed |
| `cargo check --locked` | Passed |
| `cargo test --locked` | **34 passed; 0 failed** |
| `npm test` | **7 passed; 0 failed** |
| `npm run build` | Passed |
| `npm audit` | **0 vulnerabilities** |
| `./frontend/scripts/e2e.sh` | **6 passed** |

The audit baseline count was newer than the historical 31-test count in README; F01 later raised the passing backend count to 37 as recorded below.

### Source/test gaps that affect readiness

- Saved-filter update/delete, transfer duplicate/reconciliation, several transaction validation/not-found paths, account-history filter behavior, legacy cross-currency transfers, aggregate overflow, and migration-upgrade preservation lack direct automated coverage.
- Frontend unit tests cover money helpers only. Browser tests do not directly cover Overview draft persistence, most validation/error paths, all filter controls, saved-filter editing/deletion, transfer duplicate/reconcile, mobile layouts, keyboard/focus behavior, or ledger request races.
- `GET /api/accounts/:id/history` deserializes the full ledger query but passes only limit/offset to the store; other filter parameters are silently ignored (`backend/src/routes/accounts.rs:20-30`).
- Ledger date filters use unvalidated lexical comparison; category filtering is exact rather than descendant-inclusive; `%`/`_` in search act as SQL wildcards (`backend/src/store/ledger.rs:8-98`).
- State/budget reads use multiple independent queries rather than one snapshot; aggregate `SUM`/`i64` addition has no explicit overflow guard.
- Ledger lacks a stale-response guard; Budgets has month-bound sequencing. Budget summary cards can render `₹0.00` from nullish fallbacks while loading or after failure. Transfer edit relies on backend same-account rejection. Dialogs lack Escape/focus trap/restoration. Invalid-date budget warnings do not link directly to Ledger.

## Existing Paisa baseline — Done, not missing work

| User-facing capability | Current Paisa evidence | Status |
| --- | --- | --- |
| Create INR accounts with opening balance, type, optional note/date, and hide preferences; view metadata/type/group/balances/history; atomically edit account metadata without changing posted money history | `backend/migrations/005_account_types.sql`; `backend/migrations/006_account_metadata.sql`; `backend/src/models/accounts.rs`; `backend/src/routes/accounts.rs`; `frontend/src/features/accounts/AccountForm.tsx`; `frontend/src/features/accounts/AccountTypeEditor.tsx`; backend/e2e account tests | Done |
| Date-grouped ledger with daily totals, search, filters, stable pagination, and account history | README “Milestone 2 behavior”; `backend/src/store/ledger.rs`; `frontend/src/features/ledger/Ledger.tsx`; 201-row regression | Done |
| Income/expense create, detail, edit, duplicate, permanent delete, and manual reconciliation | README API table and acceptance flow; `backend/src/routes/transactions.rs`; `frontend/src/features/ledger/EntryEditor.tsx` | Done |
| Atomic transfer create/edit/delete/duplicate/reconcile; one combined main-ledger row and relevant account-history side | README acceptance flow; `backend/src/store/transfers.rs`; `frontend/src/features/transfers/TransferForm.tsx`; rollback tests | Done |
| Two-level editable category hierarchy, optional categorization, Uncategorized fallback, safe archive behavior | README category rules; `backend/src/store/categories.rs`; `frontend/src/features/categories/CategoryManager.tsx` | Done |
| Named saved ledger filters, including create, reopen after reload, and delete | README “Milestone 2 behavior”; `backend/src/store/filters.rs`; `frontend/src/features/ledger/Ledger.tsx` | Done |
| Filtered full-ledger CSV download with stable headers and spreadsheet safety | README “Downloading CSV”; `backend/src/csv.rs`; `frontend/src/shared/api.ts`; CSV e2e regressions | Done |
| Month-specific category/subcategory budgets, parent rollups, remaining/over states, archived history, and stale-response guards | README “Using monthly budgets”; `backend/src/store/budgets.rs`; `frontend/src/features/budgets/Budgets.tsx`; budget e2e regressions | Done |

## Discrete remaining gaps by subsystem

These are the open gaps after crediting the existing CSV and monthly-category-budget work:

- **Accounts:** F01 classification and F02 metadata/hide controls are implemented; remaining work is report-screen integration for hide-from-reports, liability semantics, card fields, and account archive/delete policy.
- **Reports:** shared period aggregation, Net Earnings, Items Summary, Labels report, balance sheet, credit-card summary/statement, trends, report date filters.
- **Dashboard:** configurable card visibility, favorite accounts, daily averages, budget/calendar/card/net-worth/cash-flow cards.
- **Ledger/editor:** time precision, calculator, similar transactions, print/PDF/HTML, bulk edit, running balances, transfer swap, auto-completion/suggestion, save-and-add-another, saved-filter update.
- **Categories/budgets:** category drill-down, overall monthly cap, frequency/custom periods, chart view/presets, archived restore policy.
- **Planning:** calendar and dependent views, recurring reminders with manual approval, labels, splits, attachments.
- **Transfers/money:** separate fee expense, multi-currency/conversion and per-currency reporting.
- **Data/platform:** CSV/QIF import, QIF export, local/online backup and restore, sync, notifications, push-notification ingestion, security, Travel Mode, settings, widgets, support.
- **Agent access:** separate read-only Paisa MCP facade.
- **Correctness:** ledger response races, budget loading/error states, transfer edit validation, ignored history filters, date/search edge cases, accessibility, and test holes.

## Feature inventory and ranked backlog

### A. Accounts, reports, dashboard, and calendar

| ID | What the user can do | Evidence type and concrete path/reference | Paisa status and source anchor | Remaining gap | Priority / rationale | Dependencies | Concrete acceptance check |
| --- | --- | --- | --- | --- | --- | --- | --- |
| F01 | Assign accounts to asset/liability types and logical groups | Runtime Accounts: walkthrough `ev_1cfeea2b6806fd7d6b69af19e22c20ab7362430e25e2326c204b11664c7af2fc`; Paisa implementation and tests are recorded in the F01 implementation record below | **Done.** `backend/migrations/005_account_types.sql`, `backend/src/models/accounts.rs`, `backend/src/store/accounts.rs`, `frontend/src/features/accounts/*` | None in this slice. Groups are organizational only; liability/card semantics remain F03. | **Completed P1** — prerequisite for liabilities, balance sheet, account pickers, and card reporting. | Satisfied: additive migration, server-owned catalog, optional create field, type-only update endpoint. | Passed: populated pre-migration IDs/data preserved; omitted classification defaults to Unclassified; asset/liability round-trips; invalid values rejected; type edits preserve balances, history, entries, transfers, reconciliation, filters, and CSV. |
| F02 | Edit account name, optional note/opening date, hide-from-selection, and selection-plus-future-reports preferences | Runtime New/Edit account: walkthrough `ev_172e201c5b150ff67b1bffec5d9579be49cce21e9a904af39d5c5ab9a91fd1b7`, `ev_ebc86a89211c8047bf54e09002ee7aa62143baff2b3cc4c9db8781a0f994a6ef`; local UI hierarchy `/tmp/opencode/bluecoins_ui_new_account.xml` shows Note, Opening Date, “Hide from account selection,” and “Hide from account selection and all reports” | **Partial.** `backend/migrations/006_account_metadata.sql`, `backend/src/store/accounts.rs`, `frontend/src/features/accounts/AccountTypeEditor.tsx`; metadata editing and both picker-hide modes are implemented, but no report screen consumes the report portion yet | Persisted report-hide preference has no current report consumer; current ledger/overview/CSV/balances intentionally keep their semantics. Account archive/delete remains out of scope. | **P2** — account-editing slice complete; report integration remains pending. | F01 satisfied; F04 and future report consumers F05/F06/F08/F09/F10/F14 must honor the flag. | Passed: legacy migration defaults, create/edit/clear round-trip, strict invalid/leap dates, old type-only writes, atomic omitted-field preservation, unchanged balances/posted dates/history/CSV, selection-only and selection-plus-reports picker/default handling, hidden selections staying cleared across repeated refreshes until explicit choice, preserved visible choices/drafts, changed-field-only editor saves preserving concurrent metadata updates, historical editing/filtering, all-hidden state, and reload persistence. |
| F03 | Model liabilities and credit-card limit/cutoff/due/latest-balance fields | Runtime Edit Account: `/tmp/opencode/bluecoins-walkthrough-45/screen.png`, `ev_ebc86a89211c8047bf54e09002ee7aa62143baff2b3cc4c9db8781a0f994a6ef`; latest balance −₹166.12, credit limit ₹14,000.00, cutoff day 18, due day 2 | **Missing.** F01 classifies liabilities and F02 provides the editor, but the schema/model has no card fields (`backend/migrations/005_account_types.sql`, `backend/migrations/006_account_metadata.sql`, `backend/src/models/accounts.rs`) | No statement semantics or liability sign/display rules. A negative balance or liability group alone is not a liability model. | **P2** — required before balance-sheet/card reporting; prevents invented card math. | F01 satisfied; F02 editing infrastructure available; explicit liability sign/display and statement-period decisions. | Card fields validate and round-trip; current balance remains ledger-derived; cutoff/due dates do not mutate transactions; liability display rules are tested. |
| F04 | Use one shared report-period query for income, expense, net, categories, and prior-period comparison | Runtime Net Earnings: walkthrough `ev_5fbfc8a55ae779d6e2aa98625411a0271d376b0fe38a98e6bd3620edaf58f0c1`, `ev_62861f06b2471bd139708217786a2e0940eb7ec444c130991bfd99642019938e` | **Partial.** Primitives exist in `backend/src/store/mod.rs:90-138`, `backend/src/store/ledger.rs`, `backend/src/store/budgets.rs`; no reusable report API | Separate feature queries can diverge; no prior-period model. | **P3** — reuse prevents divergent totals across reports, dashboard, and calendar. | Validated half-open ranges; transfer exclusion; integer money; invalid-date policy; F02 hide-from-reports integration for report consumers. | Same fixture yields identical income/expense/net/category totals across endpoints/UI; prior period and month/leap boundaries pass; transfers never affect net; report-only hide preferences are honored without changing ledger/CSV semantics. |
| F05 | View Net Earnings with prior/current comparison and category contribution | Runtime Main → Net Earnings; F04 Evidence IDs | **Missing.** No report route/screen; complete route inventory `backend/src/app.rs:32-101`, `frontend/src/App.tsx:19-53` | No period report or comparison. | **P4** — high-value first consumer that validates F04. | F04; category hierarchy. | Selected/prior periods show income, expense, net, category rollups; empty/error states work; transfers excluded. |
| F06 | View ranked item/payee income and expense summary | Runtime Main → Items Summary: walkthrough `ev_ac759e8e92fb2d797c6cc720d0cc6f844e60dcef7d65d03ebc89a95b32e3bfec` | **Missing.** Raw ledger search only; no item aggregation in `backend/src/app.rs` | No ranked item report. | **P5** — lightweight consumer of F04 with immediate user value. | F04. | Fixed-period names aggregate exactly once, sort deterministically, and reconcile to report totals. |
| F07 | View daily summary chart and 7/30-day expense averages | Runtime Main → Daily Summary; new demo capture `ev_c127d34beb249fe035b57397b38c12c0875c5a9613f96dd2cf7510db42eda50b`; prior dashboard `ev_8269...` | **Missing.** Overview has all-time cards only (`frontend/src/features/overview/Overview.tsx:71-214`) | No daily series or averages. | **P5** — strong dashboard value once F04 exists. | F04. | Seven-day fixture totals and both averages match backend values; transfers excluded; empty days render correctly. |
| F08 | View balance sheet with assets, liabilities, account groups, and net worth | Runtime Main → Balance Sheet: walkthrough `ev_bad76aaf8ada5f7720f32d61d30af0d495ae82a0d8c336bd32fcb35690cb99b3`, `ev_c4ce5907ef86624f81c30970fbf6206f115b7c250f81335ed7451ceda9d691fd` | **Missing.** F01 classification exists, but there is no balance-sheet query or liability semantics | No point-in-time asset/liability/net-worth semantics. | **P6** — depends on trustworthy F03 semantics; must not precede it. | F01 satisfied; F02 editing satisfied and report-hide integration pending here; F03; point-in-time balance rules; F04 if date ranges are shared. | Assets − liabilities equals net worth; prior/current cutoffs include only applicable entries; transfers do not change net worth; unclassified accounts are explicit; hide-from-reports is applied. |
| F09 | View credit-card summary and statement reporting by cutoff/due period | Runtime dashboard **Credit Card Summary** toggle: `ev_1871cd4a386fcb4e5272ea3656f51a67e12345d7bb572aa2b8f348614f524985`; account editor prior Evidence | **Missing.** No card-specific report | No statement/payment/outstanding calculations. | **P7** — useful only after card fields and balance rules are correct. | F01 satisfied; F03; statement-period rules; F04 if shared ranges are used. | Fixture entries produce documented statement period and outstanding/payment figures without double counting. |
| F10 | View cash-flow comparison and net-worth trend | Runtime Main dashboard cards; prior dashboard Evidence `ev_8269...`, `ev_0d477...`; new toggles `ev_1871...` | **Missing.** No trend series or snapshot policy | Historical recomputation/snapshot semantics undecided. | **P7** — requires historical correctness, not just charting. | F04; F01 satisfied and F03 for net worth; decide snapshot-vs-recompute policy. | Trend and current point use identical cutoff rules and reconcile to report/balance totals. |
| F11 | Browse a month calendar with transaction markers and selected-day drill-down | Runtime Main → Calendar: walkthrough `ev_bb9ca9d787f66726b0e4390fb35ef1a8476b4903410cb64f791eebb16db55885`; dashboard toggle `ev_1871...` | **Missing.** No calendar screen or month-grid query | No markers/day view. | **P8** — calendar must follow shared report/ledger query work. | F04; validated dates and existing ledger semantics. | Markers and selected-day rows reconcile to ledger; month/leap boundaries and invalid legacy dates are explicit. |
| F12 | Switch calendar among Transactions, Reminders, Net Earnings, and Balance Sheet views | Runtime Calendar view controls; same Evidence ID as F11 | **Missing.** No dependent views | Calendar integration is blocked by reminders/reports/balance sheet. | **P9** — dependent integration, not the first calendar slice. | F11 plus F05/F08/F34. | Each view uses the selected day/period and authoritative totals; unavailable dependencies are not shown as complete. |
| F13 | Create/edit labels and assign them to transactions | Runtime transaction editor Labels field: walkthrough `ev_fd998da37cf8c851736f35e01742e2e9df923b71afc6ee1b4992bc8079d516bf`; automatic-completion Labels option `ev_f299...` | **Missing.** No label table/model/API/UI in migrations or `backend/src/app.rs` | Labels cannot be assigned or managed. | **P6** — cross-category organization unlocks filters and reports. | Additive label model; transaction association; CSV/report inclusion policy. | Label CRUD/assignment works; existing transactions remain valid; label data round-trips without changing amounts. |
| F14 | View label totals and “No Labels” | Runtime Main → Labels: walkthrough `ev_f4a96dab46081184264bdbfb246b4ec0138920decccc0086ee1f65ff1f621498` | **Missing.** No label report | No period label aggregation. | **P7** — first report consumer of F13. | F13; F04. | Label totals plus No Labels equal all report income/expense exactly once. |
| F15 | Filter ledger/reports by labels and save/reopen label filters | Runtime Filters screen and editor label field; walkthrough filter/editor rows | **Missing.** `backend/src/models/filters.rs:11-32` has no labels | No label filter semantics. | **P8** — extends existing applied-filter behavior. | F13; preserve applied-vs-draft and saved-filter semantics. | Save/reopen a label filter; visible ledger and CSV use the same applied filter and all matches. |
| F16 | Toggle dashboard cards and favorite accounts | Runtime Main Dashboard settings: `ev_1871cd4a386fcb4e5272ea3656f51a67e12345d7bb572aa2b8f348614f524985`, `ev_68bceea5685c0a3c3f9580c18fdad07162101649a98e56518086d12b5f0d8312` | **Partial.** Static Overview only (`frontend/src/features/overview/Overview.tsx`) | No card/favorite preference persistence. Reordering was not observed and is not assumed. | **P8** — polish after real card data sources exist. | F04/F05/F08/F09 as cards are enabled; preference persistence design. | User can enable/disable observed card types and favorite accounts; reload persists choices; missing data has an explicit state. |

### B. Categories, budgets, ledger, editor, and transfers

| ID | What the user can do | Evidence type and concrete path/reference | Paisa status and source anchor | Remaining gap | Priority / rationale | Dependencies | Concrete acceptance check |
| --- | --- | --- | --- | --- | --- | --- | --- |
| F17 | Set an overall monthly expense budget | Runtime Main → Budget Summary → Budget Report: walkthrough `ev_b74344134895415a55abf8acad5798dbf4e15ea27a19be0f0ee5ba42fc73c10b` | **Partial.** Category/month budgets and overall spending exist in `backend/src/store/budgets.rs`; no overall limit | No overall cap/left/over state. | **P7** — discrete budget gap; current category budgets stay unchanged. | Existing monthly expense aggregate; limit persistence/validation. | Overall spent counts each raw expense once; left/over updates after edits; month reload persists the cap. |
| F18 | Choose budget frequency and custom/report date-range presets | Runtime category editor Enable budgeting/Frequency/Budget; Budget Report Last Month/This Month/Last 30 Days/custom | **Partial.** Only independent `YYYY-MM` category limits (`frontend/src/features/budgets/Budgets.tsx`) | No frequency, preset, or arbitrary-period semantics. | **P9** — broader period model needs design; current monthly behavior is correct. | F04; explicit recurrence/copy policy. | Preset/custom periods use correct half-open ranges; existing monthly limits/totals remain unchanged. |
| F19 | Drill into a category with period totals, chart/table, filter/print, and add transaction | Runtime Categories → category detail: walkthrough `ev_13090e375c25fbf9bda28a5febfe355d70849d3fed08892902645dd589707cbc`, `ev_6b4bd0847fe28ee32add60e6acb714b955d9122c428b1831ada0134fae64286c` | **Partial.** Category manager exists (`frontend/src/features/categories/CategoryManager.tsx`); no drill-down | No category report/chart/print or preselected add action. | **P8** — extends existing category strength. | F04; existing category/ledger filters. | Drill-down total equals filtered ledger; chart/table preserves period; add transaction preselects category. |
| F20 | Enter transaction date/time and use an amount calculator | Runtime transaction editor: walkthrough `ev_fd998da37cf8c851736f35e01742e2e9df923b71afc6ee1b4992bc8079d516bf` | **Partial.** Date-only local-noon input and exact amount field (`frontend/src/shared/date.ts`, `TransactionForm.tsx`) | No time picker/calculator. | **P7** — improves central workflow without changing money semantics. | Existing date validation; decide timezone display/storage policy. | Accepted date/time round-trips exactly; calculator result uses integer paise; invalid/fractional-paise input is rejected without writes. |
| F21 | Inspect similar transactions from detail | Runtime Transactions → detail: walkthrough `ev_6899edb1607af8db8a2fa278874064cbbb6d83847f54e9a88de9182c771247a8` | **Missing.** No similar-transaction query | No matching/suggestion list. | **P9** — useful but lower prerequisite leverage. | Define match rules; existing ledger query. | Matching fixture returns documented name/category/account candidates deterministically and excludes the current entry. |
| F22 | Print/export a detail or report as PDF/printer/HTML | Runtime Transactions → Print; walkthrough transaction row and CSV export section | **Partial.** Ledger CSV export is Done; no print/PDF/HTML | Additional output formats absent. | **P9** — CSV already covers the agreed sharing workflow. | Explicit format/scope decision; reuse report queries. | Output contains selected scope and totals, handles special text safely, and does not mutate data. |
| F23 | Bulk edit multiple transactions | Runtime Transactions bulk-edit affordance; walkthrough `ev_735325...` | **Missing.** No bulk endpoint/UI | No multi-select edit. | **P10** — broad validation/atomicity scope. | Bulk selection and validation design; preserve permanent-delete decision. | Previewed bulk changes are atomic; invalid rows block the batch; balances/totals reconcile afterward. |
| F24 | Swap transfer source and destination | Runtime transfer editor: walkthrough `ev_59dd523ad3e5fb9536733bc03eb02a5f85f2759c452b10b26135f7cda4c471f0`; APK static swap event in parity notes | **Missing.** `frontend/src/features/transfers/TransferForm.tsx` has no swap | No one-action direction reversal. | **P7** — small usability gap in an existing core workflow. | Existing transfer validation/atomic update. | Swap reverses endpoints, preserves amount/date/note/status, updates both balances atomically, and leaves income/expense totals unchanged. |
| F25 | Set a transfer fee and record it as a separate expense on the paying account | Runtime transfer editor fee affordance: `ev_59dd...`; APK static fee toggle/account target in `docs/bluecoins-parity.md` | **Missing.** `backend/src/models/transfers.rs:3-16` has one amount; both legs copy it | No coupled fee behavior. | **P9** — preserves agreed fee semantics; needs category/report treatment. | Explicit fee category/account rule; atomic transfer+fee design. | Transfer and fee remain distinct; fee appears once as expense on paying account; transfer legs and fee commit/rollback together. |
| F26 | Split a transaction across categories/accounts | Runtime editor Split affordance: `ev_fd998...`; static split list in parity notes | **Missing.** One account/amount/category per entry | No split child model/editor/report/CSV rules. | **P9** — broad model change; defer until reports stabilize. | Split schema; budget/category/report/CSV inclusion rules. | Split total equals transaction amount; each split contributes once; CSV semantics are explicit and tested. |
| F27 | Attach receipts/files to a transaction | Runtime editor Attachment affordance: `ev_fd998...`; sync Upload attachments toggle `ev_365...` | **Missing.** No attachment metadata/storage | Storage/privacy/backup policy unresolved. | **P10** — requires platform and data-lifecycle decisions. | Local storage policy; DB metadata-only if chosen; backup/sync policy. | Attach/detach preserves transaction; missing file is explicit; exports/backups follow documented policy. |
| F28 | Auto-complete or suggest transaction fields from history | Runtime Settings → Transaction Setup → Item Entry: `ev_18f5...`; completion options `ev_f299...` | **Missing.** No entry-preference or suggestion logic | No automatic fill/suggestion. | **P8** — high daily-entry value after core data is stable. | Define suggestion source/match rules; F13/F26 when labels/splits are included. | User can enable fields; suggestions are previewable/overridable; no save occurs until explicit confirmation; amounts remain exact paise. |
| F29 | Choose a preferred name for unnamed transactions | Runtime Item Entry → Unnamed transaction: `ev_18f5...` | **Partial.** Backend blank-name fallback uses note, then type (`backend/src/store/mod.rs:27-55`) | No user-configurable default. | **P8** — small entry-workflow improvement. | Preference persistence; preserve existing fallback when unset. | Configured default appears for blank names; unset behavior remains compatible; CSV/detail show the stored name. |
| F30 | Save and add another while clearing selected parameters | Runtime Item Entry → Save and Add another: `ev_18f5...` | **Missing.** Forms submit once; Overview drafts have different persistence semantics | No configurable clear-after-save workflow. | **P8** — improves batch entry but must not break draft preservation. | Explicit cleared-field configuration; preserve Overview draft rules. | After save, only configured fields clear; amount/date/account defaults follow settings; no duplicate submit occurs. |
| F31 | Show account running balance on ledger/history rows | Runtime Transactions list: walkthrough `ev_735325...`; Paisa already has date-grouped full-day totals and account history, but no per-entry running balance | **Missing.** Per-entry running account balance is a separate gap from the completed ledger/day/history work | No running-balance projection. | **P7** — useful reconciliation context. | Defined ordering and opening-balance baseline; transfer-side semantics. | Running balance starts from correct opening balance, applies each relevant side once, and reconciles to current balance at page end. |
| F32 | Rename/edit an existing saved filter | Runtime Filters save/open workflow; README API documents `PUT /api/filters/:id` | **Partial.** Backend update exists (`backend/src/store/filters.rs`); frontend supports create/reopen/delete only (`Ledger.tsx`) | No filter update UI; update/delete backend paths lack direct tests. | **P6** — closes an existing API/UI mismatch cheaply. | Existing saved-filter model; applied-vs-draft semantics. | Rename/edit persists after reload; reopened filter drives ledger and CSV; delete removes it without affecting transactions. |
| F33 | Use Trash/undo for deleted transactions | Runtime navigation Trash: `ev_50f54632bee685fd5d7751931d3d4930ac8a089a5eb53567c591c763034da1be`; evidence establishes the entry point only—retention/undo behavior was not captured | **Missing.** No soft-delete/trash in Paisa schema/routes | Reference retention/undo semantics were not captured; may conflict with agreed permanent delete. | **P10** — requires explicit product decision before implementation. | User decision on trash retention; restore/atomicity rules. | If adopted, restore recreates exact balances/history; existing permanent-delete confirmation behavior is not silently changed. |

### C. Reminders, data portability, settings, platform, and agent access

| ID | What the user can do | Evidence type and concrete path/reference | Paisa status and source anchor | Remaining gap | Priority / rationale | Dependencies | Concrete acceptance check |
| --- | --- | --- | --- | --- | --- | --- | --- |
| F34 | Schedule recurring reminders and approve due entries before posting | Runtime Main → Reminders: walkthrough `ev_d4165679e505e112e0c4855838cd352666d070a3b5e7a9f8a5439e50fcd72e4f`; APK due-reminder receiver in `ev_1762...` | **Missing.** `POST /api/transactions` posts immediately; no schedule/queue/approval model | No recurrence, due queue, approve/dismiss/snooze, or timezone policy. | **P8** — high value but must preserve manual approval. | Recurrence specification; F04/date helpers; no auto-post. | Due reminder creates no entry until approval; approve once creates exactly one correct entry; dismiss/snooze is explicit and tested. |
| F35 | Import CSV with mapping, transfer pairing, preview, and line errors | Runtime Settings → Data Management → Excel (.csv): `ev_5d63...`; static CSV fields/pairing in `docs/bluecoins-parity.md` | **Missing.** README explicitly says CSV import is not implemented | No importer. | **P9** — useful portability after export semantics are stable. | Import spec; atomic transaction/transfer writes; formula-safe handling. | Valid fixture imports exact amounts/dates/categories; invalid rows give line-specific errors with no partial write unless explicitly designed. |
| F36 | Import/export QIF | Runtime Data Management → QIF: `ev_5d63...`; evidence establishes the menu entry only—end-to-end QIF behavior was not observed | **Missing.** No Paisa QIF implementation | Format and transfer/account mapping remain unverified. | **P10** — needs format/recovery design. | F35 conventions; QIF mapping spec. | Disposable QIF round-trip preserves documented fields; unsupported/corrupt records fail safely. |
| F37 | Back up and restore locally, including scheduled backup | Runtime Phone Storage: `ev_293cb72e04ac19d6305a3fc051b26ead2178cc7ed545a20c540c47f736c2e5f9` | **Missing.** Ledger CSV is not a full backup; no backup/restore schema/versioning | No accounts/budgets/filters/categories/settings backup or restore safety. | **P10** — destructive recovery workflow needs careful design. | Versioned backup format; F35 import lessons; restore transaction. | Restore round-trips a disposable backup exactly; incompatible/corrupt files fail without overwriting current data; automatic schedule is testable. |
| F38 | Link a cloud provider and configure QuickSync/automatic/Wi-Fi-only sync | Runtime Online Sync: `ev_365bd2c5cf09017bcd70822441617aaaa54b2ad841f3332df3ecf4cec2e07f98`; APK WorkManager data-sync component `ev_1762...`; login/sync was not attempted, so provider/conflict behavior is unverified | **Missing.** No Paisa sync/device identity/change log/conflict model | Provider auth, conflict resolution, offline behavior, and attachment sync remain unresolved. | **P10** — requires deployment/identity/conflict product decisions. | Hosting/identity/conflict design; database decision remains open. | Documented conflict cases resolve deterministically on disposable accounts; offline/failure recovery is tested; no silent data loss. |
| F39 | Configure reminder, credit-card, and system notification controls | Runtime Phone Notification: `ev_a316...`, `ev_5e56...`; APK daily/due/card alarm receivers and boot receiver `ev_1762...`; detailed controls are permission-gated and delivery semantics are unverified | **Missing.** No Paisa notification preferences/delivery | OS permission, exact-alarm, boot, and delivery semantics remain unresolved. | **P10** — platform permission and delivery design required. | F34 for reminder events; F03 for card reminders; platform decision. | Permission denial is handled; enabled notification maps to one due event; no notification auto-posts. |
| F40 | Import transactions from bank/payment push notifications | Runtime Push Notifications/SMS Banking label: `ev_a316...`; static listener `ev_210...`, `ev_93...`; smali fallback was marked incorrect, so parsing/posting behavior is unverified | **Missing.** No Paisa listener/parser | Supported providers, parsing, confirmation, duplicates, and privacy remain unknown. | **P10** — high-risk data ingestion; do not infer from label alone. | Explicit provider/parser spec; user confirmation; duplicate protection. | Only approved notification sources are parsed; preview requires confirmation; duplicate notification cannot double-post; sensitive content is not retained unnecessarily. |
| F41 | Protect app data with password/fingerprint/biometric lock | Runtime Settings → Password and Fingerprint: `ev_269...`; APK `USE_BIOMETRIC`/`USE_FINGERPRINT` `ev_1762...` | **Missing.** No app lock | Threat model and platform fallback unresolved. | **P10** — security design required. | Deployment/platform decision; no lock-setting changes during research. | Lock/unlock works on disposable data; denial/fallback does not expose ledger; credentials are not stored in plaintext. |
| F42 | Toggle Travel Mode | Runtime navigation toggle: `ev_50f...`; static class-name search found no `Travel` class: `ev_cd...` | **Unverified.** Toggle exists; meaning/effects unknown. No Paisa implementation | No established behavior or acceptance semantics. | **P10** — do not guess or mutate timezone/money behavior. | Focused reference research or product decision. | Criteria can be finalized only after behavior is established; default must not alter transaction dates or amounts. |
| F43 | Configure language, currency/date, appearance, calendar, and advanced preferences | Runtime Settings groups: `ev_269...`; Transaction Setup: `ev_67...` | **Partial.** Fixed INR/date presentation; no settings screen | No preference model or UI; multi-currency remains intentionally deferred. | **P9** — broad surface; keep current money/date/CSV semantics stable. | Preference persistence; F01 satisfied and F24 where relevant. | Reload persists preferences; existing API/CSV formats remain compatible; unsupported currencies are not exposed. |
| F44 | Use help, feedback, tell-a-friend, and support links | Runtime drawer: `ev_50f...`; Settings Help: `ev_269...` | **Missing.** No help/support surface | No destinations or privacy-safe feedback flow. | **P10** — low prerequisite leverage. | Support destination and data-sharing decision. | Links open documented destinations; feedback does not silently attach financial data. |
| F45 | Use home-screen widgets and app shortcuts | Static APK manifest/widget receivers and shortcut initializer: `ev_1762...`; evidence is manifest-only and runtime behavior was not exercised | **Missing.** No Paisa widgets/shortcuts | Widget types, refresh, quick-add, and privacy behavior remain unverified. | **P10** — platform-specific surface after core data is stable. | Platform decision; read-only/widget data contract. | Widget/shortcut shows only configured data; quick-add uses normal validation; lock/privacy behavior is explicit. |
| F46 | Use multi-currency accounts, conversion rates, and per-currency reports | Runtime account/editor currency and CSV Exchange Rate vocabulary; walkthrough CSV section; parity open questions | **Missing.** INR-only API/UI validation is intentional | No currency/rate model or conversion timing. | **P10** — large correctness scope; must not weaken current semantics. | Currency/rate model; per-currency totals; F04. | INR behavior unchanged; mixed-currency totals never summed without conversion; rate provenance round-trips. |
| F47 | Restore or otherwise re-activate an archived category | Runtime category management prior Evidence; Paisa archive policy in walkthrough; reference deletion/restore behavior was not observed | **Missing.** Archived categories are read-only in `frontend/src/features/categories/CategoryManager.tsx`; backend rejects edit/new assignment/new budget | No restore path; reference behavior remains an open research question. | **P10** — requires policy decision; archive safety is already correct. | Explicit restore rules and budget/history checks. | If adopted, restore never changes historical totals or assignments and respects existing budget constraints. |
| F48 | Expose read-only Paisa MCP queries through the Rust application layer | Paisa-specific README deferral and walkthrough decision #10; not a Bluecoins feature | **Missing.** No MCP/JSON-RPC/tool registry/scope enforcement; existing REST router mixes reads/writes (`backend/src/app.rs`) | No safe agent facade or authorization boundary. | **P8, independent track** — valuable agent access but separate from parity/database choice. | Stable read-only query service; allowlisted fields; no direct DB exposure. | MCP client can read authorized summaries; every mutation/export/secret-bearing path is refused; tests prove no DB writes. |

### D. Current-scope correctness and accessibility gaps

These are not claims that CSV export or monthly category budgets are missing. They are discrete readiness fixes in existing workflows.

| ID | What the user experiences | Source evidence | Status / gap | Priority / rationale | Dependencies | Concrete acceptance check |
| --- | --- | --- | --- | --- | --- | --- |
| Q01 | Rapid filter/history changes can show stale ledger rows/totals | `frontend/src/features/ledger/Ledger.tsx:93-120`; Budgets sequencing at `Budgets.tsx` | **Partial.** No ledger request sequence/abort guard | **P3** — visible correctness race in a core screen. | Existing ledger API; no schema change. | Delayed older response cannot replace newer filter/history results; Load more remains deduplicated. |
| Q02 | Budget loading/failure can look like zero spending | `frontend/src/features/budgets/Budgets.tsx:187-205,263-275`; budget Playwright selected-month race case | **Done.** Summary amounts show Pending/Unavailable and expense count shows pending/unavailable until a successful selected-month response; real zero remains ₹0.00 and 0 expenses | None. Request sequencing, disabled controls, rollups, and monthly semantics are unchanged. | **Completed P3** — removed misleading financial zero states. | Existing budget response shape; satisfied. | Passed delayed, failed, stale-response, and real zero-month browser checks; `npm test` 11 passed, build passed, browser suite 11 passed. |
| Q03 | Transfer edit can briefly permit same source/destination | `frontend/src/features/ledger/EntryEditor.tsx:45-83,143-228`; existing transfer-edit Playwright flow | **Done.** Matching From/To selections are retained, an inline alert is shown, save is disabled, and submit is guarded before any PUT; restoring a different destination re-enables save | None. Server guard and transfer/history/CSV semantics remain unchanged. | **Completed P3** — prevents invalid client submits without rewriting selections. | Existing transfer validation; satisfied. | Passed focused matching-account alert/disabled-save/recovery flow; `npm test` 11 passed, build passed, browser suite 11 passed. |
| Q04 | Account-history filter query parameters are accepted but ignored | `backend/src/routes/accounts.rs:20-30`, `backend/src/store/ledger.rs:208-213` | **Partial.** API contract silently drops filters | **P4** — hidden correctness/contract bug. | Decide supported history filters; shared ledger query. | Supported parameters produce filtered rows/totals; unsupported parameters are rejected explicitly, not ignored. |
| Q05 | Date/search/aggregate edge cases can violate expectations | `backend/src/store/ledger.rs:8-98`; `backend/src/store/mod.rs:90-129` | **Partial.** Lexical dates, wildcard search, unguarded sum/addition | **P4** — financial correctness and input-safety risk. | Shared date validation; overflow policy. | Invalid dates rejected consistently; literal `%`/`_` search is defined/tested; overflow returns an error without corrupting totals. |
| Q06 | Keyboard/screen-reader users lack full dialog/navigation behavior | `frontend/src/features/ledger/Ledger.tsx:545-625`; `CategoryManager.tsx:220-327`; `App.tsx` | **Partial.** Labels/`aria-modal` exist; no Escape/focus trap/restore/`aria-current` | **P5** — accessibility and safe editing. | Existing markup/test IDs. | Escape behaves consistently; focus enters/returns correctly; background is inert; active tab has `aria-current`. |
| Q07 | Invalid-date budget warning cannot jump to the offending ledger context | `frontend/src/features/budgets/Budgets.tsx:204-216` | **Partial.** Warning is visible/actionable in text only | **P6** — small remediation-path improvement. | Existing Ledger navigation/filter API. | Warning link opens Ledger with a documented invalid-date correction path and preserves budget month context. |

**Q02 checkpoint (2026-11-27 06:51 IST):** Budget summary cards now distinguish pending and failed selected-month loads from real zero-spend months. Browser coverage delays January 2028, fails March 2028, verifies stale responses cannot replace February 2028, and confirms a real empty month shows ₹0.00 with 0 expenses. No backend changes were made.

**Q03 checkpoint (2026-11-27 06:56 IST):** the transfer editor now retains matching From/To values, shows an inline “Choose two different accounts” alert, disables save, and blocks submit before any PUT. The browser regression restores the original destination and completes the existing transfer edit successfully; all 11 browser tests pass.

## Ranked implementation sequence

Priorities are dependency tiers; items within a tier can be parallelized only when their dependencies are already satisfied.

1. **Completed P1 — F01 account type/group metadata.** Foundational classification is implemented and verified; no further F01 work is assigned.
2. **P2 — F02 account editing and hide controls are implemented; report integration remains pending. F03 liability/card fields are next but unassigned.** F01 is satisfied; Codex must explicitly assign F03 before work starts.
3. **P3 — F04 shared report-period aggregation plus Q01–Q03 correctness fixes.** One authoritative report contract and no core-screen races/false zero states.
4. **P4–P5 — F05 Net Earnings, Q04/Q05 query correctness, F06/F07 report consumers, Q06 accessibility.** Validate the primitives with visible reports.
5. **P6–P7 — F08 balance sheet, F13 labels, F32 saved-filter update, Q07 warning navigation, then F09/F10/F14/F17/F20/F24/F31.** Add dependent reports and focused workflow improvements only after their prerequisites.
6. **P8 — F11 calendar, F15/F16 filters/dashboard preferences, F19 category drill-down, F28–F30 entry automation, F34 reminders.** Calendar follows shared queries; reminders preserve manual approval.
7. **P9 — F12 calendar views, F18 budget ranges, F21/F22 detail/print, F25/F26 fees/splits, F35 CSV import, F43 settings.** Broader model/format work.
8. **P10 — F23/F27/F33/F36–F47.** Bulk edit, attachments, trash, QIF, backup/sync/security/notifications/Travel Mode/widgets/multi-currency/archive restore/support require additional product or platform decisions.
9. **Independent P8 track — F48 Paisa MCP.** Can be scheduled separately once the read-only tool contract is approved; it does not block or get blocked by Bluecoins parity features.

## F01 implementation record — completed for Codex review

### Scope delivered

- Additive migration `backend/migrations/005_account_types.sql` adds only `accounts.account_type`, defaulting every existing row to `unclassified`.
- `backend/src/models/accounts.rs` owns the supported type catalog and the only type-to-group mapping. The database stores no group column; `account_group` is derived on every account response.
- Mainstream types: Cash, Checking, Savings, Wallet, Investment, Cryptocurrency, Property, Receivable, Other Asset, Credit Card, Loan, Mortgage, Payable, Other Liability, and Unclassified.
- `POST /api/accounts` accepts optional `account_type`; omitted or null classification becomes `unclassified`. Unknown types return the established HTTP 400 `{ "error": "account type is not supported" }` shape.
- `PUT /api/accounts/:id` changes only `account_type`. It never accepts a client group, never writes entries/transfers, returns HTTP 404 for a missing account, and preserves INR validation and current balance math.
- `/api/state` includes the server-owned `account_types` catalog plus each account's `account_type` and derived `account_group`.
- Overview now has a grouped type selector and live logical-group preview for create, visible type/group badges on every account card, and an inline type editor with save/cancel and usable error feedback.
- Existing history links, ledger/account filters, reconciliation, one-row transfer display, atomic transfer behavior, and the 12-column CSV shape are unchanged.

### Exact final verification commands

| Working directory | Command | Accepted result |
| --- | --- | --- |
| `backend/` | `cargo fmt` | Formatted Rust sources |
| `backend/` | `cargo fmt --check` | Passed |
| `backend/` | `cargo check --locked` | Passed |
| `backend/` | `cargo test --locked tests::accounts` | 5 account tests passed |
| `backend/` | `cargo test --locked` | **37 passed; 0 failed** |
| `frontend/` | `npm test` | **10 passed; 0 failed** |
| `frontend/` | `npm run build` | Passed |
| repository root | `./frontend/scripts/e2e.sh` | **8 passed** against disposable `/tmp/opencode/paisa-e2e.db` |
| repository root | `git diff --check` | Passed |
| repository root | `git diff --no-index --check /dev/null docs/feature-roadmap.md` | No whitespace diagnostics; exit 1 denotes the expected new-file diff |

### Accepted behavior covered by tests

- A populated database migrated through versions 001–004, with explicit account IDs, income, expense, transfer header, and both transfer legs, upgrades to 005 with IDs, names, opening amounts, balances, histories, totals, and CSV unchanged; both accounts show Unclassified.
- Old create requests that omit classification remain compatible and show Unclassified.
- Asset and liability choices round-trip through create, state, and type-only update; a client-supplied contradictory `account_group` cannot override the server mapping.
- Invalid create/update values return HTTP 400 without changing existing classification; missing account updates return HTTP 404.
- Type edits preserve balances, account history, entries, transfer counts, reconciliation state, filters, and CSV bytes.
- Browser coverage includes the empty catalog state, UI create with asset/liability types, legacy Unclassified display, asset → liability → asset edits, reload persistence, unchanged balance/history, and a simulated HTTP 400 inline/global error state.

### Explicitly not implemented

At the F01 handoff, F02 account notes/opening date/hiding, F03 liability/card fields, balance sheet, credit-card reporting, and later items were pending. The F02 record below supersedes only F02's implementation status; F03 and later work remain pending Codex review and assignment.

## F02 verification record — account metadata and staged hiding

**Implemented:** additive migration `006_account_metadata.sql`; optional create metadata; atomic partial account edits for name, note, opening date, type, and hide flags; strict `YYYY-MM-DD` validation through `backend/src/date.rs`; explicit null/empty clearing; Overview create/edit controls; selection-only and selection-plus-reports picker hiding/default invalidation for new transaction/transfer flows; historical editor/filter/history preservation; and persisted report-hide preference. The F01 frontend mismatch was corrected by introducing `AccountMetadata` (no `balance_minor`) for create/update responses while keeping the existing PUT path and type-only writes compatible.

**Review checkpoint (2026-11-27 06:35 IST):** picker state now distinguishes uninitialized defaults from initialized blank/cleared selections, so hidden accounts remain blank across repeated `/api/state` refreshes and source changes until explicit user choice; visible choices and unrelated draft fields are preserved. The account editor snapshots its opening draft and sends only changed fields, so an unrelated concurrent metadata update survives a later edit while explicit nullable clearing still works.

**Destination review verification (2026-11-27 06:42 IST):** setting From equal to an already valid visible To no longer changes To; existing validation reports “Choose two different accounts.” Fresh initialization still waits for From and chooses the first visible different account, while no alternative leaves To unset. Hidden-cleared destinations remain blank until explicit choice. The full browser suite is green at **11 passed**.

| Working directory | Command | Accepted result |
| --- | --- | --- |
| `backend/` | `cargo fmt` | Formatted Rust sources |
| `backend/` | `cargo fmt --check` | Passed |
| `backend/` | `cargo check --locked` | Passed |
| `backend/` | `cargo test --locked tests::accounts` | 9 account tests passed |
| `backend/` | `cargo test --locked` | **42 passed; 0 failed** |
| `frontend/` | `npm test` | **11 passed; 0 failed** |
| `frontend/` | `npm run build` | Passed |
| repository root | `./frontend/scripts/e2e.sh` | **11 passed** against disposable `/tmp/opencode/paisa-e2e.db` |
| repository root | `git diff --check` | Passed |
| repository root | `git diff --no-index --check /dev/null docs/feature-roadmap.md` | No whitespace diagnostics; exit 1 denotes the expected new-file diff |

**Demonstrated behavior:** populated pre-F02 databases retain IDs, names, amounts, balances, histories, and CSV while gaining empty note/null date/false hide defaults; old create and type-only update requests remain compatible; metadata round-trips and clears explicitly; invalid/non-leap dates are rejected without writes while leap dates pass; omitted fields survive partial updates; renames flow through existing history joins; opening-date edits never redatetime entries; selection-only and selection-plus-reports accounts disappear from new pickers/defaults and invalidate stale drafts without being silently reselected across repeated refreshes or source changes; visible choices and unrelated draft fields persist; changed-field-only editor saves preserve concurrent metadata updates; hidden accounts remain in historical editors, account histories, ledger filters, overview, and CSV; all-hidden state is explicit; settings persist after reload.

**Tracked report dependency:** the selection-plus-reports option hides new pickers now; `hide_from_reports` is also persisted and visible but intentionally has no current report consumer. F04 and report features F05/F06/F08/F09/F10/F14 must apply the report portion when built. Current ledger, overview, CSV, ordinary balances, and posted-entry semantics remain unchanged. F02 is therefore **Partial** overall. F03 and all later work remain unassigned and untouched.

## F03 implementation note — prepared, not started

The remaining window is insufficient for a complete, verified F03 slice, so no F03 product code was written. Planned scope: additive nullable card metadata (`credit_limit_minor`, `cutoff_day`, `due_day`); positive-paise and 1–31 validation; card-only create/edit controls using the existing account components; retention of card settings when retyping away and restoration when retyping back; atomic changed-field updates that revalidate the effective type; and ledger-derived latest-balance presentation (negative = owed, positive = credit, zero = settled). Preserve signed balances, purchase/payment behavior, CSV, history, and report visibility. Statement reporting, reminders, scheduling, month-end handling, and balance sheet remain deferred to F09/later work.

## Independent Paisa MCP track (F48)

- **Goal:** let an agent inspect authorized Paisa data through a narrow read-only facade in Rust.
- **Not in scope:** transaction writes, settings changes, exports, raw SQL/database handles, secrets, or choosing a new database.
- **Candidate first tools:** health/capabilities, account summary, filtered ledger summary, category summary. Exact field allowlist and authorization require Codex approval.
- **Acceptance:** mutation and non-allowlisted calls fail closed; read results match REST/application-layer calculations; integration tests prove zero database writes.

## Blockers and unresolved research

- **Notification permission gate:** detailed Reminders controls were not visible; no permission was requested or granted. Prior evidence and APK declarations establish only the entry points and alarm/component types.
- **Login/purchase/sync:** Google Drive linking and any purchase/login workflow were not attempted. Sync conflict and attachment-upload behavior remain unknown.
- **Destructive data flows:** reset, restore, real import, delete, and Exit Demo were not used. Backup format and restore semantics remain unverified.
- **Private app data:** prior direct access to `/data/data/com.rammigsoftware.bluecoins` was denied; runtime database/preferences inspection remains unavailable.
- **Travel Mode:** only the drawer toggle is established; semantics remain unresolved after runtime capture and a bounded class-name search.
- **Reference internals:** exact recurrence/timezone rules, transfer-fee report treatment, category deletion/restore behavior, card-summary calculations, normal-onboarding defaults, Trash behavior, QIF behavior, and saved-filter/dashboard persistence were not fully observed.
- **Paisa product decisions:** liability sign/display, account archive/delete, arbitrary report periods, attachment storage, sync identity/conflicts, notification delivery, security threat model, and MCP authorization remain open.
- **Tooling note:** REA exposes ADB capture/inspection but no general Android UI-input tool, so reversible navigation used the reported local ADB binary. The initial `adb` PATH failure was recovered without installing anything.

## Evidence retention

New screenshots are under `/tmp/opencode/paisa-bluecoins-audit-2026-11-27/`. The complete REA session bundle is exported separately as `/tmp/opencode/paisa-bluecoins-audit-2026-11-27/rea-evidence.json`; Evidence IDs above remain usable from that bundle in a later session.
