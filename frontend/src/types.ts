export type TransactionKind = "income" | "expense";
export type EntryKind = TransactionKind | "transfer_in" | "transfer_out";

export interface Account {
  id: number;
  name: string;
  currency: string;
  opening_balance_minor: number;
  created_at: string;
  balance_minor: number;
}

export interface Totals {
  income_minor: number;
  expense_minor: number;
}

export interface Entry {
  id: number;
  kind: EntryKind;
  account_id: number;
  account_name: string;
  amount_minor: number;
  note: string;
  occurred_at: string;
  transfer_id: number | null;
}

export interface BudgetState {
  accounts: Account[];
  totals: Totals;
  entries: Entry[];
}
