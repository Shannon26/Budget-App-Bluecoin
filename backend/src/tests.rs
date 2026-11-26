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
    assert_eq!(body.as_array().unwrap().len(), 1);
    assert_eq!(body[0]["id"], reconciled);
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
    assert_eq!(body[0]["kind"], "transfer_out");
    let (status, body) = send_json(
        &app,
        "GET",
        &format!("/api/accounts/{savings}/history"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body[0]["kind"], "transfer_in");

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
    let rows = body.as_array().unwrap();
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
