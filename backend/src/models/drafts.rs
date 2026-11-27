use serde::{Deserialize, Serialize};

use crate::date::is_valid_calendar_date;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftRowKind {
    #[default]
    Expense,
    Income,
    Transfer,
    NewAccount,
}

impl DraftRowKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Expense => "expense",
            Self::Income => "income",
            Self::Transfer => "transfer",
            Self::NewAccount => "new_account",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftRowStatus {
    #[default]
    Ready,
    NeedsInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftSource {
    #[default]
    Manual,
    Assistant,
}

/// One reviewed ledger row or proposed new account inside a private draft.
///
/// `row_id` is stable and member-scoped so an assistant follow-up can target a
/// single row without touching unrelated manual edits.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DraftRow {
    pub row_id: String,
    pub ordinal: i64,
    pub kind: DraftRowKind,
    pub name: String,
    pub amount_minor: Option<i64>,
    pub date: Option<String>,
    pub account_id: Option<i64>,
    pub account_name: Option<String>,
    pub to_account_id: Option<i64>,
    pub to_account_name: Option<String>,
    pub category_id: Option<i64>,
    pub note: String,
    pub status: DraftRowStatus,
    pub issues: Vec<String>,
    pub manual: bool,
    pub source: DraftSource,
    /// Set on an entry row to reference a `new_account` row in the same draft.
    pub proposed_row_id: Option<String>,
    /// Optional account type when `kind == new_account`.
    pub account_type: Option<String>,
    /// Stated opening balance for a new account.
    pub opening_balance_minor: Option<i64>,
    /// Stated current balance for a new account, reconciled backwards against
    /// the activity the member confirms it includes.
    pub current_balance_minor: Option<i64>,
    /// Cross-member transfers become pending requests rather than posted legs.
    pub transfer_request: bool,
}

impl DraftRow {
    /// Recompute unresolved fields from the row's own data. Kept here so the
    /// store, the assistant application step, and the UI all agree on issues.
    pub fn refresh_issues(&mut self) {
        let mut issues = Vec::new();
        if self.name.trim().is_empty() {
            issues.push("Add a description".to_string());
        }
        match self.kind {
            DraftRowKind::NewAccount => {
                if self.opening_balance_minor.is_none() && self.current_balance_minor.is_none() {
                    issues.push("Add an opening or current balance".to_string());
                }
            }
            DraftRowKind::Transfer => {
                if self.amount_minor.is_none() || self.amount_minor == Some(0) {
                    issues.push("Add an amount".to_string());
                }
                if self.account_id.is_none()
                    && self.account_name.is_none()
                    && self.proposed_row_id.is_none()
                {
                    issues.push("Choose a sending account".to_string());
                }
                if self.to_account_id.is_none() && self.to_account_name.is_none() {
                    issues.push("Choose a receiving account".to_string());
                }
            }
            DraftRowKind::Expense | DraftRowKind::Income => {
                if self.amount_minor.is_none() || self.amount_minor == Some(0) {
                    issues.push("Add an amount".to_string());
                }
                if self.account_id.is_none()
                    && self.account_name.is_none()
                    && self.proposed_row_id.is_none()
                {
                    issues.push("Choose an account".to_string());
                }
            }
        }
        if let Some(amount) = self.amount_minor {
            if amount < 0 {
                issues.push("Amount must be positive".to_string());
            }
        }
        if let Some(date) = &self.date {
            if !is_valid_calendar_date(date) {
                issues.push("Use a valid date".to_string());
            }
        }
        self.status = if issues.is_empty() {
            DraftRowStatus::Ready
        } else {
            DraftRowStatus::NeedsInput
        };
        self.issues = issues;
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Draft {
    pub member_id: i64,
    pub revision: i64,
    pub local_date: String,
    pub updated_at: String,
    pub rows: Vec<DraftRow>,
    pub prompts: Vec<DraftPrompt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftPrompt {
    pub submission_id: String,
    pub seq: i64,
    pub role: String,
    pub text: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct PromptRequest {
    pub text: String,
    pub client_prompt_id: String,
    #[serde(default)]
    pub local_date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SaveRequest {
    pub revision: i64,
}

#[derive(Debug, Deserialize)]
pub struct AssignAccountRequest {
    pub row_ids: Vec<String>,
    pub account_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct AliasRequest {
    pub alias: String,
    pub account_id: Option<i64>,
    pub proposed_row_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DraftResponse {
    pub draft: Draft,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clarification: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SaveReceipt {
    pub batch_id: String,
    pub member_id: i64,
    pub revision: i64,
    pub accounts_created: Vec<i64>,
    pub entry_ids: Vec<i64>,
    pub transfer_ids: Vec<i64>,
    pub pending_request_ids: Vec<i64>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct OkResponse {
    pub ok: bool,
}
