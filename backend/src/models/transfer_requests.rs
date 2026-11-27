use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestStatus {
    Pending,
    Approved,
    Declined,
    Canceled,
}

impl RequestStatus {
    #[allow(dead_code)]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Declined => "declined",
            Self::Canceled => "canceled",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TransferRequestRecord {
    pub id: i64,
    pub source_account_id: i64,
    pub source_account_name: String,
    pub destination_account_id: i64,
    pub destination_account_name: String,
    pub amount_minor: i64,
    pub name: String,
    pub note: String,
    pub requested_date: String,
    pub sender_member_id: i64,
    pub recipient_member_id: i64,
    pub status: RequestStatus,
    pub source_batch_id: Option<String>,
    pub original_transfer_id: Option<i64>,
    pub created_at: String,
    pub decided_at: Option<String>,
    pub decided_by_member_id: Option<i64>,
    pub posted_transfer_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewTransferRequest {
    pub source_account_id: i64,
    pub destination_account_id: i64,
    pub amount_minor: i64,
    pub name: String,
    pub note: String,
    pub requested_date: String,
    pub sender_member_id: i64,
    pub recipient_member_id: i64,
    pub source_batch_id: Option<String>,
    pub original_transfer_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct TransferRequestsResponse {
    pub incoming: Vec<TransferRequestRecord>,
    pub outgoing: Vec<TransferRequestRecord>,
    pub incoming_count: usize,
    pub outgoing_count: usize,
}

#[derive(Debug, Serialize)]
pub struct DecisionResponse {
    pub request: TransferRequestRecord,
    pub posted_transfer_id: Option<i64>,
}
