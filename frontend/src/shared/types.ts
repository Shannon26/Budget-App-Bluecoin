export type TransactionKind = "income" | "expense";
export type EntryKind = TransactionKind | "transfer" | "transfer_in" | "transfer_out";

export type AccountGroup = "asset" | "liability" | "unclassified";

export interface AccountTypeDefinition {
  value: string;
  label: string;
  group: AccountGroup;
}

export interface AccountMetadata {
  id: number;
  name: string;
  currency: string;
  opening_balance_minor: number;
  created_at: string;
  note: string;
  opening_date: string | null;
  hide_from_selection: boolean;
  hide_from_reports: boolean;
  account_type: string;
  account_group: AccountGroup;
}

export interface Account extends AccountMetadata {
  balance_minor: number;
}

export interface AccountUpdate {
  name?: string;
  note?: string | null;
  opening_date?: string | null;
  account_type?: string;
  hide_from_selection?: boolean;
  hide_from_reports?: boolean;
}

export interface Totals {
  income_minor: number;
  expense_minor: number;
}

export interface Category {
  id: number;
  name: string;
  kind: TransactionKind;
  parent_id: number | null;
  parent_name: string | null;
  color: string;
  icon: string;
  sort_order: number;
  archived_at: string | null;
  created_at: string;
}

export interface CategoryTotal {
  category_id: number | null;
  kind: TransactionKind;
  name: string;
  total_minor: number;
  entry_count: number;
}

export interface CategoryBudget {
  category_id: number;
  name: string;
  parent_id: number | null;
  parent_name: string | null;
  color: string;
  icon: string;
  sort_order: number;
  archived_at: string | null;
  limit_minor: number | null;
  direct_spent_minor: number;
  spent_minor: number;
  remaining_minor: number | null;
}

export interface MonthlyCategoryBudgets {
  month: string;
  total_spent_minor: number;
  uncategorized_spent_minor: number;
  expense_count: number;
  invalid_date_expense_count: number;
  invalid_date_spent_minor: number;
  categories: CategoryBudget[];
}

export interface LedgerEntry {
  id: number;
  kind: EntryKind;
  name: string;
  note: string;
  account_id: number;
  account_name: string;
  counterpart_account_id: number | null;
  counterpart_account_name: string | null;
  amount_minor: number;
  occurred_at: string;
  category_id: number | null;
  category_name: string | null;
  category_parent_name: string | null;
  reconciled: boolean;
  transfer_id: number | null;
}

export interface DailyTotal {
  date: string;
  income_minor: number;
  expense_minor: number;
  transfer_minor: number;
}

export interface LedgerPage {
  entries: LedgerEntry[];
  daily_totals: DailyTotal[];
  total_count: number;
  next_offset: number | null;
}

export interface FilterDefinition {
  search?: string | null;
  date_from?: string | null;
  date_to?: string | null;
  account_id?: number | null;
  category_id?: number | null;
  uncategorized?: boolean | null;
  reconciled?: boolean | null;
  amount_min_minor?: number | null;
  amount_max_minor?: number | null;
}

export interface SavedFilter {
  id: number;
  name: string;
  definition: FilterDefinition;
  created_at: string;
  updated_at: string;
}

export interface BudgetState {
  accounts: Account[];
  account_types: AccountTypeDefinition[];
  totals: Totals;
  categories: Category[];
  category_totals: CategoryTotal[];
  saved_filters: SavedFilter[];
  entries: LedgerEntry[];
}
