use crate::app::AppState;
use crate::error::AppResult;
use crate::models::*;
use axum::extract::State;
use axum::Json;

pub(crate) async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

pub(crate) async fn get_state(State(state): State<AppState>) -> AppResult<Json<StateResponse>> {
    Ok(Json(state.store.state().await?))
}
