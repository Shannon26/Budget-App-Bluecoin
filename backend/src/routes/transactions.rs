use crate::app::AppState;
use crate::error::AppResult;
use crate::models::*;
use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

pub(crate) async fn create_transaction(
    State(state): State<AppState>,
    Json(request): Json<TransactionRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let id = state.store.create_transaction(request).await?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

pub(crate) async fn update_transaction(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<TransactionRequest>,
) -> AppResult<Json<serde_json::Value>> {
    state.store.update_transaction(id, request).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub(crate) async fn transaction_detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<LedgerEntry>> {
    Ok(Json(state.store.transaction_detail(id).await?))
}

pub(crate) async fn duplicate_transaction(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let id = state.store.duplicate_transaction(id).await?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

pub(crate) async fn delete_transaction(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<serde_json::Value>> {
    state.store.delete_transaction(id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub(crate) async fn reconcile_transaction(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<ReconciliationRequest>,
) -> AppResult<Json<serde_json::Value>> {
    state
        .store
        .set_transaction_reconciled(id, request.reconciled)
        .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
