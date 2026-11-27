use crate::app::AppState;
use crate::error::AppResult;
use crate::models::*;
use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

pub(crate) async fn create_transaction(
    State(state): State<AppState>,
    member: MemberId,
    Json(request): Json<TransactionRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    state
        .store
        .ensure_account_write(
            state.identity.household_enabled,
            member.0,
            request.account_id,
        )
        .await?;
    let id = state.store.create_transaction(request).await?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

pub(crate) async fn update_transaction(
    State(state): State<AppState>,
    member: MemberId,
    Path(id): Path<i64>,
    Json(request): Json<TransactionRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let enforce = state.identity.household_enabled;
    state
        .store
        .ensure_entry_write(enforce, member.0, id)
        .await?;
    state
        .store
        .ensure_account_write(enforce, member.0, request.account_id)
        .await?;
    state.store.update_transaction(id, request).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub(crate) async fn transaction_detail(
    State(state): State<AppState>,
    member: MemberId,
    Path(id): Path<i64>,
) -> AppResult<Json<LedgerEntry>> {
    if state.identity.household_enabled && !state.store.member_exists(member.0).await? {
        return Err(crate::error::AppError::Unauthorized(
            "household membership is required".to_string(),
        ));
    }
    Ok(Json(state.store.transaction_detail(id).await?))
}

pub(crate) async fn duplicate_transaction(
    State(state): State<AppState>,
    member: MemberId,
    Path(id): Path<i64>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    state
        .store
        .ensure_entry_write(state.identity.household_enabled, member.0, id)
        .await?;
    let id = state.store.duplicate_transaction(id).await?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

pub(crate) async fn delete_transaction(
    State(state): State<AppState>,
    member: MemberId,
    Path(id): Path<i64>,
) -> AppResult<Json<serde_json::Value>> {
    state
        .store
        .ensure_entry_write(state.identity.household_enabled, member.0, id)
        .await?;
    state.store.delete_transaction(id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub(crate) async fn reconcile_transaction(
    State(state): State<AppState>,
    member: MemberId,
    Path(id): Path<i64>,
    Json(request): Json<ReconciliationRequest>,
) -> AppResult<Json<serde_json::Value>> {
    state
        .store
        .ensure_entry_write(state.identity.household_enabled, member.0, id)
        .await?;
    state
        .store
        .set_transaction_reconciled(id, request.reconciled)
        .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
