use super::*;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::str::FromStr;
use tempfile::TempDir;
use tower::ServiceExt;

async fn setup() -> (Router, SqlitePool, TempDir) {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("test.db");
    let url = format!("sqlite://{}?mode=rwc", database_path.display());
    let options = SqliteConnectOptions::from_str(&url)
        .unwrap()
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(3)
        .connect_with(options)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (build_router(pool.clone()), pool, directory)
}

async fn send_json(app: &Router, method: &str, uri: &str, body: Value) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

async fn send_csv(app: &Router, uri: &str) -> (StatusCode, String, String, Vec<u8>) {
    let request = Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let content_disposition = response
        .headers()
        .get(header::CONTENT_DISPOSITION)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, content_type, content_disposition, bytes.to_vec())
}

async fn create_account(app: &Router, name: &str, opening_balance_minor: i64) -> i64 {
    let (status, body) = send_json(
        app,
        "POST",
        "/api/accounts",
        json!({
            "name": name,
            "currency": "INR",
            "opening_balance_minor": opening_balance_minor
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_i64().unwrap()
}

async fn state(app: &Router) -> Value {
    let (status, body) = send_json(app, "GET", "/api/state", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

async fn transfer(app: &Router, from: i64, to: i64, amount: i64) -> (StatusCode, Value) {
    send_json(
        app,
        "POST",
        "/api/transfers",
        json!({
            "from_account_id": from,
            "to_account_id": to,
            "amount_minor": amount,
            "note": "milestone transfer"
        }),
    )
    .await
}

#[tokio::test]
async fn non_inr_accounts_are_rejected() {
    let (app, pool, _directory) = setup().await;
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/accounts",
        json!({
            "name": "US Dollars",
            "currency": "USD",
            "opening_balance_minor": 10000
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        body["error"],
        "only INR accounts are supported in this milestone"
    );

    let account_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(account_count, 0);
}

#[tokio::test]
async fn acceptance_balances_survive_reload_and_totals_stay_zero() {
    let (app, pool, _directory) = setup().await;
    let first = create_account(&app, "Cash", 100_000).await;
    let second = create_account(&app, "Wallet", 20_000).await;

    let (status, body) = transfer(&app, first, second, 25_000).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let reloaded = state(&app).await;
    assert_eq!(reloaded["accounts"][0]["balance_minor"], 75_000);
    assert_eq!(reloaded["accounts"][1]["balance_minor"], 45_000);
    assert_eq!(reloaded["totals"]["income_minor"], 0);
    assert_eq!(reloaded["totals"]["expense_minor"], 0);

    let transfer_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transfers")
        .fetch_one(&pool)
        .await
        .unwrap();
    let leg_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM entries WHERE kind IN ('transfer_in', 'transfer_out')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(transfer_count, 1);
    assert_eq!(leg_count, 2);
}

#[tokio::test]
async fn income_and_expense_totals_do_not_change_on_transfer() {
    let (app, _pool, _directory) = setup().await;
    let first = create_account(&app, "Checking", 100_000).await;
    let second = create_account(&app, "Savings", 20_000).await;

    for (kind, amount) in [("income", 10_000), ("expense", 4_000)] {
        let (status, body) = send_json(
            &app,
            "POST",
            "/api/transactions",
            json!({
                "kind": kind,
                "account_id": first,
                "amount_minor": amount,
                "note": kind
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }

    let before = state(&app).await;
    assert_eq!(before["totals"]["income_minor"], 10_000);
    assert_eq!(before["totals"]["expense_minor"], 4_000);
    assert_eq!(before["accounts"][0]["balance_minor"], 106_000);

    let (status, body) = transfer(&app, first, second, 25_000).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let after = state(&app).await;
    assert_eq!(
        after["totals"]["income_minor"],
        before["totals"]["income_minor"]
    );
    assert_eq!(
        after["totals"]["expense_minor"],
        before["totals"]["expense_minor"]
    );
    assert_eq!(after["accounts"][0]["balance_minor"], 81_000);
    assert_eq!(after["accounts"][1]["balance_minor"], 45_000);
}

#[tokio::test]
async fn invalid_transfers_are_rejected_without_side_effects() {
    let (app, pool, _directory) = setup().await;
    let first = create_account(&app, "Cash", 100_000).await;
    let second = create_account(&app, "Wallet", 20_000).await;
    let before = state(&app).await;

    let (same_status, same_body) = transfer(&app, first, first, 25_000).await;
    assert_eq!(same_status, StatusCode::BAD_REQUEST, "{same_body}");

    let (zero_status, zero_body) = transfer(&app, first, second, 0).await;
    assert_eq!(zero_status, StatusCode::BAD_REQUEST, "{zero_body}");

    let (missing_status, missing_body) = transfer(&app, first, 999_999, 25_000).await;
    assert_eq!(missing_status, StatusCode::NOT_FOUND, "{missing_body}");

    let after = state(&app).await;
    assert_eq!(after["accounts"], before["accounts"]);
    let transfer_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transfers")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(transfer_count, 0);
}

#[tokio::test]
async fn transfer_header_and_both_legs_roll_back_together() {
    let (app, pool, _directory) = setup().await;
    let first = create_account(&app, "Cash", 100_000).await;
    let second = create_account(&app, "Wallet", 20_000).await;
    let before = state(&app).await;

    sqlx::query(
        "CREATE TRIGGER fail_transfer_in
         BEFORE INSERT ON entries
         WHEN NEW.kind = 'transfer_in'
         BEGIN
             SELECT RAISE(ABORT, 'forced transfer leg failure');
         END",
    )
    .execute(&pool)
    .await
    .unwrap();

    let (status, body) = transfer(&app, first, second, 25_000).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");

    let after = state(&app).await;
    assert_eq!(after["accounts"], before["accounts"]);
    assert_eq!(after["entries"].as_array().unwrap().len(), 0);

    let transfer_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transfers")
        .fetch_one(&pool)
        .await
        .unwrap();
    let leg_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM entries WHERE kind IN ('transfer_in', 'transfer_out')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(transfer_count, 0);
    assert_eq!(leg_count, 0);
}

async fn category_id(app: &Router, kind: &str, name: &str) -> i64 {
    let current = state(app).await;
    current["categories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|category| category["kind"] == kind && category["name"] == name)
        .unwrap()["id"]
        .as_i64()
        .unwrap()
}

async fn create_entry(
    app: &Router,
    kind: &str,
    account_id: i64,
    amount_minor: i64,
    name: &str,
    category_id: Option<i64>,
) -> i64 {
    let (status, body) = send_json(
        app,
        "POST",
        "/api/transactions",
        json!({
            "kind": kind,
            "account_id": account_id,
            "amount_minor": amount_minor,
            "name": name,
            "category_id": category_id,
            "note": name
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_i64().unwrap()
}

async fn create_entry_at(
    app: &Router,
    kind: &str,
    account_id: i64,
    amount_minor: i64,
    name: &str,
    category_id: Option<i64>,
    occurred_at: &str,
) -> i64 {
    let (status, body) = send_json(
        app,
        "POST",
        "/api/transactions",
        json!({
            "kind": kind,
            "account_id": account_id,
            "amount_minor": amount_minor,
            "name": name,
            "category_id": category_id,
            "occurred_at": occurred_at
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_i64().unwrap()
}

async fn monthly_budgets(app: &Router, month: &str) -> Value {
    let (status, body) = send_json(
        app,
        "GET",
        &format!("/api/category-budgets?month={month}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

async fn set_budget(
    app: &Router,
    category_id: i64,
    month: &str,
    amount_minor: i64,
) -> (StatusCode, Value) {
    send_json(
        app,
        "PUT",
        &format!("/api/category-budgets/{category_id}?month={month}"),
        json!({ "amount_minor": amount_minor }),
    )
    .await
}

fn budget_row(body: &Value, category_id: i64) -> &Value {
    body["categories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|category| category["category_id"] == category_id)
        .unwrap()
}

#[tokio::test]
async fn starter_catalog_supports_dining_and_salary() {
    let (app, _pool, _directory) = setup().await;
    let dining = category_id(&app, "expense", "Dining Out").await;
    let salary = category_id(&app, "income", "Salary").await;
    assert!(dining > 0);
    assert!(salary > 0);
    let current = state(&app).await;
    assert!(current["categories"].as_array().unwrap().len() >= 50);
}

#[tokio::test]
async fn edit_duplicate_delete_and_uncategorized_keep_balances_and_totals_correct() {
    let (app, _pool, _directory) = setup().await;
    let checking = create_account(&app, "Checking", 100_000).await;
    let dining = category_id(&app, "expense", "Dining Out").await;
    let salary = category_id(&app, "income", "Salary").await;

    let expense = create_entry(&app, "expense", checking, 25_000, "Dinner", Some(dining)).await;
    create_entry(
        &app,
        "income",
        checking,
        100_000,
        "Monthly salary",
        Some(salary),
    )
    .await;
    let current = state(&app).await;
    assert_eq!(current["accounts"][0]["balance_minor"], 175_000);
    assert_eq!(current["totals"]["expense_minor"], 25_000);
    assert_eq!(current["totals"]["income_minor"], 100_000);

    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/transactions/{expense}"),
        json!({
            "kind": "expense",
            "account_id": checking,
            "amount_minor": 30_000,
            "name": "Dinner updated",
            "category_id": dining,
            "note": "Dinner updated"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let current = state(&app).await;
    assert_eq!(current["accounts"][0]["balance_minor"], 170_000);
    assert_eq!(current["totals"]["expense_minor"], 30_000);
    assert!(current["category_totals"]
        .as_array()
        .unwrap()
        .iter()
        .any(|total| total["category_id"] == dining && total["total_minor"] == 30_000));

    let (status, body) = send_json(
        &app,
        "POST",
        &format!("/api/transactions/{expense}/duplicate"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let duplicate = body["id"].as_i64().unwrap();
    let current = state(&app).await;
    assert_eq!(current["accounts"][0]["balance_minor"], 140_000);
    assert_eq!(current["totals"]["expense_minor"], 60_000);

    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/transactions/{duplicate}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let current = state(&app).await;
    assert_eq!(current["accounts"][0]["balance_minor"], 170_000);
    assert_eq!(current["totals"]["expense_minor"], 30_000);

    create_entry(
        &app,
        "expense",
        checking,
        5_000,
        "Uncategorized snack",
        None,
    )
    .await;
    let current = state(&app).await;
    assert_eq!(current["accounts"][0]["balance_minor"], 165_000);
    assert!(current["category_totals"]
        .as_array()
        .unwrap()
        .iter()
        .any(|total| total["category_id"].is_null()
            && total["name"] == "Uncategorized"
            && total["total_minor"] == 5_000));
}

#[tokio::test]
async fn referenced_category_archives_and_preserves_history() {
    let (app, _pool, _directory) = setup().await;
    let checking = create_account(&app, "Checking", 100_000).await;
    let dining = category_id(&app, "expense", "Dining Out").await;
    create_entry(&app, "expense", checking, 25_000, "Dinner", Some(dining)).await;

    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/categories/{dining}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["archived"], true);

    let current = state(&app).await;
    let category = current["categories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|category| category["id"] == dining)
        .unwrap();
    assert!(!category["archived_at"].is_null());
    assert!(current["category_totals"]
        .as_array()
        .unwrap()
        .iter()
        .any(|total| total["category_id"] == dining && total["total_minor"] == 25_000));
    assert!(current["entries"][0]["category_name"] == "Dining Out");

    let (status, body) = send_json(
        &app,
        "POST",
        "/api/transactions",
        json!({
            "kind": "expense",
            "account_id": checking,
            "amount_minor": 1_000,
            "name": "New dinner",
            "category_id": dining
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn saved_filters_persist_and_reconciliation_is_filterable() {
    let (app, _pool, _directory) = setup().await;
    let checking = create_account(&app, "Checking", 100_000).await;
    let dining = category_id(&app, "expense", "Dining Out").await;
    let reconciled = create_entry(
        &app,
        "expense",
        checking,
        25_000,
        "Reconciled dinner",
        Some(dining),
    )
    .await;
    create_entry(&app, "expense", checking, 1_000, "Open snack", Some(dining)).await;

    let (status, body) = send_json(
        &app,
        "PATCH",
        &format!("/api/transactions/{reconciled}/reconciliation"),
        json!({ "reconciled": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body) = send_json(
        &app,
        "POST",
        "/api/filters",
        json!({
            "name": "Reconciled dining",
            "definition": {
                "category_id": dining,
                "reconciled": true
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert!(body["id"].as_i64().unwrap() > 0);

    let current = state(&app).await;
    assert_eq!(current["saved_filters"].as_array().unwrap().len(), 1);
    let (status, body) = send_json(
        &app,
        "GET",
        &format!("/api/ledger?category_id={dining}&reconciled=true"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["entries"].as_array().unwrap().len(), 1);
    assert_eq!(body["entries"][0]["id"], reconciled);
}

#[tokio::test]
async fn main_ledger_shows_one_transfer_and_account_histories_show_relevant_side() {
    let (app, _pool, _directory) = setup().await;
    let checking = create_account(&app, "Checking", 100_000).await;
    let savings = create_account(&app, "Savings", 20_000).await;
    let (status, body) = transfer(&app, checking, savings, 25_000).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let transfer_id = body["id"].as_i64().unwrap();

    let current = state(&app).await;
    assert_eq!(current["entries"].as_array().unwrap().len(), 1);
    assert_eq!(current["entries"][0]["kind"], "transfer");
    assert_eq!(current["entries"][0]["id"], transfer_id);

    let (status, body) = send_json(
        &app,
        "GET",
        &format!("/api/accounts/{checking}/history"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["entries"][0]["kind"], "transfer_out");
    let (status, body) = send_json(
        &app,
        "GET",
        &format!("/api/accounts/{savings}/history"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["entries"][0]["kind"], "transfer_in");

    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/transfers/{transfer_id}"),
        json!({
            "from_account_id": checking,
            "to_account_id": savings,
            "amount_minor": 30_000,
            "name": "Updated transfer",
            "reconciled": true
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let current = state(&app).await;
    assert_eq!(current["accounts"][0]["balance_minor"], 70_000);
    assert_eq!(current["accounts"][1]["balance_minor"], 50_000);
    assert_eq!(current["totals"]["income_minor"], 0);
    assert_eq!(current["totals"]["expense_minor"], 0);

    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/transfers/{transfer_id}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let current = state(&app).await;
    assert_eq!(current["accounts"][0]["balance_minor"], 100_000);
    assert_eq!(current["accounts"][1]["balance_minor"], 20_000);
    assert_eq!(current["entries"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn old_shape_entries_remain_uncategorized_and_keep_balances() {
    let (app, pool, _directory) = setup().await;
    let checking = create_account(&app, "Checking", 100_000).await;
    sqlx::query(
        "INSERT INTO entries (kind, account_id, amount_minor, note, occurred_at, created_at, transfer_id)
         VALUES ('expense', ?, 2500, 'Legacy snack', '2026-01-01T00:00:00', '2026-01-01T00:00:00', NULL)",
    )
    .bind(checking)
    .execute(&pool)
    .await
    .unwrap();
    let current = state(&app).await;
    assert_eq!(current["accounts"][0]["balance_minor"], 97_500);
    assert_eq!(current["entries"][0]["category_id"], Value::Null);
    assert_eq!(current["entries"][0]["reconciled"], false);
    assert!(current["category_totals"]
        .as_array()
        .unwrap()
        .iter()
        .any(|total| total["name"] == "Uncategorized" && total["total_minor"] == 2_500));
}

#[tokio::test]
async fn ledger_filters_cover_search_date_account_category_status_and_amount() {
    let (app, _pool, _directory) = setup().await;
    let checking = create_account(&app, "Checking", 100_000).await;
    let savings = create_account(&app, "Savings", 20_000).await;
    let dining = category_id(&app, "expense", "Dining Out").await;
    let target = create_entry(&app, "expense", checking, 25_000, "Dinner", Some(dining)).await;
    create_entry(&app, "expense", savings, 25_000, "Dinner", Some(dining)).await;
    create_entry(&app, "expense", checking, 5_000, "Snack", Some(dining)).await;

    let (status, body) = send_json(
        &app,
        "GET",
        &format!(
            "/api/ledger?search=dinner&date_from=2026-11-01&date_to=2026-11-30&account_id={checking}&category_id={dining}&reconciled=false&amount_min_minor=20000&amount_max_minor=30000"
        ),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let rows = body["entries"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"], target);
}

#[tokio::test]
async fn category_crud_prevents_cycles_and_deletes_unreferenced_categories() {
    let (app, _pool, _directory) = setup().await;
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/categories",
        json!({
            "name": "Pets",
            "kind": "expense",
            "parent_id": null,
            "color": "#426b8a",
            "icon": "🐾",
            "sort_order": 110
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let parent = body["id"].as_i64().unwrap();
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/categories",
        json!({
            "name": "Vet",
            "kind": "expense",
            "parent_id": parent,
            "color": "#426b8a",
            "icon": "🩺",
            "sort_order": 111
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let child = body["id"].as_i64().unwrap();

    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/categories/{parent}"),
        json!({
            "name": "Pets",
            "kind": "expense",
            "parent_id": child,
            "color": "#426b8a",
            "icon": "🐾",
            "sort_order": 110
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/categories/{child}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["archived"], false);
    let current = state(&app).await;
    assert!(!current["categories"]
        .as_array()
        .unwrap()
        .iter()
        .any(|category| category["id"] == child));
}

#[tokio::test]
async fn category_hierarchy_stays_two_levels_and_moves_preserve_assignments() {
    let (app, _pool, _directory) = setup().await;
    let food = category_id(&app, "expense", "Food & Drink").await;
    let shopping = category_id(&app, "expense", "Shopping").await;
    let seeded_dining = category_id(&app, "expense", "Dining Out").await;

    // Recreate the exact Food → Dining Out branch through the API.
    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/categories/{seeded_dining}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["archived"], false);
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/categories",
        json!({
            "name": "Dining Out",
            "kind": "expense",
            "parent_id": food,
            "color": "#b65d35",
            "icon": "🍴",
            "sort_order": 11
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let dining = body["id"].as_i64().unwrap();
    assert_eq!(body["parent_id"], food);

    // A third level is rejected.
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/categories",
        json!({
            "name": "Invalid third level",
            "kind": "expense",
            "parent_id": dining,
            "color": "#b65d35",
            "icon": "×",
            "sort_order": 12
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        body["error"],
        "subcategories can only be created under top-level categories"
    );

    // Food already has subcategories, so it cannot be moved beneath Shopping.
    let before = state(&app).await;
    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/categories/{food}"),
        json!({
            "name": "Food & Drink",
            "kind": "expense",
            "parent_id": shopping,
            "color": "#b65d35",
            "icon": "🍽",
            "sort_order": 10
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"], "Move this category’s subcategories first.");
    let after = state(&app).await;
    assert_eq!(after["categories"], before["categories"]);

    // Dining Out has no children, so it can move under another valid top-level category.
    let checking = create_account(&app, "Checking", 100_000).await;
    let expense = create_entry(&app, "expense", checking, 25_000, "Dinner", Some(dining)).await;
    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/categories/{dining}"),
        json!({
            "name": "Dining Out",
            "kind": "expense",
            "parent_id": shopping,
            "color": "#b65d35",
            "icon": "🍴",
            "sort_order": 11
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["parent_id"], shopping);
    let current = state(&app).await;
    assert_eq!(current["entries"][0]["id"], expense);
    assert_eq!(current["entries"][0]["category_id"], dining);
    assert_eq!(current["entries"][0]["category_name"], "Dining Out");
    assert!(current["category_totals"]
        .as_array()
        .unwrap()
        .iter()
        .any(|total| total["category_id"] == dining && total["total_minor"] == 25_000));

    // A childless category can also become top-level.
    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/categories/{dining}"),
        json!({
            "name": "Dining Out",
            "kind": "expense",
            "parent_id": null,
            "color": "#b65d35",
            "icon": "🍴",
            "sort_order": 11
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["parent_id"], Value::Null);
}

#[tokio::test]
async fn editing_an_entry_with_an_archived_category_preserves_the_assignment() {
    let (app, _pool, _directory) = setup().await;
    let checking = create_account(&app, "Checking", 100_000).await;
    let dining = category_id(&app, "expense", "Dining Out").await;
    let expense = create_entry(&app, "expense", checking, 25_000, "Dinner", Some(dining)).await;
    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/categories/{dining}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/transactions/{expense}"),
        json!({
            "kind": "expense",
            "account_id": checking,
            "amount_minor": 30_000,
            "name": "Dinner updated",
            "category_id": dining
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let current = state(&app).await;
    assert_eq!(current["entries"][0]["category_id"], dining);
    assert_eq!(current["entries"][0]["category_name"], "Dining Out");
}

#[tokio::test]
async fn transfer_edit_rolls_back_both_legs_when_one_side_fails() {
    let (app, pool, _directory) = setup().await;
    let checking = create_account(&app, "Checking", 100_000).await;
    let savings = create_account(&app, "Savings", 20_000).await;
    let (status, body) = transfer(&app, checking, savings, 25_000).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let transfer_id = body["id"].as_i64().unwrap();
    let before = state(&app).await;

    sqlx::query(
        "CREATE TRIGGER fail_transfer_update
         BEFORE UPDATE ON entries
         WHEN NEW.kind = 'transfer_in'
         BEGIN
             SELECT RAISE(ABORT, 'forced transfer update failure');
         END",
    )
    .execute(&pool)
    .await
    .unwrap();

    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/transfers/{transfer_id}"),
        json!({
            "from_account_id": checking,
            "to_account_id": savings,
            "amount_minor": 30_000,
            "name": "Should roll back"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    let after = state(&app).await;
    assert_eq!(after["accounts"], before["accounts"]);
    assert_eq!(after["entries"][0]["amount_minor"], 25_000);
}

#[tokio::test]
async fn ledger_pagination_has_stable_order_and_full_day_totals_for_201_entries() {
    let (app, pool, _directory) = setup().await;
    let checking = create_account(&app, "Pagination account", 100_000).await;
    sqlx::query(
        "WITH RECURSIVE numbers(value) AS (
             SELECT 1 UNION ALL SELECT value + 1 FROM numbers WHERE value < 201
         )
         INSERT INTO entries
             (kind, account_id, amount_minor, name, category_id, reconciled, note, occurred_at, created_at, transfer_id)
         SELECT 'expense', ?, 100, 'Pagination expense ' || value, NULL, 0, '',
                '2026-11-20T12:00:00', '2026-11-20T12:00:00Z', NULL
         FROM numbers",
    )
    .bind(checking)
    .execute(&pool)
    .await
    .unwrap();

    let (status, first) = send_json(
        &app,
        "GET",
        &format!("/api/ledger?account_id={checking}&search=Pagination&limit=200&offset=0"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["total_count"], 201);
    assert_eq!(first["entries"].as_array().unwrap().len(), 200);
    assert_eq!(first["next_offset"], 200);
    assert_eq!(first["daily_totals"].as_array().unwrap().len(), 1);
    assert_eq!(first["daily_totals"][0]["date"], "2026-11-20");
    assert_eq!(first["daily_totals"][0]["expense_minor"], 20_100);
    assert_eq!(first["daily_totals"][0]["income_minor"], 0);
    assert_eq!(first["daily_totals"][0]["transfer_minor"], 0);

    let (status, second) = send_json(
        &app,
        "GET",
        &format!("/api/ledger?account_id={checking}&search=Pagination&limit=200&offset=200"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(second["entries"].as_array().unwrap().len(), 1);
    assert_eq!(second["next_offset"], Value::Null);

    let mut ids = Vec::new();
    for page in [&first, &second] {
        for entry in page["entries"].as_array().unwrap() {
            ids.push(entry["id"].as_i64().unwrap());
        }
    }
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 201);

    let (status, history) = send_json(
        &app,
        "GET",
        &format!("/api/accounts/{checking}/history?limit=200&offset=0"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{history}");
    assert_eq!(history["total_count"], 201);
    assert_eq!(history["daily_totals"][0]["expense_minor"], 20_100);
}

#[tokio::test]
async fn csv_export_returns_every_filtered_row_and_ignores_pagination() {
    let (app, pool, _directory) = setup().await;
    let checking = create_account(&app, "CSV pagination account", 100_000).await;
    sqlx::query(
        "WITH RECURSIVE numbers(value) AS (
             SELECT 1 UNION ALL SELECT value + 1 FROM numbers WHERE value < 201
         )
         INSERT INTO entries
             (kind, account_id, amount_minor, name, category_id, reconciled, note, occurred_at, created_at, transfer_id)
         SELECT 'expense', ?, 100, 'CSV pagination ' || value, NULL, 0, '',
                '2026-11-20T12:00:00', '2026-11-20T12:00:00Z', NULL
         FROM numbers",
    )
    .bind(checking)
    .execute(&pool)
    .await
    .unwrap();
    create_entry(&app, "expense", checking, 999, "Excluded row", None).await;

    let (status, first) = send_json(
        &app,
        "GET",
        &format!("/api/ledger?account_id={checking}&search=CSV%20pagination&limit=1&offset=200"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["total_count"], 201);
    assert_eq!(first["entries"].as_array().unwrap().len(), 1);

    let (status, content_type, content_disposition, bytes) = send_csv(
        &app,
        &format!(
            "/api/ledger/export?account_id={checking}&search=CSV%20pagination&limit=1&offset=200"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type, "text/csv; charset=utf-8");
    assert_eq!(
        content_disposition,
        "attachment; filename=\"paisa-ledger.csv\""
    );
    let csv = String::from_utf8(bytes).unwrap();
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 202);
    assert_eq!(lines[0], csv::LEDGER_CSV_HEADERS.join(","));
    assert!(lines[1].contains("CSV pagination 201,"));
    assert!(lines[201].contains("CSV pagination 1,"));
    assert!(lines
        .iter()
        .skip(1)
        .all(|line| line.contains(",Expense,") && line.contains(",1.00,INR,Unreconciled,")));
    assert!(!csv.contains("Excluded row"));
}

#[tokio::test]
async fn csv_export_preserves_categories_transfers_and_special_text() {
    let (app, _pool, _directory) = setup().await;
    let checking = create_account(&app, "=Cash, \"Main\"", 100_000).await;
    let savings = create_account(&app, "Savings", 20_000).await;
    let dining = category_id(&app, "expense", "Dining Out").await;

    let (status, body) = send_json(
        &app,
        "POST",
        "/api/transactions",
        json!({
            "kind": "expense",
            "account_id": checking,
            "amount_minor": 1234,
            "name": "=SUM(1,2), \"lunch\"\nsecond line",
            "category_id": dining,
            "reconciled": true,
            "note": "@note\nwith, \"quotes\"",
            "occurred_at": "2026-11-20T10:00:00"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/categories/{dining}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["archived"], true);

    let (status, _content_type, _content_disposition, bytes) =
        send_csv(&app, "/api/ledger/export?search=SUM").await;
    assert_eq!(status, StatusCode::OK);
    let csv = String::from_utf8(bytes).unwrap();
    let expected = r#"Date,Type,Name,Account,Source Account,Destination Account,Category,Subcategory,Amount,Currency,Status,Note
2026-11-20,Expense,"'=SUM(1,2), ""lunch""
second line","'=Cash, ""Main""",,,Entertainment,Dining Out,12.34,INR,Reconciled,"'@note
with, ""quotes"""
"#;
    assert_eq!(csv, expected);

    let (status, body) = send_json(
        &app,
        "POST",
        "/api/transactions",
        json!({
            "kind": "income",
            "account_id": checking,
            "amount_minor": 500000,
            "name": "CSV Salary",
            "occurred_at": "2026-11-20T11:00:00"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let (status, body) = send_json(
        &app,
        "POST",
        "/api/transfers",
        json!({
            "from_account_id": checking,
            "to_account_id": savings,
            "amount_minor": 25000,
            "name": "+CSV transfer, \"move\"",
            "note": "-transfer note",
            "occurred_at": "2026-11-20T12:00:00"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let (status, _content_type, _content_disposition, bytes) =
        send_csv(&app, &format!("/api/ledger/export?account_id={checking}")).await;
    assert_eq!(status, StatusCode::OK);
    let csv = String::from_utf8(bytes).unwrap();
    assert_eq!(
        csv.lines()
            .filter(|line| line.contains(",Transfer,"))
            .count(),
        1
    );
    assert!(csv.contains(
        "2026-11-20,Transfer,\"'+CSV transfer, \"\"move\"\"\",,\"'=Cash, \"\"Main\"\"\",Savings,,,250.00,INR,Unreconciled,'-transfer note"
    ));
    assert!(csv.contains(
        "2026-11-20,Income,CSV Salary,\"'=Cash, \"\"Main\"\"\",,,Uncategorized,,5000.00,INR,Unreconciled,"
    ));
    assert!(!csv.contains("transfer_in"));
    assert!(!csv.contains("transfer_out"));
}

#[tokio::test]
async fn transaction_and_transfer_dates_are_validated_before_writes() {
    let (app, pool, _directory) = setup().await;
    let checking = create_account(&app, "Date checking", 100_000).await;
    let savings = create_account(&app, "Date savings", 20_000).await;
    let invalid_date_error =
        "occurred_at must be a valid date in YYYY-MM-DD or ISO 8601 date-time format";

    for invalid in ["not-a-date", "2026-02-30T12:00:00", "=2026-11-20"] {
        let (status, body) = send_json(
            &app,
            "POST",
            "/api/transactions",
            json!({
                "kind": "expense",
                "account_id": checking,
                "amount_minor": 100,
                "name": "Invalid date transaction",
                "occurred_at": invalid
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"], invalid_date_error);
    }
    let entry_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entries")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(entry_count, 0);

    let (status, body) = send_json(
        &app,
        "POST",
        "/api/transactions",
        json!({
            "kind": "expense",
            "account_id": checking,
            "amount_minor": 100,
            "name": "Valid date transaction",
            "occurred_at": "2026-11-20"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let transaction_id = body["id"].as_i64().unwrap();

    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/transactions/{transaction_id}"),
        json!({
            "kind": "expense",
            "account_id": checking,
            "amount_minor": 100,
            "name": "Invalid date update",
            "occurred_at": "2026-11-31T12:00:00"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"], invalid_date_error);
    let stored_date: String = sqlx::query_scalar("SELECT occurred_at FROM entries WHERE id = ?")
        .bind(transaction_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored_date, "2026-11-20");

    for invalid in ["not-a-date", "2026-02-30T12:00:00", "+2026-11-20"] {
        let (status, body) = send_json(
            &app,
            "POST",
            "/api/transfers",
            json!({
                "from_account_id": checking,
                "to_account_id": savings,
                "amount_minor": 100,
                "name": "Invalid date transfer",
                "occurred_at": invalid
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"], invalid_date_error);
    }
    let transfer_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transfers")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(transfer_count, 0);
    let leg_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM entries WHERE kind IN ('transfer_in', 'transfer_out')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(leg_count, 0);

    let (status, body) = send_json(
        &app,
        "POST",
        "/api/transfers",
        json!({
            "from_account_id": checking,
            "to_account_id": savings,
            "amount_minor": 100,
            "name": "Valid date transfer",
            "occurred_at": "2026-11-20T12:00:00Z"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let transfer_id = body["id"].as_i64().unwrap();

    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/transfers/{transfer_id}"),
        json!({
            "from_account_id": checking,
            "to_account_id": savings,
            "amount_minor": 100,
            "name": "Invalid transfer date update",
            "occurred_at": "2026-11-31T12:00:00"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"], invalid_date_error);
    let stored_date: String = sqlx::query_scalar("SELECT occurred_at FROM transfers WHERE id = ?")
        .bind(transfer_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored_date, "2026-11-20T12:00:00Z");
}

#[tokio::test]
async fn csv_export_omits_invalid_legacy_dates_without_leaking_formula_text() {
    let (app, pool, _directory) = setup().await;
    let checking = create_account(&app, "Legacy date checking", 100_000).await;
    let savings = create_account(&app, "Legacy date savings", 20_000).await;

    for (name, date) in [
        ("Legacy formula date", "2026-11-20"),
        ("Legacy impossible date", "2026-11-20"),
        ("Valid timestamp", "2026-11-21T23:59:59Z"),
        ("Valid date only", "2026-11-22"),
    ] {
        let (status, body) = send_json(
            &app,
            "POST",
            "/api/transactions",
            json!({
                "kind": "expense",
                "account_id": checking,
                "amount_minor": 100,
                "name": name,
                "occurred_at": date
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }
    for (name, invalid) in [
        ("Legacy formula date", "=2026-11-20"),
        ("Legacy impossible date", "2026-02-30T12:00:00"),
    ] {
        sqlx::query("UPDATE entries SET occurred_at = ? WHERE name = ?")
            .bind(invalid)
            .bind(name)
            .execute(&pool)
            .await
            .unwrap();
    }

    let (status, body) = send_json(
        &app,
        "POST",
        "/api/transfers",
        json!({
            "from_account_id": checking,
            "to_account_id": savings,
            "amount_minor": 100,
            "name": "Legacy invalid transfer",
            "occurred_at": "2026-11-23T12:00:00"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let transfer_id = body["id"].as_i64().unwrap();
    sqlx::query("UPDATE transfers SET occurred_at = ? WHERE id = ?")
        .bind("not-a-date")
        .bind(transfer_id)
        .execute(&pool)
        .await
        .unwrap();

    let (status, _content_type, _content_disposition, bytes) =
        send_csv(&app, "/api/ledger/export").await;
    assert_eq!(status, StatusCode::OK);
    let csv = String::from_utf8(bytes).unwrap();
    let lines: Vec<&str> = csv.lines().collect();
    assert!(lines
        .iter()
        .find(|line| line.contains("Legacy formula date"))
        .unwrap()
        .starts_with(",Expense,"));
    assert!(lines
        .iter()
        .find(|line| line.contains("Legacy impossible date"))
        .unwrap()
        .starts_with(",Expense,"));
    assert!(lines
        .iter()
        .find(|line| line.contains("Legacy invalid transfer"))
        .unwrap()
        .starts_with(",Transfer,"));
    assert!(lines
        .iter()
        .find(|line| line.contains("Valid timestamp"))
        .unwrap()
        .starts_with("2026-11-21,Expense,"));
    assert!(lines
        .iter()
        .find(|line| line.contains("Valid date only"))
        .unwrap()
        .starts_with("2026-11-22,Expense,"));
    assert!(!csv.contains("=2026-11-20"));
    assert!(!csv.contains("2026-02-30"));
    assert!(!csv.contains("not-a-date"));
}

#[tokio::test]
async fn monthly_category_budgets_roll_up_children_without_double_counting_overall_spend() {
    let (app, pool, _directory) = setup().await;
    let checking = create_account(&app, "Budget checking", 1_000_000).await;
    let savings = create_account(&app, "Budget savings", 100_000).await;
    let food = category_id(&app, "expense", "Food & Drink").await;
    let groceries = category_id(&app, "expense", "Groceries").await;
    let dining = category_id(&app, "expense", "Dining Out").await;
    // This isolated fixture uses the requested Food & Drink → Dining Out example.
    sqlx::query("UPDATE categories SET parent_id = ? WHERE id = ?")
        .bind(food)
        .bind(dining)
        .execute(&pool)
        .await
        .unwrap();

    let (status, body) = set_budget(&app, food, "2026-11", 500_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = set_budget(&app, dining, "2026-11", 150_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = set_budget(&app, dining, "2026-12", 200_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    create_entry_at(
        &app,
        "expense",
        checking,
        100_000,
        "Budget groceries",
        Some(groceries),
        "2026-11-05T12:00:00",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        checking,
        180_000,
        "Budget dining",
        Some(dining),
        "2026-11-06T12:00:00",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        checking,
        30_000,
        "Budget uncategorized",
        None,
        "2026-11-07T12:00:00",
    )
    .await;
    create_entry_at(
        &app,
        "income",
        checking,
        500_000,
        "Budget income does not spend",
        None,
        "2026-11-08T12:00:00",
    )
    .await;
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/transfers",
        json!({
            "from_account_id": checking,
            "to_account_id": savings,
            "amount_minor": 100_000,
            "name": "Budget transfer does not spend",
            "occurred_at": "2026-11-09T12:00:00"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let november = monthly_budgets(&app, "2026-11").await;
    assert_eq!(november["total_spent_minor"], 310_000);
    assert_eq!(november["uncategorized_spent_minor"], 30_000);
    assert_eq!(november["expense_count"], 3);
    assert_eq!(budget_row(&november, food)["limit_minor"], 500_000);
    assert_eq!(budget_row(&november, food)["direct_spent_minor"], 0);
    assert_eq!(budget_row(&november, food)["spent_minor"], 280_000);
    assert_eq!(budget_row(&november, food)["remaining_minor"], 220_000);
    assert_eq!(budget_row(&november, groceries)["spent_minor"], 100_000);
    assert_eq!(budget_row(&november, dining)["limit_minor"], 150_000);
    assert_eq!(budget_row(&november, dining)["spent_minor"], 180_000);
    assert_eq!(budget_row(&november, dining)["remaining_minor"], -30_000);

    let december = monthly_budgets(&app, "2026-12").await;
    assert_eq!(december["total_spent_minor"], 0);
    assert_eq!(budget_row(&december, dining)["limit_minor"], 200_000);
    assert_eq!(budget_row(&november, dining)["limit_minor"], 150_000);

    create_entry_at(
        &app,
        "expense",
        checking,
        20_000,
        "Direct parent budget expense",
        Some(food),
        "2026-12-15T12:00:00",
    )
    .await;
    let december = monthly_budgets(&app, "2026-12").await;
    assert_eq!(december["total_spent_minor"], 20_000);
    assert_eq!(budget_row(&december, food)["direct_spent_minor"], 20_000);
    assert_eq!(budget_row(&december, food)["spent_minor"], 20_000);
    assert_eq!(budget_row(&december, dining)["spent_minor"], 0);

    // A fresh read after unrelated state reload keeps the persisted monthly limits.
    state(&app).await;
    let reloaded = monthly_budgets(&app, "2026-11").await;
    assert_eq!(budget_row(&reloaded, food)["limit_minor"], 500_000);
    assert_eq!(budget_row(&reloaded, dining)["limit_minor"], 150_000);

    let (status, body) = set_budget(&app, food, "2026-11", 400_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(budget_row(&body, food)["remaining_minor"], 120_000);
    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/category-budgets/{dining}?month=2026-11"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(budget_row(&body, dining)["limit_minor"], Value::Null);
    assert_eq!(budget_row(&body, dining)["spent_minor"], 180_000);
}

#[tokio::test]
async fn category_budgets_use_calendar_boundaries_and_recalculate_after_entry_changes() {
    let (app, _pool, _directory) = setup().await;
    let checking = create_account(&app, "Boundary checking", 1_000_000).await;
    let shopping = category_id(&app, "expense", "Shopping").await;
    let (status, body) = set_budget(&app, shopping, "2026-11", 100_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let october = create_entry_at(
        &app,
        "expense",
        checking,
        10_000,
        "October boundary",
        Some(shopping),
        "2026-10-31T23:59:59",
    )
    .await;
    let november_first = create_entry_at(
        &app,
        "expense",
        checking,
        20_000,
        "November first",
        Some(shopping),
        "2026-11-01T00:00:00",
    )
    .await;
    let november_last = create_entry_at(
        &app,
        "expense",
        checking,
        30_000,
        "November last",
        Some(shopping),
        "2026-11-30T23:59:59",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        checking,
        40_000,
        "December boundary",
        Some(shopping),
        "2026-12-01T00:00:00",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        checking,
        5_000,
        "Leap day",
        Some(shopping),
        "2024-02-29T23:59:59",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        checking,
        6_000,
        "After leap day",
        Some(shopping),
        "2024-03-01T00:00:00",
    )
    .await;

    assert_eq!(
        monthly_budgets(&app, "2026-10").await["total_spent_minor"],
        10_000
    );
    let november = monthly_budgets(&app, "2026-11").await;
    assert_eq!(november["total_spent_minor"], 50_000);
    assert_eq!(budget_row(&november, shopping)["spent_minor"], 50_000);
    assert_eq!(
        monthly_budgets(&app, "2026-12").await["total_spent_minor"],
        40_000
    );
    assert_eq!(
        monthly_budgets(&app, "2024-02").await["total_spent_minor"],
        5_000
    );
    assert_eq!(
        monthly_budgets(&app, "2024-03").await["total_spent_minor"],
        6_000
    );

    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/transactions/{november_first}"),
        json!({
            "kind": "expense",
            "account_id": checking,
            "amount_minor": 25_000,
            "name": "November first edited",
            "category_id": shopping,
            "occurred_at": "2026-11-01T00:00:00"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        monthly_budgets(&app, "2026-11").await["total_spent_minor"],
        55_000
    );

    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/transactions/{november_last}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        monthly_budgets(&app, "2026-11").await["total_spent_minor"],
        25_000
    );
    assert_eq!(
        monthly_budgets(&app, "2026-10").await["total_spent_minor"],
        10_000
    );

    // Keep the fixture IDs referenced so accidental boundary omissions are explicit.
    assert_ne!(october, november_first);
}

#[tokio::test]
async fn category_budgets_validate_inputs_and_keep_archived_history_visible() {
    let (app, pool, _directory) = setup().await;
    let checking = create_account(&app, "Archive budget checking", 100_000).await;
    let shopping = category_id(&app, "expense", "Shopping").await;
    let salary = category_id(&app, "income", "Salary").await;

    for month in ["2026-13", "2026-00", "2026-1", "=2026-11", "+2026-11"] {
        let (status, body) = send_json(
            &app,
            "GET",
            &format!("/api/category-budgets?month={month}"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        let (status, body) = set_budget(&app, shopping, month, 100_000).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
    for amount in [0, -1] {
        let (status, body) = set_budget(&app, shopping, "2026-11", amount).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
    let (status, body) = set_budget(&app, salary, "2026-11", 100_000).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        body["error"],
        "budgets can only be set for expense categories"
    );
    let (status, body) = set_budget(&app, 999_999, "2026-11", 100_000).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let budget_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM category_budgets")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(budget_count, 0);

    let (status, body) = set_budget(&app, shopping, "2026-11", 100_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    create_entry_at(
        &app,
        "expense",
        checking,
        12_345,
        "Archived budget expense",
        Some(shopping),
        "2026-11-20T12:00:00",
    )
    .await;
    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/categories/{shopping}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["archived"], true);

    let november = monthly_budgets(&app, "2026-11").await;
    assert!(budget_row(&november, shopping)["archived_at"].is_string());
    assert_eq!(budget_row(&november, shopping)["limit_minor"], 100_000);
    assert_eq!(budget_row(&november, shopping)["spent_minor"], 12_345);
    let (status, body) = set_budget(&app, shopping, "2026-12", 100_000).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, body) = set_budget(&app, shopping, "2026-11", 150_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(budget_row(&body, shopping)["limit_minor"], 150_000);
    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/category-budgets/{shopping}?month=2026-11"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(budget_row(&body, shopping)["limit_minor"], Value::Null);

    let (status, body) = send_json(
        &app,
        "POST",
        "/api/categories",
        json!({
            "name": "Temporary budget category",
            "kind": "expense",
            "parent_id": null,
            "color": "#426b8a",
            "icon": "•",
            "sort_order": 999
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let temporary = body["id"].as_i64().unwrap();
    let (status, body) = set_budget(&app, temporary, "2026-11", 100_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/categories/{temporary}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["archived"], false);
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM category_budgets WHERE category_id = ?")
            .bind(temporary)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(remaining, 0);
}
