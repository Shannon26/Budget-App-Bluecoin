use crate::assistant::AssistantClient;
use crate::error::{AppError, AppResult};
use crate::models::MemberId;
use crate::store::Store;
use axum::http::request::Parts;
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
    pub(crate) identity: IdentityConfig,
    pub(crate) assistant: AssistantClient,
}

/// The trusted identity boundary. Household mode is opt-in so all legacy
/// callers and tests keep the single-member behavior they already rely on.
/// Production sign-in is a separate deployment decision; the shared token is
/// a placeholder that only the configured deployment can supply.
#[derive(Clone, Debug)]
pub(crate) struct IdentityConfig {
    pub(crate) household_enabled: bool,
    pub(crate) shared_token: Option<String>,
    pub(crate) default_member_id: i64,
}

impl Default for IdentityConfig {
    fn default() -> Self {
        Self {
            household_enabled: false,
            shared_token: None,
            default_member_id: 1,
        }
    }
}

impl IdentityConfig {
    pub(crate) fn from_env() -> Self {
        let household_enabled = std::env::var("PAISA_HOUSEHOLD_ENABLED")
            .map(|value| value == "true" || value == "1")
            .unwrap_or(false);
        let shared_token = std::env::var("PAISA_MEMBER_TOKEN")
            .ok()
            .filter(|value| !value.is_empty());
        let default_member_id = std::env::var("PAISA_DEFAULT_MEMBER_ID")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(1);
        Self {
            household_enabled,
            shared_token,
            default_member_id,
        }
    }

    pub(crate) fn resolve_member(&self, parts: &Parts) -> AppResult<MemberId> {
        if !self.household_enabled {
            return Ok(MemberId(self.default_member_id));
        }
        let expected = self.shared_token.as_deref().ok_or_else(|| {
            AppError::Unauthorized("household access is not configured".to_string())
        })?;
        let token = parts
            .headers
            .get("x-paisa-member-token")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        if token != expected {
            return Err(AppError::Unauthorized(
                "a valid member token is required".to_string(),
            ));
        }
        let raw = parts
            .headers
            .get("x-paisa-member")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| AppError::Unauthorized("a member identity is required".to_string()))?;
        let id: i64 = raw
            .parse()
            .map_err(|_| AppError::BadRequest("invalid member identity".to_string()))?;
        if id <= 0 {
            return Err(AppError::BadRequest("invalid member identity".to_string()));
        }
        Ok(MemberId(id))
    }
}

#[allow(dead_code)]
pub(crate) fn build_router(pool: SqlitePool) -> Router {
    build_router_with(pool, IdentityConfig::default(), AssistantClient::default())
}

pub(crate) fn build_router_with(
    pool: SqlitePool,
    identity: IdentityConfig,
    assistant: AssistantClient,
) -> Router {
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
        .allow_headers([
            header::CONTENT_TYPE,
            header::HeaderName::from_static("x-paisa-member"),
            header::HeaderName::from_static("x-paisa-member-token"),
        ]);

    Router::new()
        .route("/api/health", get(routes::system::health))
        .route("/api/state", get(routes::system::get_state))
        .route(
            "/api/assistant/usage",
            get(routes::assistant_usage::usage_summary),
        )
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
        .route("/api/reports/labels", get(routes::reports::label_report))
        .route(
            "/api/reports/daily-summary",
            get(routes::reports::daily_summary),
        )
        .route(
            "/api/reports/balance-sheet",
            get(routes::reports::balance_sheet),
        )
        .route(
            "/api/reports/cash-flow-trend",
            get(routes::reports::cash_flow_trend),
        )
        .route(
            "/api/reports/credit-card",
            get(routes::reports::credit_card),
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
            "/api/overall-budget",
            axum::routing::put(routes::budgets::set_overall_budget)
                .delete(routes::budgets::remove_overall_budget),
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
            "/api/labels",
            get(routes::labels::list_labels).post(routes::labels::create_label),
        )
        .route(
            "/api/labels/:id",
            axum::routing::put(routes::labels::update_label),
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
        // Command Center: private drafts, atomic batch save, transfer requests.
        .route("/api/draft", get(routes::drafts::load_draft))
        .route("/api/draft/prompt", post(routes::drafts::append_prompt))
        .route("/api/draft/rows", post(routes::drafts::add_row))
        .route(
            "/api/draft/rows/:row_id",
            axum::routing::put(routes::drafts::replace_row).delete(routes::drafts::remove_row),
        )
        .route(
            "/api/draft/rows/assign-account",
            post(routes::drafts::assign_account),
        )
        .route(
            "/api/draft/remember-alias",
            post(routes::drafts::remember_alias),
        )
        .route("/api/draft/discard", post(routes::drafts::discard))
        .route("/api/draft/save", post(routes::drafts::save))
        .route(
            "/api/transfer-requests",
            get(routes::transfer_requests::list_requests),
        )
        .route(
            "/api/transfer-requests/:id/approve",
            post(routes::transfer_requests::approve),
        )
        .route(
            "/api/transfer-requests/:id/decline",
            post(routes::transfer_requests::decline),
        )
        .route(
            "/api/transfer-requests/:id/cancel",
            post(routes::transfer_requests::cancel),
        )
        .route(
            "/api/transfers/:id/reverse",
            post(routes::transfer_requests::reverse),
        )
        .route(
            "/api/household/members",
            get(routes::household::list_members),
        )
        .route(
            "/api/household/accounts",
            get(routes::household::accounts_view),
        )
        .route(
            "/api/household/assignments",
            get(routes::household::assignments),
        )
        .route(
            "/api/household/assignments/propose",
            post(routes::household::propose_assignments),
        )
        .route(
            "/api/household/assignments/confirm",
            post(routes::household::confirm_assignments),
        )
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(AppState {
            store: Store::new(pool),
            identity,
            assistant,
        })
}
