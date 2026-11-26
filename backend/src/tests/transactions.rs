use super::*;

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
