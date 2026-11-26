use serde::{Deserialize, Serialize};
use sqlx::FromRow;

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
