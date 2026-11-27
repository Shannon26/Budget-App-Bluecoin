use serde::{Deserialize, Serialize};

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
    #[serde(default)]
    pub labels: Option<Vec<i64>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransferResponse {
    pub id: i64,
    pub from_account_id: i64,
    pub to_account_id: i64,
    pub amount_minor: i64,
}
