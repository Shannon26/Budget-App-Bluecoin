# Bluecoins parity notes

## Scope and evidence boundary

This document records what REA confirmed from the supplied Bluecoins APK and what remains unknown. Static decompiler observations are not runtime observations. Later ADB/REA runtime walkthroughs, including the demo CSV export, are recorded separately in [`bluecoins-walkthrough.md`](bluecoins-walkthrough.md).

Target artifact:

- File: `Bluecoins Finance & Budget v13.1.149 b33215.apk`
- SHA-256: `2167e31867af032f5b9e6aa68bc1017048123a5477d39419523859dc54d62000`
- Package: `com.rammigsoftware.bluecoins`
- Version: `13.1.149` (`33215`)
- Min SDK: 24; target SDK: 36
- REA provider: JADX headless 0.7.1

REA Evidence IDs below are session-scoped. The artifact digest is the stable join key.

## Confirmed from the APK

| Area | Confirmed static observation | Evidence |
| --- | --- | --- |
| App shape | One exported launcher `MainActivity`; the manifest declares 63 components (28 activities, 15 services, 16 receivers, 4 providers). Most product UI is therefore behind the single activity rather than separate manifest activities. | `ev_453de85be4ba95736981986c43f2cdde55cea30bfaee45afca8d4d688b8f909d`, `ev_2e41a788a556dc4ec3433736303bd8e9d3452a67d48ce53c7c7b0c77049c1674` |
| Transaction taxonomy | The transaction-type enum contains `System` (1), `NewAccount` (2), `Expense` (3), `Income` (4), and `Transfer` (5). | `ev_b1dedc9222328bc6f94f213885dbb3f786a592bf323c747837aa17068aac8d10`, `ev_a837d5023a582ff8490aaf893c129c0f4ca69c55ba1aecbbbcc6ef1461c9535a` |
| Amount direction | The editor's amount-sign enum contains `Positive`, `Negative`, and `Transfer`. | `ev_2918c92b900cfb827370ed17a3a34f52aa893cef65ae2d88a3f85a4e6df09c5a`, `ev_282d11626160050b45a65c400074dd204af183ec10adc9297acc64cca0070ce5` |
| Account selection | The editor models three selection targets: `Account`, `AccountPair`, and `AccountTransferFee`. It also exposes events for setting an account, setting an account pair, swapping accounts, and toggling a transfer fee. | `ev_8ef36084e0ced21e73cd643e006e73488c14bc3eacec89aba711ec97af8675d8`, `ev_d8e15a314d3f5d721204c96fc0fde3b25f23f79a4d7e55744ee8a8166ca52153a`, `ev_bf601f2baf35d9d17d67aec5bd39d4cd2f2b99e51c7362b6de11ce5d03b291b8`, `ev_bf076c90ce9b2c00ce5eee088d1409b14fbbd08d559f3d11356e04db15800561` |
| Transfer direction | A persisted transfer direction enum contains `From` (1), `To` (2), and `NewAccount` (3). | `ev_2e10a322f4a6586a3ad4bd27e35aa32582538b854f6bfcc4b8f76042adce2bc3`, `ev_46037b2cb3f1359c68cc2f48b084e547e455e1f21ba6f971a3b58ad1d2648634` |
| Persisted transaction shape | The decompiled transaction record has two account IDs, a transfer direction, a primary `double` amount, an optional second `double` amount, a split list, status, category ID, timestamps, labels, notes, recurring data, and a nullable long that can link records. Exact field names are obfuscated, so not every semantic mapping is proven. | `ev_0964f4382e62c8a1b9285e4b1b200c484bf928b16f61e3968226aa8f4337b623`, `ev_78dc1ae42d23e6f46473fb9c39c59fd046229dc01fee3122eebabb72cf2b62e5` |
| Account shape | The account model has a name string, several numeric balances/limits stored as `double`, account-type and status integers, currency-like strings, creation/opening dates, hidden/include flags, and an account-type object. The constructor confirms defaults such as empty names, zero balances, and default type/status values. | `ev_300d57aa5bea194603d16b361364caad21b4f5c3fe0651d83154934f2c1f8e2d`, `ev_0cbf267672a780510246cb78ab66aa88da88cb26ec959116378f1e1caaba62dd` |
| CSV type mapping | Import code maps localized/English `expense` or `e` to Expense, `income` or `i` to Income, and `transfer` or `t` to Transfer. Invalid type text raises `TransactionTypeError`. | `ev_e46158b0f99b2524ae57daf47052fcb54f13041c6588bf6bb2d67c0f59da7695` |
| CSV transaction fields | The import bean includes account, account type, amount, category, category parent, conversion rate, currency, date, labels, name, notes, split, status, and type. | `ev_2f62e727c8f31d6c4b719aa89ab617dec10f857b5a4a8b1a1df53172361ee40a` |
| Transfer pairing during import | Import logic explicitly identifies Transfer rows and has a `TransferPairError`, confirming that transfer rows are treated as a paired concept during import. This does not prove the database write is atomic. | `ev_0aed107fbdbd9107ec61dfcaeae7cc52a18ff5ffe20807e65f1ca404b43d0307`, `ev_a4385d187984b67ea224e786bc0cb3ea4fee0e5942b06644d4d3937beb17c432` |

## Android observation

The physical USB device reconnected and was ready for observation:

- Serial `<device-serial>`, model <android-device>, ADB state `device`, transport ID 6.
- Android 13 / SDK 33; `ro.debuggable = 0`.
- USB debugging and developer options were enabled.
- Installed package `com.rammigsoftware.bluecoins`, version `13.1.149` (`33215`), installer `pc`, first installed `2026-11-26 18:06:47`. Evidence: `ev_170992c2def8c2d52e4043137613e7aaafd005157150d7c532869a7d4342dcb3`, `ev_35eeec5e5904890383edfd0e8cf0a23779f9b63eaf57e635fe2c041cb13226a8`.
- The installed base APK was pulled read-only and its SHA-256 exactly matched the analyzed local artifact: `2167e31867af032f5b9e6aa68bc1017048123a5477d39419523859dc54d62000`. Evidence: `ev_7624e0e94e489b7fbae01202787e5074fa95170ca17a28a9c96898de7d0b683c`.
- The initial screen was the phone home screen. After launching `MainActivity`, the focused window was Bluecoins and the app displayed its first-run welcome screen: Bluecoins branding, “Editor's Choice”, “Get started”, and an English language selector. No financial sample data was present. Launch changed only foreground app state; no transactions or accounts were created. Evidence: `ev_7cf4041ed642534221baae536ef59c9fb5ad2a0ad2453b817b5f9636fda86a48`, `ev_5946a410f0265aa2b23c6c759afc5881e85e58b315ec793779c0800b4596d8bf`, `ev_6a8649f1c00e8efc7cf434c112f7a6d0b207704eb64425ac4864b263d304a66e`.
- Display: 1080×2400, physical density 480, override density 408. Evidence: `ev_7365c4578f48aa84a35717aeb47c4b564804438af3dc168c6b8945da9d5bda9b`.
- Direct access to `/data/data/com.rammigsoftware.bluecoins` was denied by Android on this user build, so runtime database/preferences inspection was unavailable.

An earlier capture batch failed while the phone was disconnected; the reconnected checks above supersede that gap. The app was then advanced through onboarding, INR was selected, and **Run Demo** created simulated sample data for a full runtime walkthrough. The demo remains active on the device. Runtime observations, Evidence IDs, and the recommended next-build plan are recorded in [`bluecoins-walkthrough.md`](bluecoins-walkthrough.md). No real financial data was used or modified.

## Design choices for this milestone

These are choices for the new app, not claims about Bluecoins internals:

- Store money as signed 64-bit integer minor units (paise), never floating point.
- Store an account opening balance separately from ledger entries.
- Store income and expense as single ledger entries.
- Store a transfer header plus two ledger entries: `transfer_out` and `transfer_in`.
- Insert the transfer header and both legs in one SQLx transaction.
- Compute balances as opening balance + income − expenses − transfer out + transfer in.
- Compute income and expense totals only from `income` and `expense` entries, so transfers cannot change them.
- Reject zero/negative amounts, same-account transfers, missing accounts, and cross-currency transfers in this milestone.
- Accept INR only in the UI and API until totals can be reported per currency.

## Open questions

1. What is Bluecoins' exact SQLite/Room schema, and does its transfer write use a single database transaction? Static class shape alone cannot establish atomicity.
2. Which obfuscated `Account` double fields are opening balance, current balance, credit limit, and transfer-fee amount?
3. How are multi-currency transfers converted, and when is the conversion rate captured?
4. How do transfer fees affect the destination amount, account balances, and expense reports?
5. What are the exact report filters that exclude transfers from income/expense totals? The enum and paired model support the design, but runtime report behavior was not observed.
6. How do splits, recurring transactions, reminders, and imports behave end to end? CSV export and the budget table/month controls were later observed at runtime; CSV import and Bluecoins' internal budget storage remain unverified.
7. What default accounts/categories does normal onboarding create compared with the demo data set, and what happens to demo data when “Exit Demo” is chosen?
8. Which UI labels, validation messages, and edit/delete undo flows should be mirrored for long-term parity?

## Next milestone backlog

- Budget recurrence/custom-period internals beyond the observed monthly table and preset month switching; Paisa monthly category budgets are implemented.
- Split transactions.
- Recurring transactions and reminders.
- Reports and date-range filters.
- CSV/QIF import and backup/restore; CSV export is implemented and documented in the runtime walkthrough.
- Account editing/archiving and transaction edit/delete.
- Categories, labels, attachments, and transaction status.
- Multi-currency accounts, conversion rates, and transfer fees.
- Runtime parity verification on a reconnected phone with controlled sample data.
