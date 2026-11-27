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
    member: MemberId,
    Json(request): Json<CreateAccountRequest>,
) -> AppResult<(StatusCode, Json<Account>)> {
    Ok((
        StatusCode::CREATED,
        Json(state.store.create_account_owned(request, member.0).await?),
    ))
}

pub(crate) async fn account_history(
    State(state): State<AppState>,
    member: MemberId,
    Path(account_id): Path<i64>,
    Query(page): Query<LedgerPageQuery>,
) -> AppResult<Json<LedgerPage>> {
    if state.identity.household_enabled && !state.store.member_exists(member.0).await? {
        return Err(crate::error::AppError::Unauthorized(
            "household membership is required".to_string(),
        ));
    }
    if page
        .account_id
        .is_some_and(|filter_account_id| filter_account_id != account_id)
    {
        return Err(crate::error::AppError::BadRequest(
            "account_id filter must match the account path".to_string(),
        ));
    }
    let limit = page.limit;
    let offset = page.offset;
    let filter = page.into_filter();
    Ok(Json(
        state
            .store
            .account_history(account_id, filter, limit, offset)
            .await?,
    ))
}

pub(crate) async fn update_account(
    State(state): State<AppState>,
    member: MemberId,
    Path(account_id): Path<i64>,
    Json(request): Json<UpdateAccountRequest>,
) -> AppResult<Json<Account>> {
    state
        .store
        .ensure_account_write(state.identity.household_enabled, member.0, account_id)
        .await?;
    Ok(Json(state.store.update_account(account_id, request).await?))
}
