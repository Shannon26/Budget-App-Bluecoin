export type TransactionKind = "income" | "expense";
export type EntryKind = TransactionKind | "transfer" | "transfer_in" | "transfer_out";

export type AccountGroup = "asset" | "liability" | "unclassified";

export interface AccountTypeDefinition {
  value: string;
  label: string;
  group: AccountGroup;
}

export interface Label {
  id: number;
  name: string;
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
  credit_limit_minor: number | null;
  cutoff_day: number | null;
  due_day: number | null;
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
  credit_limit_minor?: number | null;
  cutoff_day?: number | null;
  due_day?: number | null;
}

export interface ReportPeriod {
  start_date: string;
  end_date_exclusive: string;
  income_minor: number;
  expense_minor: number;
  net_minor: number;
  income_count: number;
  expense_count: number;
}

export interface ReportMetricValues {
  direct_minor: number;
  rollup_minor: number;
  direct_count: number;
  rollup_count: number;
}

export interface ReportCategoryPeriodValues {
  income: ReportMetricValues;
  expense: ReportMetricValues;
}

export interface ReportCategory {
  category_id: number | null;
  name: string;
  kind: string;
  parent_id: number | null;
  parent_name: string | null;
  color: string;
  icon: string;
  sort_order: number;
  archived_at: string | null;
  current: ReportCategoryPeriodValues;
  prior: ReportCategoryPeriodValues;
}

export interface NetEarningsReport {
  month: string;
  prior_month: string;
  current: ReportPeriod;
  prior: ReportPeriod;
  invalid_date_entry_count: number;
  categories: ReportCategory[];
}

export interface ReportItem {
  rank: number;
  name: string;
  total_minor: number;
  entry_count: number;
}

export interface ItemsSummaryReport {
  month: string;
  current: ReportPeriod;
  income_items: ReportItem[];
  expense_items: ReportItem[];
  invalid_date_entry_count: number;
}

export interface DailySpending {
  date: string;
  expense_minor: number;
}

export interface DailySummaryReport {
  anchor_date: string;
  seven_day_start_date: string;
  thirty_day_start_date: string;
  average_7_minor: number;
  average_30_minor: number;
  days: DailySpending[];
  invalid_date_entry_count: number;
}

export interface BalanceSheetAccount {
  account_id: number;
  name: string;
  account_type: string;
  account_type_label: string;
  prior_minor: number;
  current_minor: number;
}

export interface BalanceSheetGroup {
  key: string;
  label: string;
  accounts: BalanceSheetAccount[];
  prior_subtotal_minor: number;
  current_subtotal_minor: number;
}

export interface BalanceSheetReport {
  month: string;
  prior_month: string;
  prior_date: string;
  current_date: string;
  groups: BalanceSheetGroup[];
  prior_net_worth_minor: number;
  current_net_worth_minor: number;
  invalid_date_entry_count: number;
  legacy_opening_account_count: number;
}

export interface CreditCardStatementEvent {
  entry_id: number;
  transfer_id: number | null;
  entry_kind: string;
  name: string;
  occurred_at: string;
  amount_minor: number;
  counterpart_account_name: string | null;
}

export interface CreditCardStatement {
  start_date: string;
  cutoff_date: string;
  end_date_exclusive: string;
  due_date: string;
  payment_start_date: string;
  payment_end_date_exclusive: string;
  opening_balance_minor: number;
  opening_baseline_before_period_minor: number;
  opening_baseline_inside_period_minor: number;
  balance_before_period_minor: number;
  balance_at_cutoff_minor: number;
  owed_at_cutoff_minor: number;
  credit_at_cutoff_minor: number;
  charge_minor: number;
  non_transfer_credit_minor: number;
  statement_payment_minor: number;
  statement_advance_minor: number;
  post_cutoff_charge_minor: number;
  post_cutoff_non_transfer_credit_minor: number;
  post_cutoff_payment_minor: number;
  post_cutoff_advance_minor: number;
  charge_count: number;
  non_transfer_credit_count: number;
  statement_payment_count: number;
  statement_advance_count: number;
  post_cutoff_charge_count: number;
  post_cutoff_non_transfer_credit_count: number;
  post_cutoff_payment_count: number;
  post_cutoff_advance_count: number;
  invalid_date_entry_count: number;
  legacy_opening_balance: boolean;
  statement_activities: CreditCardStatementEvent[];
  statement_payments: CreditCardStatementEvent[];
  post_cutoff_charges: CreditCardStatementEvent[];
  post_cutoff_advances: CreditCardStatementEvent[];
  post_cutoff_payments: CreditCardStatementEvent[];
  post_cutoff_non_transfer_credits: CreditCardStatementEvent[];
}

export interface CreditCardReport {
  account_id: number;
  account_name: string;
  anchor_date: string;
  current_balance_minor: number;
  current_owed_minor: number;
  current_credit_minor: number;
  available_credit_minor: number | null;
  credit_limit_minor: number | null;
  cutoff_day: number | null;
  due_day: number | null;
  missing_settings: string[];
  statement: CreditCardStatement | null;
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
  labels: Label[];
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
  labels: Label[];
  entries: LedgerEntry[];
}
