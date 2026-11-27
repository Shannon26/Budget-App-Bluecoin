use crate::app::AppState;
use crate::error::AppResult;
use crate::models::*;
use axum::extract::State;
use axum::Json;

pub(crate) async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

pub(crate) async fn get_state(
    State(state): State<AppState>,
    member: MemberId,
) -> AppResult<Json<StateResponse>> {
    if state.identity.household_enabled && !state.store.member_exists(member.0).await? {
        return Err(crate::error::AppError::Unauthorized(
            "household membership is required".to_string(),
        ));
    }
    Ok(Json(state.store.state().await?))
}
