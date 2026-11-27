use serde::{Deserialize, Serialize};

/// Query for the reusable Net Earnings report foundation.
/// `month` is a validated calendar month in YYYY-MM format.
#[derive(Debug, Clone, Deserialize)]
pub struct NetEarningsReportQuery {
    pub month: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ItemsSummaryReportQuery {
    pub month: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LabelReportQuery {
    pub month: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DailySummaryReportQuery {
    pub anchor_date: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BalanceSheetReportQuery {
    pub month: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CashFlowNetWorthQuery {
    pub month: String,
    pub months: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreditCardReportQuery {
    pub account_id: i64,
    pub anchor_date: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportPeriod {
    pub start_date: String,
    pub end_date_exclusive: String,
    pub income_minor: i64,
    pub expense_minor: i64,
    pub net_minor: i64,
    pub income_count: i64,
    pub expense_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportMetricValues {
    pub direct_minor: i64,
    pub rollup_minor: i64,
    pub direct_count: i64,
    pub rollup_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportCategoryPeriodValues {
    pub income: ReportMetricValues,
    pub expense: ReportMetricValues,
}

/// Deterministic category metadata plus current/prior direct and rollup values.
/// A null `category_id` is the synthetic Uncategorized row.
#[derive(Debug, Clone, Serialize)]
pub struct ReportCategory {
    pub category_id: Option<i64>,
    pub name: String,
    pub kind: String,
    pub parent_id: Option<i64>,
    pub parent_name: Option<String>,
    pub color: String,
    pub icon: String,
    pub sort_order: i64,
    pub archived_at: Option<String>,
    pub current: ReportCategoryPeriodValues,
    pub prior: ReportCategoryPeriodValues,
}

/// Response consumed by F05 and later report screens.
/// Income/expense totals count posted entries once; transfer legs and opening
/// balances are excluded. Invalid legacy timestamps are counted separately.
#[derive(Debug, Clone, Serialize)]
pub struct NetEarningsReport {
    pub month: String,
    pub prior_month: String,
    pub current: ReportPeriod,
    pub prior: ReportPeriod,
    pub invalid_date_entry_count: i64,
    pub categories: Vec<ReportCategory>,
}

/// One exact transaction-name group in the selected report month.
#[derive(Debug, Clone, Serialize)]
pub struct ReportItem {
    pub rank: usize,
    pub name: String,
    pub total_minor: i64,
    pub entry_count: i64,
}

/// Selected-month item/payee totals. Equal totals share a competition rank;
/// exact names provide deterministic display order within a tie.
#[derive(Debug, Clone, Serialize)]
pub struct ItemsSummaryReport {
    pub month: String,
    pub current: ReportPeriod,
    pub income_items: Vec<ReportItem>,
    pub expense_items: Vec<ReportItem>,
    pub invalid_date_entry_count: i64,
}

/// One additive allocation row. `label_id: null` plus `is_no_labels: true`
/// identifies the synthetic bucket, even when a user label is named "No Labels".
#[derive(Debug, Clone, Serialize)]
pub struct LabelReportRow {
    pub label_id: Option<i64>,
    pub name: String,
    pub is_no_labels: bool,
    pub income_minor: i64,
    pub expense_minor: i64,
    pub net_minor: i64,
}

/// Selected-month income and expense allocation across current labels and
/// No Labels. Every eligible posted entry contributes its paise exactly once.
#[derive(Debug, Clone, Serialize)]
pub struct LabelReport {
    pub month: String,
    pub current: ReportPeriod,
    pub rows: Vec<LabelReportRow>,
    pub invalid_date_entry_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DailySpending {
    pub date: String,
    pub expense_minor: i64,
}

/// Seven chart days ending on the anchor, plus full-window expense averages.
/// Averages round to the nearest integer paise, with exact halves rounded up.
#[derive(Debug, Clone, Serialize)]
pub struct DailySummaryReport {
    pub anchor_date: String,
    pub seven_day_start_date: String,
    pub thirty_day_start_date: String,
    pub average_7_minor: i64,
    pub average_30_minor: i64,
    pub days: Vec<DailySpending>,
    pub invalid_date_entry_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BalanceSheetAccount {
    pub account_id: i64,
    pub name: String,
    pub account_type: String,
    pub account_type_label: String,
    pub prior_minor: i64,
    pub current_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BalanceSheetGroup {
    pub key: String,
    pub label: String,
    pub accounts: Vec<BalanceSheetAccount>,
    pub prior_subtotal_minor: i64,
    pub current_subtotal_minor: i64,
}

/// Cumulative signed balances at the prior and selected month-end.
#[derive(Debug, Clone, Serialize)]
pub struct BalanceSheetReport {
    pub month: String,
    pub prior_month: String,
    pub prior_date: String,
    pub current_date: String,
    pub groups: Vec<BalanceSheetGroup>,
    pub prior_net_worth_minor: i64,
    pub current_net_worth_minor: i64,
    pub invalid_date_entry_count: i64,
    pub legacy_opening_account_count: i64,
}

/// One month using the same half-open cash-flow period and month-end net-worth cutoff.
#[derive(Debug, Clone, Serialize)]
pub struct CashFlowNetWorthPoint {
    pub month: String,
    pub cutoff_date: String,
    pub cash_flow: ReportPeriod,
    pub net_worth_minor: i64,
}

/// Current-data recomputation across identical requested monthly cutoffs.
#[derive(Debug, Clone, Serialize)]
pub struct CashFlowNetWorthReport {
    pub month: String,
    pub prior_month: String,
    pub months: usize,
    pub current: ReportPeriod,
    pub prior: ReportPeriod,
    pub points: Vec<CashFlowNetWorthPoint>,
    pub invalid_date_entry_count: i64,
    pub legacy_opening_account_count: i64,
}

/// One identifiable card-side ledger event. Incoming transfer_in legs are
/// card-directed payments; outgoing transfer_out legs are advances/withdrawals;
/// non-transfer income remains a separate credit.
#[derive(Debug, Clone, Serialize)]
pub struct CreditCardStatementEvent {
    pub entry_id: i64,
    pub transfer_id: Option<i64>,
    pub entry_kind: String,
    pub name: String,
    pub occurred_at: String,
    pub amount_minor: i64,
    pub counterpart_account_name: Option<String>,
}

/// Paisa's ledger-derived statement convention, not an issuer-provided bill.
#[derive(Debug, Clone, Serialize)]
pub struct CreditCardStatement {
    pub start_date: String,
    pub cutoff_date: String,
    pub end_date_exclusive: String,
    pub due_date: String,
    pub payment_start_date: String,
    pub payment_end_date_exclusive: String,
    pub opening_balance_minor: i64,
    pub opening_baseline_before_period_minor: i64,
    pub opening_baseline_inside_period_minor: i64,
    pub balance_before_period_minor: i64,
    pub balance_at_cutoff_minor: i64,
    pub owed_at_cutoff_minor: i64,
    pub credit_at_cutoff_minor: i64,
    pub charge_minor: i64,
    pub non_transfer_credit_minor: i64,
    pub statement_payment_minor: i64,
    pub statement_advance_minor: i64,
    pub post_cutoff_charge_minor: i64,
    pub post_cutoff_non_transfer_credit_minor: i64,
    pub post_cutoff_payment_minor: i64,
    pub post_cutoff_advance_minor: i64,
    pub charge_count: i64,
    pub non_transfer_credit_count: i64,
    pub statement_payment_count: i64,
    pub statement_advance_count: i64,
    pub post_cutoff_charge_count: i64,
    pub post_cutoff_non_transfer_credit_count: i64,
    pub post_cutoff_payment_count: i64,
    pub post_cutoff_advance_count: i64,
    pub invalid_date_entry_count: i64,
    pub legacy_opening_balance: bool,
    pub statement_activities: Vec<CreditCardStatementEvent>,
    pub statement_payments: Vec<CreditCardStatementEvent>,
    pub post_cutoff_charges: Vec<CreditCardStatementEvent>,
    pub post_cutoff_advances: Vec<CreditCardStatementEvent>,
    pub post_cutoff_payments: Vec<CreditCardStatementEvent>,
    pub post_cutoff_non_transfer_credits: Vec<CreditCardStatementEvent>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreditCardReport {
    pub account_id: i64,
    pub account_name: String,
    pub anchor_date: String,
    pub current_balance_minor: i64,
    pub current_owed_minor: i64,
    pub current_credit_minor: i64,
    pub available_credit_minor: Option<i64>,
    pub credit_limit_minor: Option<i64>,
    pub cutoff_day: Option<i64>,
    pub due_day: Option<i64>,
    pub missing_settings: Vec<String>,
    pub statement: Option<CreditCardStatement>,
}
