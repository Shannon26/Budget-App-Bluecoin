use super::*;

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
