import type { LedgerEntry } from "../../shared/types";

export function entryPresentation(entry: LedgerEntry) {
  if (entry.kind === "transfer") {
    return {
      label: "Transfer",
      sign: "⇄",
      tone: "transfer",
      account: `${entry.account_name} → ${entry.counterpart_account_name ?? ""}`,
      category: "Not categorized",
    };
  }
  if (entry.kind === "transfer_in" || entry.kind === "transfer_out") {
    return {
      label: entry.kind === "transfer_in" ? "Transfer in" : "Transfer out",
      sign: entry.kind === "transfer_in" ? "+" : "−",
      tone: "transfer",
      account: `${entry.account_name} · ${entry.counterpart_account_name ?? ""}`,
      category: "Not categorized",
    };
  }
  return {
    label: entry.kind === "income" ? "Income" : "Expense",
    sign: entry.kind === "income" ? "+" : "−",
    tone: entry.kind === "income" ? "positive" : "negative",
    account: entry.account_name,
    category: entry.category_name
      ? `${entry.category_parent_name ? `${entry.category_parent_name} / ` : ""}${entry.category_name}`
      : "Uncategorized",
  };
}
