use crate::app::AppState;
use crate::error::AppResult;
use crate::models::*;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;

pub(crate) async fn list_labels(State(state): State<AppState>) -> AppResult<Json<Vec<Label>>> {
    Ok(Json(state.store.labels().await?))
}

pub(crate) async fn create_label(
    State(state): State<AppState>,
    Json(request): Json<LabelRequest>,
) -> AppResult<(StatusCode, Json<Label>)> {
    Ok((
        StatusCode::CREATED,
        Json(state.store.create_label(request).await?),
    ))
}

pub(crate) async fn update_label(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<LabelRequest>,
) -> AppResult<Json<Label>> {
    Ok(Json(state.store.update_label(id, request).await?))
}
