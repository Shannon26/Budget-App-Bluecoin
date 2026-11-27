use crate::app::AppState;
use crate::error::AppResult;
use crate::models::*;
use axum::extract::Query;
use axum::extract::State;
use axum::Json;

pub(crate) async fn net_earnings(
    State(state): State<AppState>,
    Query(query): Query<NetEarningsReportQuery>,
) -> AppResult<Json<NetEarningsReport>> {
    Ok(Json(state.store.net_earnings_report(&query.month).await?))
}
