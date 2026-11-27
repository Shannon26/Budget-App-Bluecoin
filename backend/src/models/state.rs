use super::{
    AccountTypeDefinition, AccountWithBalance, Category, CategoryTotal, LedgerEntry, SavedFilter,
};
use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Totals {
    pub income_minor: i64,
    pub expense_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StateResponse {
    pub accounts: Vec<AccountWithBalance>,
    pub account_types: Vec<AccountTypeDefinition>,
    pub totals: Totals,
    pub categories: Vec<Category>,
    pub category_totals: Vec<CategoryTotal>,
    pub saved_filters: Vec<SavedFilter>,
    pub entries: Vec<LedgerEntry>,
}
