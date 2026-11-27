use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct Member {
    pub id: i64,
    pub name: String,
    pub is_designated: bool,
}

#[derive(Debug, Serialize)]
pub struct HouseholdAccountView {
    pub owner: Member,
    pub accounts: Vec<HouseholdAccount>,
    pub read_only: bool,
}

#[derive(Debug, Serialize)]
pub struct HouseholdAccount {
    pub id: i64,
    pub name: String,
    pub account_type: String,
    pub currency: String,
    pub opening_balance_minor: i64,
    pub balance_minor: i64,
    pub owner_member_id: Option<i64>,
    pub can_write: bool,
}

#[derive(Debug, Serialize)]
pub struct AssignmentView {
    pub can_assign: bool,
    pub accounts: Vec<AssignmentRow>,
}

#[derive(Debug, Serialize)]
pub struct AssignmentRow {
    pub account_id: i64,
    pub account_name: String,
    pub current_owner_member_id: Option<i64>,
    pub proposed_owner_member_id: Option<i64>,
    pub confirmed_owner_member_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct AssignmentProposal {
    pub account_id: i64,
    pub owner_member_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct AssignmentProposals {
    pub proposals: Vec<AssignmentProposal>,
}

#[derive(Debug, Deserialize)]
pub struct AssignmentConfirmations {
    pub assignments: Vec<AssignmentProposal>,
}
