use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use crate::app::AppState;
use crate::error::AppResult;
use crate::models::*;

#[derive(Debug, Deserialize)]
pub(crate) struct AccountsQuery {
    pub member_id: Option<i64>,
}

pub(crate) async fn list_members(
    State(state): State<AppState>,
    member: MemberId,
) -> AppResult<Json<Vec<Member>>> {
    if state.identity.household_enabled && !state.store.member_exists(member.0).await? {
        return Err(crate::error::AppError::Unauthorized(
            "viewer is not a household member".to_string(),
        ));
    }
    Ok(Json(state.store.members().await?))
}

pub(crate) async fn accounts_view(
    State(state): State<AppState>,
    member: MemberId,
    Query(query): Query<AccountsQuery>,
) -> AppResult<Json<HouseholdAccountView>> {
    let target = query.member_id.unwrap_or(member.0);
    Ok(Json(
        state
            .store
            .accounts_view(member.0, target, state.identity.household_enabled)
            .await?,
    ))
}

pub(crate) async fn assignments(
    State(state): State<AppState>,
    member: MemberId,
) -> AppResult<Json<AssignmentView>> {
    Ok(Json(
        state
            .store
            .assignments(member.0, state.identity.household_enabled)
            .await?,
    ))
}

pub(crate) async fn propose_assignments(
    State(state): State<AppState>,
    member: MemberId,
    Json(request): Json<AssignmentProposals>,
) -> AppResult<Json<AssignmentView>> {
    Ok(Json(
        state
            .store
            .propose_assignments(member.0, request.proposals)
            .await?,
    ))
}

pub(crate) async fn confirm_assignments(
    State(state): State<AppState>,
    member: MemberId,
    Json(request): Json<AssignmentConfirmations>,
) -> AppResult<Json<OkResponse>> {
    state
        .store
        .confirm_assignments(member.0, request.assignments)
        .await?;
    Ok(Json(OkResponse { ok: true }))
}
