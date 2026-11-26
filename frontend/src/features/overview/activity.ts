import type { LedgerEntry } from "../../shared/types";

export function activityDetails(entry: LedgerEntry) {
  if (entry.kind === "transfer") {
    return {
      label: "Transfer",
      sign: "⇄",
      tone: "transfer",
      account: `${entry.account_name} → ${entry.counterpart_account_name ?? ""}`,
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
