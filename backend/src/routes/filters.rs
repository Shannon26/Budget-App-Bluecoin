use crate::app::AppState;
use crate::error::AppResult;
use crate::models::*;
use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

pub(crate) async fn list_saved_filters(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<SavedFilter>>> {
    Ok(Json(state.store.saved_filters().await?))
}

pub(crate) async fn create_saved_filter(
    State(state): State<AppState>,
    Json(request): Json<SavedFilterRequest>,
) -> AppResult<(StatusCode, Json<SavedFilter>)> {
    Ok((
        StatusCode::CREATED,
        Json(state.store.create_saved_filter(request).await?),
    ))
}

pub(crate) async fn update_saved_filter(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<SavedFilterRequest>,
) -> AppResult<Json<SavedFilter>> {
    Ok(Json(state.store.update_saved_filter(id, request).await?))
}

pub(crate) async fn delete_saved_filter(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<serde_json::Value>> {
    state.store.delete_saved_filter(id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
