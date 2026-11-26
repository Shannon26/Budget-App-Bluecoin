use crate::app::AppState;
use crate::csv;
use crate::error::AppResult;
use crate::models::*;
use axum::extract::Query;
use axum::extract::State;
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::Json;

pub(crate) async fn ledger(
    State(state): State<AppState>,
    Query(page): Query<LedgerPageQuery>,
) -> AppResult<Json<LedgerPage>> {
    let limit = page.limit;
    let offset = page.offset;
    Ok(Json(
        state
            .store
            .ledger(page.into_filter(), limit, offset)
            .await?,
    ))
}

pub(crate) async fn export_ledger(
    State(state): State<AppState>,
    Query(page): Query<LedgerPageQuery>,
) -> AppResult<Response> {
    let entries = state.store.ledger_entries(page.into_filter()).await?;
    let body = csv::ledger_csv(&entries);
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"paisa-ledger.csv\"",
            ),
        ],
        body,
    )
        .into_response())
}
