use serde::{Deserialize, Serialize};

use super::drafts::DraftRow;

/// Explicit allowlist of context sent to the assistant. No saved ledger
/// history, balances, or prompt transcript from other drafts is included.
#[derive(Debug, Clone, Serialize)]
pub struct AssistantRequest {
    pub text: String,
    pub local_date: String,
    pub rows: Vec<DraftRow>,
    pub account_names: Vec<AssistantAccountName>,
    pub category_names: Vec<AssistantCategoryName>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssistantAccountName {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssistantCategoryName {
    pub id: i64,
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AssistantProposal {
    /// New rows to append to the draft.
    pub rows: Vec<DraftRow>,
    /// Targeted replacements keyed by stable draft row ID.
    pub edits: Vec<AssistantRowEdit>,
    pub message: String,
    pub clarification: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AssistantRowEdit {
    pub row_id: String,
    pub row: DraftRow,
}
