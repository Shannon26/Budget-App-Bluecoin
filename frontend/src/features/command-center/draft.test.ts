import { describe, expect, it } from "vitest";
import type { Account, DraftPrompt, DraftRow, SaveReceipt } from "../../shared/types";
import {
  accountLabel,
  amountToInput,
  blankDraftRow,
  formatAmount,
  formatGroupDate,
  groupRowsByDate,
  hasUnresolvedAccount,
  isReadyToSave,
  nextOrdinal,
  ownPrompts,
  parseAmountInput,
  receiptCounts,
  unresolvedAccountRowIds,
} from "./draft";

function row(overrides: Partial<DraftRow> = {}): DraftRow {
  return {
    ...blankDraftRow("expense", 1, "2026-01-15", "row-1"),
    ...overrides,
  };
}

const accounts: Account[] = [
  {
    id: 7,
    name: "HDFC",
    currency: "INR",
    opening_balance_minor: 0,
    created_at: "2026-01-01T12:00:00",
    note: "",
    opening_date: null,
    hide_from_selection: false,
    hide_from_reports: false,
    account_type: "checking",
    account_group: "asset",
    credit_limit_minor: null,
    cutoff_day: null,
    due_day: null,
    balance_minor: 0,
  },
];

describe("draft row helpers", () => {
  it("creates a complete blank row with an explicit id", () => {
    const blank = blankDraftRow("new_account", 4, "2026-02-01", "manual-4");
    expect(blank).toMatchObject({
      row_id: "manual-4",
      ordinal: 4,
      kind: "new_account",
      date: "2026-02-01",
      status: "needs_input",
      manual: true,
      source: "manual",
      account_type: "cash",
      transfer_request: false,
    });
    expect(blank.amount_minor).toBeNull();
  });

  it("uses the largest ordinal plus one, including an empty draft", () => {
    expect(nextOrdinal([])).toBe(1);
    expect(nextOrdinal([row({ ordinal: 3 }), row({ ordinal: 9 }), row({ ordinal: 2 })])).toBe(10);
  });

  it("groups rows by local date and keeps ordinal order inside each group", () => {
    const groups = groupRowsByDate(
      [
        row({ row_id: "b", date: "2026-01-15", ordinal: 2 }),
        row({ row_id: "a", date: "2026-01-15", ordinal: 1 }),
        row({ row_id: "c", date: null, ordinal: 3 }),
      ],
      "2026-01-16",
    );
    expect(groups.map((group) => group.date)).toEqual(["2026-01-15", "2026-01-16"]);
    expect(groups[0].rows.map((entry) => entry.row_id)).toEqual(["a", "b"]);
    expect(groups[1].rows.map((entry) => entry.row_id)).toEqual(["c"]);
  });

  it("labels today and yesterday without a timezone shift", () => {
    expect(formatGroupDate("2026-03-10", "2026-03-10")).toBe("Today");
    expect(formatGroupDate("2026-03-09", "2026-03-10")).toBe("Yesterday");
    expect(formatGroupDate("2026-03-01", "2026-03-10")).not.toBe("Today");
  });
});

describe("account and amount presentation", () => {
  it("finds rows with unresolved account endpoints only", () => {
    const rows = [
      row({ row_id: "unresolved", issues: ["Choose an account"] }),
      row({ row_id: "named", account_name: "Cash" }),
      row({ row_id: "resolved", account_id: 7 }),
      row({ row_id: "proposed", proposed_row_id: "new-1" }),
      blankDraftRow("new_account", 5, "2026-01-15", "new-1"),
      row({ row_id: "transfer", kind: "transfer", account_id: 7 }),
    ];
    expect(hasUnresolvedAccount(rows[0])).toBe(true);
    expect(hasUnresolvedAccount(rows[1])).toBe(true);
    expect(hasUnresolvedAccount(rows[2])).toBe(false);
    expect(hasUnresolvedAccount(rows[3])).toBe(false);
    expect(hasUnresolvedAccount(rows[4])).toBe(false);
    expect(hasUnresolvedAccount(rows[5])).toBe(true);
    expect(unresolvedAccountRowIds(rows)).toEqual(["unresolved", "named", "transfer"]);
  });

  it("labels saved, proposed, and unmatched account references", () => {
    const draftRows = [blankDraftRow("new_account", 1, "2026-01-15", "new-1")];
    draftRows[0].name = "Wallet";
    expect(accountLabel(row({ account_id: 7 }), accounts, draftRows)).toBe("HDFC");
    expect(accountLabel(row({ proposed_row_id: "new-1" }), accounts, draftRows)).toBe("New: Wallet");
    expect(accountLabel(row({ account_name: "Cash" }), accounts, draftRows)).toBe(
      "Cash (needs confirmation)",
    );
    expect(accountLabel(row(), accounts, draftRows)).toBe("Choose an account");
    expect(accountLabel(row({ kind: "transfer", account_id: 7 }), accounts, draftRows, true)).toBe(
      "Choose a receiving account",
    );
  });

  it("round-trips paise through the shared money helpers", () => {
    expect(parseAmountInput("240")).toBe(24000);
    expect(parseAmountInput("1,23,456.50")).toBe(12345650);
    expect(parseAmountInput("")).toBeNull();
    expect(parseAmountInput("abc")).toBeNull();
    expect(amountToInput(24000)).toBe("240.00");
    expect(amountToInput(null)).toBe("");
    expect(formatAmount(null)).toBe("Amount needed");
    expect(formatAmount(8000)).toContain("80");
  });
});

describe("review and receipt helpers", () => {
  it("keeps only the member's own prompt messages", () => {
    const prompts: DraftPrompt[] = [
      { submission_id: "a", seq: 1, role: "user", text: "lunch", created_at: "" },
      { submission_id: "a", seq: 2, role: "assistant", text: "1 row", created_at: "" },
      { submission_id: "b", seq: 3, role: "user", text: "coffee", created_at: "" },
    ];
    expect(ownPrompts(prompts).map((prompt) => prompt.text)).toEqual(["lunch", "coffee"]);
  });

  it("requires every row to have its fields resolved before saving", () => {
    const ready = row({ name: "Lunch", amount_minor: 24000, account_id: 7, status: "ready" });
    expect(isReadyToSave([])).toBe(false);
    expect(isReadyToSave([row({ name: "Lunch", amount_minor: 24000 })])).toBe(false);
    expect(isReadyToSave([row({ name: "Lunch", account_id: 7 })])).toBe(false);
    expect(isReadyToSave([ready])).toBe(true);
    expect(isReadyToSave([{ ...ready, issues: ["Use a valid date"] }])).toBe(true);
    expect(isReadyToSave([{ ...ready, date: null }])).toBe(false);
    expect(
      isReadyToSave([
        blankDraftRow("new_account", 1, "2026-01-15", "new-1"),
      ]),
    ).toBe(false);
  });

  it("summarizes receipt ids as counts", () => {
    const receipt: SaveReceipt = {
      batch_id: "batch-1",
      member_id: 1,
      revision: 2,
      accounts_created: [4],
      entry_ids: [9, 10],
      transfer_ids: [11],
      pending_request_ids: [12],
      created_at: "2026-01-15T12:00:00",
    };
    expect(receiptCounts(receipt)).toEqual({ accounts: 1, entries: 2, transfers: 1, pending: 1 });
  });
});
