mod error;
mod models;
mod store;

#[cfg(test)]
mod tests;

use axum::extract::State;
use axum::http::{header, Method, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use error::AppResult;
use models::{
    Account, CreateAccountRequest, CreateTransactionRequest, CreateTransferRequest, StateResponse,
    TransferResponse,
};
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

async fn create_transaction(
    State(state): State<AppState>,
    Json(request): Json<CreateTransactionRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let id = state.store.create_transaction(request).await?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

async fn create_transfer(
    State(state): State<AppState>,
    Json(request): Json<CreateTransferRequest>,
) -> AppResult<(StatusCode, Json<TransferResponse>)> {
    Ok((
        StatusCode::CREATED,
        Json(state.store.create_transfer(request).await?),
    ))
}

fn build_router(pool: SqlitePool) -> Router {
    let cors = CorsLayer::new()
        .allow_origin([
            "http://127.0.0.1:5173".parse().unwrap(),
            "http://localhost:5173".parse().unwrap(),
        ])
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE]);

    Router::new()
        .route("/api/health", get(health))
        .route("/api/state", get(get_state))
        .route("/api/accounts", post(create_account))
        .route("/api/transactions", post(create_transaction))
        .route("/api/transfers", post(create_transfer))
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
