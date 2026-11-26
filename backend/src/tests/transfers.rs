use super::*;

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
