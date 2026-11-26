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
