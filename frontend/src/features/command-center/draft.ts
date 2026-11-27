import { localDateValue } from "../../shared/date";
import { formatMinor, parseMinorUnits } from "../../shared/money";
import type {
  Account,
  DraftPrompt,
  DraftRow,
  DraftRowKind,
  SaveReceipt,
} from "../../shared/types";

export interface DateGroup {
  date: string;
  label: string;
  rows: DraftRow[];
}

/** A full row shape so every write sends the same fields the backend validates. */
export function blankDraftRow(
  kind: DraftRowKind,
  ordinal: number,
  localDate: string,
  rowId: string,
): DraftRow {
  return {
    row_id: rowId,
    ordinal,
    kind,
    name: "",
    amount_minor: null,
    date: localDate,
    account_id: null,
    account_name: null,
    to_account_id: null,
    to_account_name: null,
    category_id: null,
    note: "",
    status: "needs_input",
    issues: [],
    manual: true,
    source: "manual",
    proposed_row_id: null,
    account_type: kind === "new_account" ? "cash" : null,
    opening_balance_minor: null,
    current_balance_minor: null,
    transfer_request: false,
  };
}

export function nextOrdinal(rows: DraftRow[]): number {
  return rows.reduce((largest, row) => Math.max(largest, row.ordinal), 0) + 1;
}

export function newLocalRowId(prefix = "local"): string {
  const random =
    typeof crypto !== "undefined" && typeof crypto.randomUUID === "function"
      ? crypto.randomUUID()
      : `${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
  return `${prefix}-${random}`;
}

export function formatGroupDate(date: string, today = localDateValue()): string {
  if (date === today) return "Today";
  const yesterday = new Date(`${today}T12:00:00`);
  yesterday.setDate(yesterday.getDate() - 1);
  if (date === localDateValue(yesterday)) return "Yesterday";
  const parsed = new Date(`${date}T12:00:00`);
  if (Number.isNaN(parsed.getTime())) return date;
  return parsed.toLocaleDateString(undefined, {
    weekday: "short",
    day: "numeric",
    month: "short",
    year: parsed.getFullYear() === new Date().getFullYear() ? undefined : "numeric",
  });
}

export function groupRowsByDate(rows: DraftRow[], fallbackDate: string): DateGroup[] {
  const grouped = new Map<string, DraftRow[]>();
  for (const row of rows) {
    const date = row.date ?? fallbackDate;
    const group = grouped.get(date);
    if (group) group.push(row);
    else grouped.set(date, [row]);
  }
  return [...grouped.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([date, groupRows]) => ({
      date,
      label: formatGroupDate(date),
      rows: [...groupRows].sort((left, right) => left.ordinal - right.ordinal),
    }));
}

/** Rows whose account endpoint is still unresolved can share one assignment. */
export function hasUnresolvedAccount(row: DraftRow): boolean {
  if (row.kind === "new_account") return false;
  // A name without a saved id still needs confirmation in review.
  const missingSource = row.account_id === null && row.proposed_row_id === null;
  const missingTarget = row.kind === "transfer" && row.to_account_id === null;
  return missingSource || missingTarget;
}

export function unresolvedAccountRowIds(rows: DraftRow[]): string[] {
  return rows.filter(hasUnresolvedAccount).map((row) => row.row_id);
}

export function accountLabel(
  row: DraftRow,
  accounts: Account[],
  draftRows: DraftRow[],
  destination = false,
): string {
  const id = destination ? row.to_account_id : row.account_id;
  const name = destination ? row.to_account_name : row.account_name;
  const proposedId = destination ? null : row.proposed_row_id;
  if (id !== null) {
    const match = accounts.find((account) => account.id === id);
    if (match) return match.name;
    return name || `Account ${id}`;
  }
  if (proposedId) {
    const proposed = draftRows.find((candidate) => candidate.row_id === proposedId);
    return `New: ${proposed?.name?.trim() || "account"}`;
  }
  if (name) return `${name} (needs confirmation)`;
  return destination ? "Choose a receiving account" : "Choose an account";
}

export function amountToInput(minor: number | null): string {
  return minor === null ? "" : (minor / 100).toFixed(2);
}

/** Returns null for empty or invalid input; the editor shows an inline error. */
export function parseAmountInput(raw: string): number | null {
  if (!raw.trim()) return null;
  return parseMinorUnits(raw);
}

export function formatAmount(minor: number | null): string {
  return minor === null ? "Amount needed" : formatMinor(minor);
}

/** The transcript shown in review belongs to the draft owner only. */
export function ownPrompts(prompts: DraftPrompt[]): DraftPrompt[] {
  return prompts.filter((prompt) => prompt.role === "user" || prompt.role === "member");
}

/** Derived from field values, so a stale backend issue label cannot block a corrected row. */
export function isReadyToSave(rows: DraftRow[]): boolean {
  return (
    rows.length > 0 &&
    rows.every((row) => {
      if (!row.name.trim() || !row.date) return false;
      if (hasUnresolvedAccount(row)) return false;
      if (row.kind === "new_account") {
        return row.opening_balance_minor !== null || row.current_balance_minor !== null;
      }
      return row.amount_minor !== null && row.amount_minor > 0;
    })
  );
}

export function receiptCounts(receipt: SaveReceipt) {
  return {
    accounts: receipt.accounts_created.length,
    entries: receipt.entry_ids.length,
    transfers: receipt.transfer_ids.length,
    pending: receipt.pending_request_ids.length,
  };
}
