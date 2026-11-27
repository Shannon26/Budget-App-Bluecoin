use super::FilterDefinition;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct LedgerEntry {
    pub id: i64,
    pub kind: String,
    pub name: String,
    pub note: String,
    pub account_id: i64,
    pub account_name: String,
    pub counterpart_account_id: Option<i64>,
    pub counterpart_account_name: Option<String>,
    pub amount_minor: i64,
    pub occurred_at: String,
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
    pub category_parent_name: Option<String>,
    pub reconciled: bool,
    pub transfer_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct DailyTotal {
    pub date: String,
    pub income_minor: i64,
    pub expense_minor: i64,
    pub transfer_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LedgerPage {
    pub entries: Vec<LedgerEntry>,
    pub daily_totals: Vec<DailyTotal>,
    pub total_count: usize,
    pub next_offset: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LedgerPaginationQuery {
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LedgerPageQuery {
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub date_from: Option<String>,
    #[serde(default)]
    pub date_to: Option<String>,
    #[serde(default)]
    pub account_id: Option<i64>,
    #[serde(default)]
    pub category_id: Option<i64>,
    #[serde(default)]
    pub uncategorized: Option<bool>,
    #[serde(default)]
    pub reconciled: Option<bool>,
    #[serde(default)]
    pub amount_min_minor: Option<i64>,
    #[serde(default)]
    pub amount_max_minor: Option<i64>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
}

impl LedgerPageQuery {
    pub fn into_filter(self) -> FilterDefinition {
        FilterDefinition {
            search: self.search,
            date_from: self.date_from,
            date_to: self.date_to,
            account_id: self.account_id,
            category_id: self.category_id,
            uncategorized: self.uncategorized,
            reconciled: self.reconciled,
            amount_min_minor: self.amount_min_minor,
            amount_max_minor: self.amount_max_minor,
        }
    }
}
