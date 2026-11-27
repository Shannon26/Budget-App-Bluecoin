use serde::{Deserialize, Serialize};

/// Query for the reusable Net Earnings report foundation.
/// `month` is a validated calendar month in YYYY-MM format.
#[derive(Debug, Clone, Deserialize)]
pub struct NetEarningsReportQuery {
    pub month: String,
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
