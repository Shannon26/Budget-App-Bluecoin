use crate::app::AppState;
use crate::error::AppResult;
use crate::models::{DecisionResponse, MemberId, TransferRequestRecord, TransferRequestsResponse};
use axum::extract::{Path, State};
use axum::Json;

pub(crate) async fn list_requests(
    State(state): State<AppState>,
    MemberId(member_id): MemberId,
) -> AppResult<Json<TransferRequestsResponse>> {
    Ok(Json(state.store.list_requests(member_id).await?))
}

pub(crate) async fn approve(
    State(state): State<AppState>,
    MemberId(member_id): MemberId,
    Path(id): Path<i64>,
) -> AppResult<Json<DecisionResponse>> {
    Ok(Json(state.store.decide(member_id, id, "approve").await?))
}

pub(crate) async fn decline(
    State(state): State<AppState>,
    MemberId(member_id): MemberId,
    Path(id): Path<i64>,
) -> AppResult<Json<DecisionResponse>> {
    Ok(Json(state.store.decide(member_id, id, "decline").await?))
}

pub(crate) async fn cancel(
    State(state): State<AppState>,
    MemberId(member_id): MemberId,
    Path(id): Path<i64>,
) -> AppResult<Json<DecisionResponse>> {
    Ok(Json(state.store.decide(member_id, id, "cancel").await?))
}

pub(crate) async fn reverse(
    State(state): State<AppState>,
    MemberId(member_id): MemberId,
    Path(id): Path<i64>,
) -> AppResult<Json<TransferRequestRecord>> {
    Ok(Json(state.store.reverse_transfer(member_id, id).await?))
}
