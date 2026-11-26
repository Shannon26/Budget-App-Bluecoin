use super::TransactionKind;
use serde::{Deserialize, Serialize};

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
