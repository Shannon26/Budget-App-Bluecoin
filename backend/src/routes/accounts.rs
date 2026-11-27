use crate::app::AppState;
use crate::error::AppResult;
use crate::models::*;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

pub(crate) async fn create_account(
    State(state): State<AppState>,
    Json(request): Json<CreateAccountRequest>,
) -> AppResult<(StatusCode, Json<Account>)> {
    Ok((
        StatusCode::CREATED,
        Json(state.store.create_account(request).await?),
    ))
}

pub(crate) async fn account_history(
    State(state): State<AppState>,
    Path(account_id): Path<i64>,
    Query(page): Query<LedgerPageQuery>,
) -> AppResult<Json<LedgerPage>> {
    Ok(Json(
        state
            .store
            .account_history(account_id, page.limit, page.offset)
            .await?,
    ))
}

pub(crate) async fn update_account(
    State(state): State<AppState>,
    Path(account_id): Path<i64>,
    Json(request): Json<UpdateAccountRequest>,
) -> AppResult<Json<Account>> {
    Ok(Json(state.store.update_account(account_id, request).await?))
}
