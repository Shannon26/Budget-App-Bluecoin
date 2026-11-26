use crate::app::AppState;
use crate::error::AppResult;
use crate::models::*;
use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

pub(crate) async fn list_categories(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<Category>>> {
    Ok(Json(state.store.categories().await?))
}

pub(crate) async fn create_category(
    State(state): State<AppState>,
    Json(request): Json<CategoryRequest>,
) -> AppResult<(StatusCode, Json<Category>)> {
    Ok((
        StatusCode::CREATED,
        Json(state.store.create_category(request).await?),
    ))
}

pub(crate) async fn update_category(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<CategoryRequest>,
) -> AppResult<Json<Category>> {
    Ok(Json(state.store.update_category(id, request).await?))
}

pub(crate) async fn delete_category(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<DeleteCategoryResponse>> {
    Ok(Json(state.store.delete_category(id).await?))
}
