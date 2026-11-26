use crate::app::AppState;
use crate::error::AppResult;
use crate::models::*;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::Json;

pub(crate) async fn list_category_budgets(
    State(state): State<AppState>,
    Query(query): Query<CategoryBudgetQuery>,
) -> AppResult<Json<MonthlyCategoryBudgets>> {
    Ok(Json(
        state.store.monthly_category_budgets(&query.month).await?,
    ))
}

pub(crate) async fn set_category_budget(
    State(state): State<AppState>,
    Path(category_id): Path<i64>,
    Query(query): Query<CategoryBudgetQuery>,
    Json(request): Json<CategoryBudgetRequest>,
) -> AppResult<Json<MonthlyCategoryBudgets>> {
    Ok(Json(
        state
            .store
            .set_category_budget(category_id, &query.month, request.amount_minor)
            .await?,
    ))
}

pub(crate) async fn remove_category_budget(
    State(state): State<AppState>,
    Path(category_id): Path<i64>,
    Query(query): Query<CategoryBudgetQuery>,
) -> AppResult<Json<MonthlyCategoryBudgets>> {
    Ok(Json(
        state
            .store
            .remove_category_budget(category_id, &query.month)
            .await?,
    ))
}
