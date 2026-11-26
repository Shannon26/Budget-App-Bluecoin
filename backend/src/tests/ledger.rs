use super::*;

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
