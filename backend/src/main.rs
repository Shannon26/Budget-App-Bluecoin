mod error;
mod models;
mod store;

#[cfg(test)]
mod tests;

use axum::extract::{Path, Query, State};
use axum::http::{header, Method, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use error::AppResult;
use models::*;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;
use store::Store;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
struct AppState {
    store: Store,
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn get_state(State(state): State<AppState>) -> AppResult<Json<StateResponse>> {
    Ok(Json(state.store.state().await?))
}

async fn create_account(
    State(state): State<AppState>,
    Json(request): Json<CreateAccountRequest>,
) -> AppResult<(StatusCode, Json<Account>)> {
    Ok((
        StatusCode::CREATED,
        Json(state.store.create_account(request).await?),
    ))
}

async fn account_history(
    State(state): State<AppState>,
    Path(account_id): Path<i64>,
) -> AppResult<Json<Vec<LedgerEntry>>> {
    Ok(Json(state.store.account_history(account_id).await?))
}

async fn create_transaction(
    State(state): State<AppState>,
    Json(request): Json<TransactionRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let id = state.store.create_transaction(request).await?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

async fn update_transaction(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<TransactionRequest>,
) -> AppResult<Json<serde_json::Value>> {
    state.store.update_transaction(id, request).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn transaction_detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<LedgerEntry>> {
    Ok(Json(state.store.transaction_detail(id).await?))
}

async fn duplicate_transaction(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let id = state.store.duplicate_transaction(id).await?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

async fn delete_transaction(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<serde_json::Value>> {
    state.store.delete_transaction(id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn reconcile_transaction(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<ReconciliationRequest>,
) -> AppResult<Json<serde_json::Value>> {
    state
        .store
        .set_transaction_reconciled(id, request.reconciled)
        .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn create_transfer(
    State(state): State<AppState>,
    Json(request): Json<TransferRequest>,
) -> AppResult<(StatusCode, Json<TransferResponse>)> {
    Ok((
        StatusCode::CREATED,
        Json(state.store.create_transfer(request).await?),
    ))
}

async fn update_transfer(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<TransferRequest>,
) -> AppResult<Json<serde_json::Value>> {
    state.store.update_transfer(id, request).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn transfer_detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<LedgerEntry>> {
    Ok(Json(state.store.transfer_detail(id).await?))
}

async fn duplicate_transfer(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let id = state.store.duplicate_transfer(id).await?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

async fn delete_transfer(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<serde_json::Value>> {
    state.store.delete_transfer(id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn reconcile_transfer(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<ReconciliationRequest>,
) -> AppResult<Json<serde_json::Value>> {
    state
        .store
        .set_transfer_reconciled(id, request.reconciled)
        .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn ledger(
    State(state): State<AppState>,
    Query(filter): Query<FilterDefinition>,
) -> AppResult<Json<Vec<LedgerEntry>>> {
    Ok(Json(state.store.ledger(filter).await?))
}

async fn list_categories(State(state): State<AppState>) -> AppResult<Json<Vec<Category>>> {
    Ok(Json(state.store.categories().await?))
}

async fn create_category(
    State(state): State<AppState>,
    Json(request): Json<CategoryRequest>,
) -> AppResult<(StatusCode, Json<Category>)> {
    Ok((
        StatusCode::CREATED,
        Json(state.store.create_category(request).await?),
    ))
}

async fn update_category(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<CategoryRequest>,
) -> AppResult<Json<Category>> {
    Ok(Json(state.store.update_category(id, request).await?))
}

async fn delete_category(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<DeleteCategoryResponse>> {
    Ok(Json(state.store.delete_category(id).await?))
}

async fn list_saved_filters(State(state): State<AppState>) -> AppResult<Json<Vec<SavedFilter>>> {
    Ok(Json(state.store.saved_filters().await?))
}

async fn create_saved_filter(
    State(state): State<AppState>,
    Json(request): Json<SavedFilterRequest>,
) -> AppResult<(StatusCode, Json<SavedFilter>)> {
    Ok((
        StatusCode::CREATED,
        Json(state.store.create_saved_filter(request).await?),
    ))
}

async fn update_saved_filter(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<SavedFilterRequest>,
) -> AppResult<Json<SavedFilter>> {
    Ok(Json(state.store.update_saved_filter(id, request).await?))
}

async fn delete_saved_filter(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<serde_json::Value>> {
    state.store.delete_saved_filter(id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

fn build_router(pool: SqlitePool) -> Router {
    let cors = CorsLayer::new()
        .allow_origin([
            "http://127.0.0.1:5173".parse().unwrap(),
            "http://localhost:5173".parse().unwrap(),
        ])
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::CONTENT_TYPE]);

    Router::new()
        .route("/api/health", get(health))
        .route("/api/state", get(get_state))
        .route("/api/accounts", post(create_account))
        .route("/api/accounts/:id/history", get(account_history))
        .route("/api/transactions", post(create_transaction))
        .route(
            "/api/transactions/:id",
            get(transaction_detail)
                .put(update_transaction)
                .delete(delete_transaction),
        )
        .route(
            "/api/transactions/:id/duplicate",
            post(duplicate_transaction),
        )
        .route(
            "/api/transactions/:id/reconciliation",
            axum::routing::patch(reconcile_transaction),
        )
        .route("/api/transfers", post(create_transfer))
        .route(
            "/api/transfers/:id",
            get(transfer_detail)
                .put(update_transfer)
                .delete(delete_transfer),
        )
        .route("/api/transfers/:id/duplicate", post(duplicate_transfer))
        .route(
            "/api/transfers/:id/reconciliation",
            axum::routing::patch(reconcile_transfer),
        )
        .route("/api/ledger", get(ledger))
        .route(
            "/api/categories",
            get(list_categories).post(create_category),
        )
        .route(
            "/api/categories/:id",
            axum::routing::put(update_category).delete(delete_category),
        )
        .route(
            "/api/filters",
            get(list_saved_filters).post(create_saved_filter),
        )
        .route(
            "/api/filters/:id",
            axum::routing::put(update_saved_filter).delete(delete_saved_filter),
        )
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(AppState {
            store: Store::new(pool),
        })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,budget_backend=debug,tower_http=warn".into()),
        )
        .init();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://budget.db?mode=rwc".to_string());
    let options = SqliteConnectOptions::from_str(&database_url)?
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    let app = build_router(pool);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    tracing::info!("budget API listening on http://127.0.0.1:3000");
    axum::serve(listener, app).await?;
    Ok(())
}
