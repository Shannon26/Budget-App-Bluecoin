use crate::app::AppState;
use crate::error::AppResult;
use crate::models::{AssistantUsageSummary, MemberId};
use axum::extract::State;
use axum::Json;

pub(crate) async fn usage_summary(
    State(state): State<AppState>,
    member: MemberId,
) -> AppResult<Json<AssistantUsageSummary>> {
    Ok(Json(state.store.assistant_usage_summary(member.0).await?))
}
