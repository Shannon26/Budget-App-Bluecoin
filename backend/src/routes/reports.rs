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

pub(crate) async fn items_summary(
    State(state): State<AppState>,
    Query(query): Query<ItemsSummaryReportQuery>,
) -> AppResult<Json<ItemsSummaryReport>> {
    Ok(Json(state.store.items_summary_report(&query.month).await?))
}

pub(crate) async fn daily_summary(
    State(state): State<AppState>,
    Query(query): Query<DailySummaryReportQuery>,
) -> AppResult<Json<DailySummaryReport>> {
    Ok(Json(
        state.store.daily_summary_report(&query.anchor_date).await?,
    ))
}
