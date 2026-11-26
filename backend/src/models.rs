use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TransactionKind {
    Income,
    Expense,
}

impl TransactionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Income => "income",
            Self::Expense => "expense",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAccountRequest {
    pub name: String,
    #[serde(default = "default_currency")]
    pub currency: String,
    #[serde(default)]
    pub opening_balance_minor: i64,
}

fn default_currency() -> String {
    "INR".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRequest {
    pub kind: TransactionKind,
    pub account_id: i64,
    pub amount_minor: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub category_id: Option<i64>,
    #[serde(default)]
    pub reconciled: bool,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub occurred_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRequest {
    pub from_account_id: i64,
    pub to_account_id: i64,
    pub amount_minor: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub reconciled: bool,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub occurred_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationRequest {
    pub reconciled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryRequest {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub parent_id: Option<i64>,
    #[serde(default = "default_category_color")]
    pub color: String,
    #[serde(default = "default_category_icon")]
    pub icon: String,
    #[serde(default)]
    pub sort_order: i64,
}

fn default_category_color() -> String {
    "#426b8a".to_string()
}

fn default_category_icon() -> String {
    "•".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedFilterRequest {
    pub name: String,
    pub definition: FilterDefinition,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct FilterDefinition {
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
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub currency: String,
    pub opening_balance_minor: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AccountWithBalance {
    pub id: i64,
    pub name: String,
    pub currency: String,
    pub opening_balance_minor: i64,
    pub created_at: String,
    pub balance_minor: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub parent_id: Option<i64>,
    pub parent_name: Option<String>,
    pub color: String,
    pub icon: String,
    pub sort_order: i64,
    pub archived_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CategoryTotal {
    pub category_id: Option<i64>,
    pub kind: String,
    pub name: String,
    pub total_minor: i64,
    pub entry_count: i64,
}

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

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Totals {
    pub income_minor: i64,
    pub expense_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SavedFilter {
    pub id: i64,
    pub name: String,
    pub definition: FilterDefinition,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub(crate) struct SavedFilterRow {
    id: i64,
    name: String,
    definition_json: String,
    created_at: String,
    updated_at: String,
}

impl SavedFilterRow {
    pub(crate) fn into_saved_filter(self) -> AppResult<(i64, SavedFilter)> {
        let definition = serde_json::from_str(&self.definition_json)
            .map_err(|error| crate::error::AppError::BadRequest(error.to_string()))?;
        Ok((
            self.id,
            SavedFilter {
                id: self.id,
                name: self.name,
                definition,
                created_at: self.created_at,
                updated_at: self.updated_at,
            },
        ))
    }
}

pub type AppResult<T> = crate::error::AppResult<T>;

#[derive(Debug, Clone, Serialize)]
pub struct StateResponse {
    pub accounts: Vec<AccountWithBalance>,
    pub totals: Totals,
    pub categories: Vec<Category>,
    pub category_totals: Vec<CategoryTotal>,
    pub saved_filters: Vec<SavedFilter>,
    pub entries: Vec<LedgerEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransferResponse {
    pub id: i64,
    pub from_account_id: i64,
    pub to_account_id: i64,
    pub amount_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeleteCategoryResponse {
    pub archived: bool,
}
