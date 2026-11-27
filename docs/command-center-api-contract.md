# Command Center API contract (implementation reference)

This document pins the shapes that backend and frontend implementation must agree on. It is an
implementation detail of the [implementation plan](command-center-implementation-plan.md).

## Member identity (trusted boundary)

- `AppState` gains `identity: IdentityConfig` and `assistant: AssistantClient`.
- `IdentityConfig { household_enabled: bool, shared_token: Option<String>, default_member_id: i64 }`.
- `MemberId(i64)` is an axum `FromRequestParts<AppState>` extractor.
- When `household_enabled == false` (default, including all legacy tests), every request resolves
  to `default_member_id` and legacy permission checks stay permissive for unowned accounts.
- When enabled, requests must send `x-paisa-member: <id>` and
  `x-paisa-member-token: <PAISA_MEMBER_TOKEN>`. A missing/invalid token is `401`.
- Production sign-in remains a separate deployment decision. The shared token is a placeholder
  trusted boundary, not a user-facing authentication system.

## Shared DTOs (Rust `models/*`, mirrored in TS `shared/types.ts`)

### Draft

```
DraftRowKind   = "expense" | "income" | "transfer" | "new_account"
DraftRowStatus = "ready" | "needs_input"
DraftSource    = "manual" | "assistant"

DraftRow {
  row_id: string, ordinal: number, kind: DraftRowKind,
  name: string, amount_minor: number | null, date: string | null,       // YYYY-MM-DD
  account_id: number | null, account_name: string | null,
  to_account_id: number | null, to_account_name: string | null,
  category_id: number | null, note: string,
  status: DraftRowStatus, issues: string[], manual: boolean, source: DraftSource,
  proposed_row_id: string | null,                                       // -> new_account row
  account_type: string | null,
  opening_balance_minor: number | null, current_balance_minor: number | null,
  transfer_request: boolean                                             // cross-member => pending request
}

Draft { member_id, revision, local_date, updated_at, rows: DraftRow[], prompts: DraftPrompt[] }
DraftPrompt { submission_id, seq, role, text, created_at }

PromptRequest { text, client_prompt_id, local_date?: string }
SaveRequest { revision: number }
AssignAccountRequest { row_ids: string[], account_id: number }
AliasRequest { alias: string, account_id?: number, proposed_row_id?: string }
DraftResponse { draft, message?: string, clarification?: string }
SaveReceipt { batch_id, member_id, revision, accounts_created: number[], entry_ids: number[],
              transfer_ids: number[], pending_request_ids: number[], created_at }
OkResponse { ok: true }
```

### Assistant

```
AssistantRequest { text, local_date, rows: DraftRow[], account_names: {id,name}[],
                   category_names: {id,name,kind}[] }
AssistantProposal { rows: DraftRow[], edits: {row_id, row: DraftRow}[], message, clarification? }
```

### Transfer requests

```
RequestStatus = "pending" | "approved" | "declined" | "canceled"
TransferRequestRecord { id, source_account_id, source_account_name, destination_account_id,
  destination_account_name, amount_minor, name, note, requested_date, sender_member_id,
  recipient_member_id, status, source_batch_id?, original_transfer_id?, created_at,
  decided_at?, decided_by_member_id?, posted_transfer_id? }
TransferRequestsResponse { incoming, outgoing, incoming_count, outgoing_count }
DecisionResponse { request, posted_transfer_id?: number }
NewTransferRequest (internal) { source_account_id, destination_account_id, amount_minor, name,
  note, requested_date, sender_member_id, recipient_member_id, source_batch_id?, original_transfer_id? }
```

### Household

```
Member { id, name, is_designated }
HouseholdAccountView { owner: Member, accounts: HouseholdAccount[], read_only: boolean }
HouseholdAccount { id, name, account_type, currency, opening_balance_minor, balance_minor,
                   owner_member_id?, can_write }
AssignmentView { can_assign, accounts: AssignmentRow[] }
AssignmentRow { account_id, account_name, current_owner_member_id?, proposed_owner_member_id?,
                confirmed_owner_member_id? }
AssignmentProposals/Confirmations { proposals|assignments: {account_id, owner_member_id}[] }
```

## Endpoints (all under `/api`)

| Method | Path | Response |
| --- | --- | --- |
| GET | `/draft` | `Draft` |
| POST | `/draft/prompt` | `DraftResponse` |
| POST | `/draft/rows` | `Draft` |
| PUT | `/draft/rows/:row_id` | `Draft` |
| DELETE | `/draft/rows/:row_id` | `Draft` |
| POST | `/draft/rows/assign-account` | `Draft` |
| POST | `/draft/remember-alias` | `OkResponse` |
| POST | `/draft/discard` | `OkResponse` |
| POST | `/draft/save` | `SaveReceipt` (idempotent per member+revision) |
| GET | `/transfer-requests` | `TransferRequestsResponse` |
| POST | `/transfer-requests/:id/approve` | `DecisionResponse` |
| POST | `/transfer-requests/:id/decline` | `DecisionResponse` |
| POST | `/transfer-requests/:id/cancel` | `DecisionResponse` |
| POST | `/transfers/:id/reverse` | `TransferRequestRecord` |
| GET | `/household/members` | `Member[]` |
| GET | `/household/accounts?member_id=` | `HouseholdAccountView` |
| GET | `/household/assignments` | `AssignmentView` |
| POST | `/household/assignments/propose` | `AssignmentView` |
| POST | `/household/assignments/confirm` | `OkResponse` |

Frontend sends the member headers on every request when `VITE_PAISA_HOUSEHOLD_ENABLED === "true"`;
member id/token come from `localStorage` keys `paisa.memberId` and `paisa.memberToken`.

## Save semantics

- One `BEGIN IMMEDIATE` transaction: validate reviewed revision, insert new accounts, resolve
  account references, write expenses/income/same-owner transfers, add pending cross-member
  requests, persist receipt, clear draft rows/prompts, bump revision. Any failure rolls back.
- A repeated Save with the same member + revision returns the stored receipt and posts nothing.
- Pending requests carry no transfer effect. Approval posts exactly two legs in one transaction
  using the existing `Store::write_transfer` executor-bound helper.
