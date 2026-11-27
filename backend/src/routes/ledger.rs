use crate::app::AppState;
use crate::csv;
use crate::error::AppResult;
use crate::models::*;
use axum::extract::Query;
use axum::extract::State;
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::Json;

async fn ensure_household_member(state: &AppState, member: MemberId) -> AppResult<()> {
    if state.identity.household_enabled && !state.store.member_exists(member.0).await? {
        return Err(crate::error::AppError::Unauthorized(
            "household membership is required".to_string(),
        ));
    }
    Ok(())
}

pub(crate) async fn ledger(
    State(state): State<AppState>,
    member: MemberId,
    Query(page): Query<LedgerPageQuery>,
) -> AppResult<Json<LedgerPage>> {
    ensure_household_member(&state, member).await?;
    let limit = page.limit;
    let offset = page.offset;
    Ok(Json(
        state
            .store
            .ledger(page.into_filter(), limit, offset)
            .await?,
    ))
}

pub(crate) async fn invalid_date_expenses(
    State(state): State<AppState>,
    member: MemberId,
    Query(page): Query<LedgerPaginationQuery>,
) -> AppResult<Json<LedgerPage>> {
    ensure_household_member(&state, member).await?;
    Ok(Json(
        state
            .store
            .invalid_date_expenses(page.limit, page.offset)
            .await?,
    ))
}

pub(crate) async fn export_ledger(
    State(state): State<AppState>,
    member: MemberId,
    Query(page): Query<LedgerPageQuery>,
) -> AppResult<Response> {
    ensure_household_member(&state, member).await?;
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
