# Paisa Command Center product plan

Command Center is Paisa’s highest-priority feature and the default home screen. Its main job is to turn a day’s spending and income into reviewed ledger entries with one save. A household member can describe several purchases at once, add details in follow-up prompts, or fill in rows manually. Adding bank and cash accounts is an occasional task and has a secondary action on the same screen.

See the [implementation plan](command-center-implementation-plan.md) for repository changes, delivery order, and verification.

## First-release scope

- Create expenses, income, transfers, and new financial accounts from the same draft. Existing records are corrected through Ledger, subject to household permissions and the approval rules for transfers between members.
- Support typing and the phone keyboard’s built-in dictation. Design the flow around one-handed phone use and support the same ledger and drafts on desktop.
- Start with two household members. Each has their own accounts and a private active draft. Both can see the other member’s saved accounts and activity.
- Defer conversational spending questions, separate voice recording, and account ownership transfers. Hosting, sign-in setup, and remote access are deployment decisions.

## Screens and interactions

| Screen | What the member sees and does |
| --- | --- |
| **Command Center** | A prominent **Log activity** box and a **Prepare** action. When the box is empty, show one short example such as “Yesterday I paid ₹240 for lunch from HDFC and ₹80 for coffee in cash.” Place **Add manually** and **Add accounts** below the main action. Show **Continue draft** when a draft exists and a **Transfers to review** badge for pending incoming requests. Keep Overview available in navigation. |
| **Review** | A dedicated screen with entries grouped by date. Each row shows the description, amount, account or transfer endpoints, and any issue that needs a decision. Tap a row to open a short editor. Select multiple rows with unresolved accounts and assign the same eligible account to the selection. Keep **Save** pinned within reach and let rows scroll above it. |
| **Follow-up and history** | **Add details** accepts follow-ups in the same draft. A collapsed-by-default **Prompt history** section gives the member access to their own previous messages. **Discard** clears that draft. |
| **Receipt** | Show counts of saved accounts and entries and list transfers submitted for approval separately. Offer **View in Ledger** and a way to start a fresh draft. The receipt reflects what was posted and what still needs approval. |
| **Transfers to review** | Show incoming and outgoing requests and their status. Each incoming request shows the sender, both accounts, amount, effective date, and description, with individual **Approve** and **Decline** actions. An outgoing pending request has **Cancel**. |

On a phone, keep the composer and primary action usable with the keyboard open. The pinned action area should leave enough room to scroll the last row fully into view. Give row actions text labels or accessible names, make unresolved fields identifiable without color, and return focus to the relevant row after editing. On desktop, use the same review order and controls. The layout can show more of each row at once.

## Draft and prompt behavior

- Each member has one active draft that survives refreshes and syncs across their own devices. They can resume with **Continue draft**, paste a whole day’s activity, or build the batch over several prompts. The flow assumes one device editing a member’s draft at a time.
- A follow-up changes only the rows and fields it refers to. Preserve unrelated rows and manual edits unless the member explicitly asks to change them. If a follow-up could refer to more than one row, ask which one before changing it.
- Open questions appear where the decision is needed. The member resolves or removes every incomplete row before saving the batch. Uncertain categories can remain **Uncategorized**.
- If OpenRouter fails, keep the draft and submitted text available. The member can edit or retry the prompt, or add and edit rows manually and save through the same review.
- Keep the prompt transcript in the private draft until a successful save or discard. Saved ledger records and the receipt retain the reviewed record details and batch attribution. The draft transcript is removed.

### Dates, amounts, and categories

- Resolve relative dates such as “yesterday” in the member’s local timezone. Default an undated entry to the local date of the prompt. Show the actual date during review.
- “I spent ₹500 in cash yesterday” is one expense if that’s the only breakdown provided. “₹200 for lunch and ₹300 for groceries, ₹500 total” produces two expenses. The stated total checks the sum. A mismatch requires the member to correct or clarify it.
- Suggest an existing category when the match is clear. The member can change it in review. Both members can manage the shared category list from the existing Categories screen.
- Save every reviewed row, including intentionally identical rows. Paisa treats a retry of the same Save as the same operation.

### Account matching and setup

- Match an explicit account name or remembered nickname when it identifies one eligible account. Ask when more than one account could match. Transfers require both endpoints.
- If an account name is unfamiliar, ask whether to use an existing account or create a new one. After a match is confirmed, offer **Remember this nickname**. Remembered names are personal and sync with that member’s draft access across devices.
- A member can describe several accounts in a prompt, use the account setup action, or add an account while logging activity. The review shows new accounts separately from their associated entries. Confirm the new account details before saving. A new account and entries that use it can save together.
- Ask for a missing balance and clarify whether the stated amount is an opening balance or a current balance. When a current balance includes earlier activity in the draft, work backward from the stated amount and show the calculation. For example, ₹5,000 now after a ₹200 expense yesterday implies a ₹5,200 opening balance and a ₹5,000 resulting balance. Ask which activity the balance covers when the timing is unclear. If it includes a transfer awaiting approval, show the balance when saved and the projected balance after approval.

## Household visibility and ownership

- Every financial account has one owner. The creator owns a new account. Each member can write to their own accounts and see the other member’s balances and individual saved entries in a read-only view. **My accounts** is the default. Use the explicit other-member view to see their accounts.
- One designated member assigns existing unowned accounts to either person. They can review and correct the proposed assignments before confirming. Existing accounts and their history remain visible during this process. Writes become available after ownership is assigned, and confirmed ownership stays fixed in v1.
- Prompt history and unsaved rows belong only to their draft owner. Paisa records who saved each batch. For an approved transfer, Paisa also records who approved it and when.

### Transfers between members

1. The sending account owner starts the request. They review the sending and receiving accounts, amount, date, and description before saving the batch.
2. Save posts ready entries, including transfers between accounts with the same owner, and registers cross-member transfer requests. Pending requests have no transfer effect on either balance or the posted ledger.
3. The receiving account owner approves or declines each request as submitted. The sender can cancel a pending request and submit a corrected one. A request stays pending until someone approves, declines, or cancels it.
4. Approval updates both balances together. If a transfer happened on Monday and was approved on Wednesday, Ledger shows it under Monday and the request history records Wednesday as the approval date.
5. A posted transfer between members stays in the ledger. A correction uses a separate approved reversal in the opposite direction, followed by a new corrected request when needed. The current owner of the reversal’s sending account initiates it. Existing transfers that become cross-member after ownership assignment stay posted and follow the same correction process.

## OpenRouter and data handling

Paisa’s backend sends the prompt and the relevant unsaved draft fields needed for follow-up interpretation. When names help resolve the request, include only the relevant saved account or category names. Match and validate account references locally. Saved transaction history and account balances from the ledger stay in Paisa.

The OpenRouter key lives in a Git-ignored `backend/.env` file and is read by the backend. OpenRouter proposes draft changes. Paisa checks the amounts, dates, accounts, categories, and permissions, and commits only what the member has reviewed and saved.
