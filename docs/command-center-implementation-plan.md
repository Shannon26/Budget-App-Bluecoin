# Paisa Command Center implementation plan

Implement the [product plan](command-center-plan.md) in the existing React, Axum, and SQLite application. Begin with member permissions and the manual review-to-save flow so automated capture and transfer approvals use the same write path.

## Repository starting point

| Area | Existing code to build on |
| --- | --- |
| Startup and API | [Axum startup](../backend/src/main.rs) opens SQLite, runs migrations, and binds to loopback. The [router](../backend/src/app.rs) exposes account, ledger, report, and catalog operations. Its application state currently has no member identity. |
| Ledger writes | The [account store](../backend/src/store/accounts.rs) validates account metadata and INR. The [transaction store](../backend/src/store/transactions.rs) stores positive integer paise and distinguishes income and expense by entry kind. The [transfer store](../backend/src/store/transfers.rs) already writes the header and both legs together. |
| Frontend | [App.tsx](../frontend/src/App.tsx) manages navigation and opens Overview. Reuse the [API client](../frontend/src/shared/api.ts), [money helpers](../frontend/src/shared/money.ts), [date helpers](../frontend/src/shared/date.ts), and account, transaction, and transfer editor controls where appropriate. |
| Secrets and tests | The root [ignore rules](../.gitignore) do not currently cover `backend/.env`. The [browser runner](../frontend/scripts/e2e.sh) uses a fixed disposable database path. New Command Center tests need an isolated database through the runner and any test that opens SQLite directly. |

## 1. Add household identity and enforce ownership

- Introduce additive migrations for household membership and nullable account ownership. Use the existing ledger as the household’s ledger. Leave existing accounts unassigned and preserve all financial rows and IDs. Assign ownership when a member creates a new account.
- Resolve the current member on the server through a trusted identity boundary. Use server-injected identities for integration tests. Treat the production sign-in and deployment method as a separate decision. Household operations require a verified member once household access is enabled.
- Add a designated-member assignment flow with a review step. Confirm selected assignments atomically and reject attempts to change an already confirmed owner through later account edits.
- Scope the account and ledger reads for the **My accounts** and other-member views, and enforce household membership on reads, including direct detail requests and CSV exports. Add the permission and owner information that existing screens need to render the right controls.
- Enforce write permission in shared backend paths for both Command Center and the existing APIs. Cover account updates, entry create/edit/delete/duplicate/reconciliation, and transfers. For an edit that changes an account, validate the current and proposed accounts. For transfers, validate both endpoints and prevent direct mutation of cross-member posted transfers. Preserve the existing shared category behavior for both members.

**Exit check.** On a migrated copy, both members can read the existing data. Only the designated member can confirm assignments. The ordinary API and draft path enforce the same write permissions, including when called directly.

## 2. Build private drafts and atomic batch save

- Store one active draft per member. Give rows stable draft IDs, with separate references for existing accounts and proposed accounts. Store unresolved fields, a private prompt transcript, and a draft revision. Apply scope checks on every read, update, and discard.
- Define API operations for loading and editing the current draft, appending a prompt, discarding, and saving a reviewed revision. Use the same draft representation for manual and AI-proposed rows. Record manual field edits so a follow-up can preserve them.
- Extract transaction-aware write operations from the existing account, entry, and transfer stores where needed. Call them from the existing standalone endpoints and the batch endpoint. Reuse existing category, amount, account type, and date validation, including strict dates, active categories for new entries, and the current INR rules.
- Validate the reviewed revision and commit ready rows in one SQLite transaction. Insert confirmed new accounts first, resolve draft references to their real account IDs, and write expenses, income, and same-owner transfers. Add the pending request records in that transaction when the approval flow is available. Any unexpected failure rolls back the whole save.
- Use the member and stable draft ID to identify a Save and persist a receipt independently of the draft. A retry returns that receipt. Use the revision to ensure the rows saved are the rows reviewed. After a successful commit, clear the draft rows and transcript. Keep any failed batch available to correct and retry.
- Resolve relative dates against the prompt’s local calendar date and persist resolved dates using the existing date convention. Parse and compute each amount as integer paise with checked arithmetic. For a new account with a stated current balance, subtract the signed effects of activity the member confirms that balance includes. If that includes a pending transfer, show the balance at Save and the projected balance on approval separately. Ask for clarification when it’s unclear which activity the stated balance includes.
- Scope remembered account nicknames to the member. Persist a nickname only after an explicit choice and link it to an eligible account. An alias for a proposed account can be persisted when that account is successfully created.

**Exit check.** A manual batch creates a new account and records dated activity against it. A forced mid-batch failure leaves the ledger untouched and the draft intact. A repeated Save returns the same receipt and ledger IDs.

## 3. Implement transfer requests, approvals, and reversals

- Add a request record with immutable transfer details, sender and recipient member IDs, status, action timestamps, and a link to the posted transfer when approved. Store the source batch ID and, for a reversal, the original transfer ID. Distinguish the requested ledger date from the approval timestamp.
- Creating the request requires control of the sending account and an assigned receiving account owned by the other member. Validate the details before storing the request. Ledger and report queries continue to read posted transfers and entries, so pending requests have no effect on totals.
- Restrict approve and decline to the receiving member and cancel to the sender. Guard each action by pending status in a SQLite transaction. Approval rechecks the endpoints and invokes the existing transfer writer to post both legs, links the posted transfer, and records the decision in the same transaction. Repeated or competing actions can produce only one terminal decision and at most one posted transfer.
- Route legacy transfer creation through the same permissions. When a cross-member request starts in an existing manual screen, carry it into the Command Center review or use the shared pending-request write path with the same review and approval outcome.
- Protect posted cross-member transfers from direct edits, deletion, and duplicate or reconciliation routes that could bypass the rules. Build a reversal from the original transfer’s accounts and amount, swap the direction, and submit it as a new request. Prevent two posted reversals of the same original. Apply the same check to existing transfers once both accounts have owners.
- Return incoming and outgoing request lists and counts scoped to the member. Keep the request history for approved, declined, and canceled decisions, including the posted transfer link when available.

**Exit check.** A mixed batch saves its ordinary entries and a pending request. The receiving member’s approval posts exactly two transfer legs under the requested date. Decline and cancellation leave balances untouched.

## 4. Add the OpenRouter draft assistant

- Add `backend/.env` to the repository ignore rules before providing a real key. Load `OPENROUTER_API_KEY` and the selected model in the backend and keep both out of frontend configuration. Log only operational metadata such as duration, status, and request IDs. Keep authorization headers, financial prompt text, and raw model responses out of logs.
- Wrap OpenRouter in a backend client with a configurable model and a stub for tests. Choose a compatible endpoint for strict JSON Schema output and require support for the requested parameters. Review endpoint data policies and configure routing accordingly at model selection. [Structured Outputs](https://openrouter.ai/docs/guides/features/structured-outputs), [Provider Routing](https://openrouter.ai/docs/guides/routing/provider-selection).
- Build the request from an explicit allowlist. Include the submitted text, local date context, only the unsaved rows and fields needed for the follow-up, and a small list of relevant permitted saved account and category names when matching warrants it. Resolve saved references and nicknames locally. The assistant path has no ledger history lookup.
- Accept a structured proposal containing new rows, targeted edits to stable draft row IDs, or clarification needs. Validate the output and apply it only to the draft revision it was prepared against. Use a prompt submission ID so a network retry of the same request cannot apply the proposal twice. Ask about ambiguous row references, accounts, dates, balances, or conflicting totals. Preserve manual edits outside the explicit scope of the follow-up.
- Limit request size and time. On missing configuration, an unavailable provider, or invalid output, retain the draft and the submitted prompt. Offer retry and manual editing. A provider response can change the draft only and can never post to the ledger or approve a transfer.

**Exit check.** Stubbed prompts prepare a full day of activity, then update one specific row without overwriting manual changes. An inspected outbound request contains only allowed context. Manual save succeeds during a simulated outage.

## 5. Ship the mobile-first UI

- Add Command Center to the navigation in `App.tsx` and make it the default home screen. Build the composer, **Add manually**, **Add accounts**, **Continue draft**, and incoming transfer badge as the main entry points.
- Build the dedicated review screen with date groups, row editors, multi-row account assignment, pending approval labels, **Add details**, collapsible **Prompt history**, and pinned Save. Reuse existing input controls and formatters. Add responsive behavior for the on-screen keyboard, small screens, and desktop layouts.
- Use the draft revision returned by the API. Keep errors actionable at the relevant row and preserve text across a failed request. Refresh the ledger and request lists after a successful write. Use the receipt’s saved record IDs to take the member to the saved activity in Ledger.
- Add the **My accounts** and read-only other-member views and the designated member’s assignment review. Wire up incoming and outgoing transfer details, action buttons, and the reversal entry point on a posted cross-member transfer.

**Exit check.** Using two browser profiles on an isolated ledger, one member can dictate or type a batch on a narrow viewport, finish review on desktop, and save. The other can see only saved activity and can independently act on incoming requests.

## 6. Verify and prepare the feature for use

| Area | Cases to verify |
| --- | --- |
| Migration and regression | Upgrade a copy with existing accounts, transfers, and reports. Confirm preserved balances and IDs, legacy ownership restrictions, and unchanged ledger/report totals before assignment. Run the existing Rust, frontend, and browser suites and the frontend production build. |
| Daily capture | Test a full day in one prompt, separate prompts in one draft, relative dates around local midnight, a standalone total, a mismatched itemized total, duplicates entered intentionally, manual edits followed by a prompt, and missing or ambiguous account details. |
| New bank accounts | Cover several accounts in one draft, existing-versus-new resolution, missing balance, a signed opening balance, current-balance reconciliation with earlier activity, and review when a pending transfer affects a projected balance. |
| Access and isolation | Test both members across separate browser sessions, including direct API requests, edits that move entries into another owner’s account, transcript isolation, personal nicknames, and access to unowned accounts. |
| Transactions and approvals | Force save and approval failures. Test retries, cancel versus approve, repeated approval, declined requests, both transfer legs, effective dates, and separately approved reversals of both new and existing transfers. |
| Phone experience and recovery | Check keyboard access, focus, rapid row edits, pinned Save visibility, small-screen readability, reload and cross-device resume, and manual completion during a provider outage. |

Use disposable SQLite databases, an OpenRouter stub, and synthetic prompts for automated checks. Verify that `backend/.env` is ignored before configuring a key. Enable actual two-member access once the chosen deployment can supply verified member identities. Hosting and remote connectivity remain separate from feature implementation.
