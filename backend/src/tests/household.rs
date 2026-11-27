use super::*;
use chrono::Utc;

/// Legacy accounts predate membership and are unowned until assignment.
async fn legacy_account(pool: &SqlitePool, name: &str, opening_balance_minor: i64) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "INSERT INTO accounts
            (name, currency, opening_balance_minor, created_at, account_type, note,
             opening_date, hide_from_selection, hide_from_reports, owner_member_id)
         VALUES (?, 'INR', ?, ?, 'unclassified', '', NULL, 0, 0, NULL)
         RETURNING id",
    )
    .bind(name)
    .bind(opening_balance_minor)
    .bind(Utc::now().to_rfc3339())
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn assignment_requires_designated_member_and_is_atomic() {
    let (app, pool, _directory) = setup_household().await;

    let (status, _) = send_json_as(
        &app,
        2,
        "POST",
        "/api/household/assignments/confirm",
        json!({ "assignments": [] }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "ordinary member cannot assign"
    );

    let account_id = legacy_account(&pool, "Checking", 500000).await;
    let cash_id = legacy_account(&pool, "Cash", 10000).await;

    let (status, body) = send_json_as(
        &app,
        1,
        "POST",
        "/api/household/assignments/propose",
        json!({
            "proposals": [
                { "account_id": account_id, "owner_member_id": 1 },
                { "account_id": cash_id, "owner_member_id": 2 }
            ]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let proposed = body["accounts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["account_id"] == account_id)
        .unwrap()
        .clone();
    assert_eq!(proposed["proposed_owner_member_id"], 1);
    assert!(proposed["confirmed_owner_member_id"].is_null());

    let (status, body) = send_json_as(
        &app,
        1,
        "POST",
        "/api/household/assignments/confirm",
        json!({
            "assignments": [
                { "account_id": account_id, "owner_member_id": 1 },
                { "account_id": cash_id, "owner_member_id": 2 }
            ]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // A confirmed owner is fixed: a later confirmation fails atomically.
    let (status, _) = send_json_as(
        &app,
        1,
        "POST",
        "/api/household/assignments/confirm",
        json!({
            "assignments": [
                { "account_id": account_id, "owner_member_id": 2 },
                { "account_id": cash_id, "owner_member_id": 1 }
            ]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, mine) = get_as(&app, 1, "/api/household/accounts").await;
    assert_eq!(status, StatusCode::OK, "{mine}");
    assert_eq!(mine["accounts"].as_array().unwrap().len(), 1);
    assert_eq!(mine["read_only"], false);

    let (status, other) = get_as(&app, 1, "/api/household/accounts?member_id=2").await;
    assert_eq!(status, StatusCode::OK, "{other}");
    assert_eq!(other["accounts"].as_array().unwrap().len(), 1);
    assert_eq!(other["read_only"], true);
    assert_eq!(other["accounts"][0]["can_write"], false);
}

#[tokio::test]
async fn writes_are_owner_scoped_and_reads_are_household_scoped() {
    let (app, pool, _directory) = setup_household().await;
    let account_id = legacy_account(&pool, "Checking", 500000).await;

    // Unassigned accounts are read-only in household mode.
    let (status, _) = send_json_as(
        &app,
        1,
        "POST",
        "/api/transactions",
        json!({
            "kind": "expense",
            "account_id": account_id,
            "amount_minor": 100,
            "name": "Blocked"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, _) = send_json_as(
        &app,
        1,
        "POST",
        "/api/household/assignments/confirm",
        json!({
            "assignments": [{ "account_id": account_id, "owner_member_id": 1 }]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send_json_as(
        &app,
        1,
        "POST",
        "/api/transactions",
        json!({
            "kind": "expense",
            "account_id": account_id,
            "amount_minor": 100,
            "name": "Lunch"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let entry_id = body["id"].as_i64().unwrap();

    // The other member can read saved activity but cannot write.
    let (status, ledger) = get_as(&app, 2, "/api/ledger").await;
    assert_eq!(status, StatusCode::OK, "{ledger}");
    assert_eq!(ledger["total_count"], 1);
    let (status, _) = send_json_as(
        &app,
        2,
        "PUT",
        &format!("/api/transactions/{entry_id}"),
        json!({
            "kind": "expense",
            "account_id": account_id,
            "amount_minor": 200,
            "name": "Changed"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = send_json_as(
        &app,
        2,
        "DELETE",
        &format!("/api/transactions/{entry_id}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Household mode rejects requests without a verified identity.
    let request = Request::builder()
        .method("GET")
        .uri("/api/state")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
