use super::*;
use crate::error::AppError;
use crate::models::*;
use crate::store::Store;
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::collections::HashSet;
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

async fn update_category_shape(
    app: &Router,
    category_id: i64,
    kind: Option<&str>,
    parent_id: Option<Option<i64>>,
) -> (StatusCode, Value) {
    let current = state(app).await;
    let category = current["categories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|category| category["id"] == category_id)
        .unwrap()
        .clone();
    let next_parent = parent_id.unwrap_or(category["parent_id"].as_i64());
    send_json(
        app,
        "PUT",
        &format!("/api/categories/{category_id}"),
        json!({
            "name": category["name"],
            "kind": kind.unwrap_or(category["kind"].as_str().unwrap()),
            "parent_id": next_parent,
            "color": category["color"],
            "icon": category["icon"],
            "sort_order": category["sort_order"]
        }),
    )
    .await
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

mod accounts;
mod budgets;
mod categories;
mod csv_export;
mod filters;
mod labels;
mod ledger;
mod reports;
mod transactions;
mod transfers;
