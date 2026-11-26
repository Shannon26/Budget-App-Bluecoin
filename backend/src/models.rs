use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
pub struct CreateTransactionRequest {
    pub kind: TransactionKind,
    pub account_id: i64,
    pub amount_minor: i64,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub occurred_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTransferRequest {
    pub from_account_id: i64,
    pub to_account_id: i64,
    pub amount_minor: i64,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub occurred_at: Option<String>,
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
pub struct EntryView {
    pub id: i64,
    pub kind: String,
    pub account_id: i64,
    pub account_name: String,
    pub amount_minor: i64,
    pub note: String,
    pub occurred_at: String,
    pub transfer_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Totals {
    pub income_minor: i64,
    pub expense_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StateResponse {
    pub accounts: Vec<AccountWithBalance>,
    pub totals: Totals,
    pub entries: Vec<EntryView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransferResponse {
    pub id: i64,
    pub from_account_id: i64,
    pub to_account_id: i64,
    pub amount_minor: i64,
}
