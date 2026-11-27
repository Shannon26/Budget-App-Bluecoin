use crate::store::Store;
use axum::http::{header, Method};
use axum::routing::{get, post};
use axum::Router;
use sqlx::SqlitePool;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::routes;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) store: Store,
}

pub(crate) fn build_router(pool: SqlitePool) -> Router {
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
        .route("/api/health", get(routes::system::health))
        .route("/api/state", get(routes::system::get_state))
        .route("/api/accounts", post(routes::accounts::create_account))
        .route(
            "/api/accounts/:id/history",
            get(routes::accounts::account_history),
        )
        .route(
            "/api/accounts/:id",
            axum::routing::put(routes::accounts::update_account),
        )
        .route(
            "/api/transactions",
            post(routes::transactions::create_transaction),
        )
        .route(
            "/api/transactions/:id",
            get(routes::transactions::transaction_detail)
                .put(routes::transactions::update_transaction)
                .delete(routes::transactions::delete_transaction),
        )
        .route(
            "/api/transactions/:id/duplicate",
            post(routes::transactions::duplicate_transaction),
        )
        .route(
            "/api/transactions/:id/reconciliation",
            axum::routing::patch(routes::transactions::reconcile_transaction),
        )
        .route("/api/transfers", post(routes::transfers::create_transfer))
        .route(
            "/api/transfers/:id",
            get(routes::transfers::transfer_detail)
                .put(routes::transfers::update_transfer)
                .delete(routes::transfers::delete_transfer),
        )
        .route(
            "/api/transfers/:id/duplicate",
            post(routes::transfers::duplicate_transfer),
        )
        .route(
            "/api/transfers/:id/reconciliation",
            axum::routing::patch(routes::transfers::reconcile_transfer),
        )
        .route("/api/ledger", get(routes::ledger::ledger))
        .route(
            "/api/ledger/invalid-date-expenses",
            get(routes::ledger::invalid_date_expenses),
        )
        .route("/api/ledger/export", get(routes::ledger::export_ledger))
        .route(
            "/api/reports/net-earnings",
            get(routes::reports::net_earnings),
        )
        .route(
            "/api/reports/items-summary",
            get(routes::reports::items_summary),
        )
        .route(
            "/api/reports/daily-summary",
            get(routes::reports::daily_summary),
        )
        .route(
            "/api/reports/balance-sheet",
            get(routes::reports::balance_sheet),
        )
        .route(
            "/api/categories",
            get(routes::categories::list_categories).post(routes::categories::create_category),
        )
        .route(
            "/api/categories/:id",
            axum::routing::put(routes::categories::update_category)
                .delete(routes::categories::delete_category),
        )
        .route(
            "/api/category-budgets",
            get(routes::budgets::list_category_budgets),
        )
        .route(
            "/api/category-budgets/:category_id",
            axum::routing::put(routes::budgets::set_category_budget)
                .delete(routes::budgets::remove_category_budget),
        )
        .route(
            "/api/filters",
            get(routes::filters::list_saved_filters).post(routes::filters::create_saved_filter),
        )
        .route(
            "/api/filters/:id",
            axum::routing::put(routes::filters::update_saved_filter)
                .delete(routes::filters::delete_saved_filter),
        )
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(AppState {
            store: Store::new(pool),
        })
}
