use super::*;

async fn draft(app: &Router) -> Value {
    let (status, body) = send_json(app, "GET", "/api/draft", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

async fn add_row(app: &Router, row: Value) -> Value {
    let (status, body) = send_json(app, "POST", "/api/draft/rows", row).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

fn expense_row(row_id: &str, name: &str, amount: i64, account: Value) -> Value {
    let mut row = json!({
        "row_id": row_id,
        "kind": "expense",
        "name": name,
        "amount_minor": amount,
        "date": "2026-11-27",
        "manual": true
    });
    match account {
        Value::Number(id) => {
            row["account_id"] = Value::Number(id);
        }
        Value::String(name) => {
            row["account_name"] = Value::String(name);
        }
        _ => panic!("account must be an id or a name"),
    }
    row
}

#[tokio::test]
async fn manual_batch_creates_account_and_entries_with_a_repeatable_receipt() {
    let (app, _pool, _directory) = setup().await;
    let checking = create_account(&app, "Checking", 100000).await;
    let initial = draft(&app).await;
    let revision = initial["revision"].as_i64().unwrap();

    add_row(
        &app,
        json!({
            "row_id": "new-1",
            "kind": "new_account",
            "name": "HDFC",
            "opening_balance_minor": 500000,
            "manual": true
        }),
    )
    .await;
    add_row(
        &app,
        expense_row("row-1", "Lunch", 24000, Value::String("HDFC".to_string())),
    )
    .await;
    let with_rows = add_row(
        &app,
        expense_row("row-2", "Coffee", 8000, Value::Number(checking.into())),
    )
    .await;
    assert_eq!(with_rows["rows"].as_array().unwrap().len(), 3);
    let reviewed_revision = with_rows["revision"].as_i64().unwrap();
    assert!(reviewed_revision > revision);

    let (status, receipt) = send_json(
        &app,
        "POST",
        "/api/draft/save",
        json!({ "revision": reviewed_revision }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(receipt["accounts_created"].as_array().unwrap().len(), 1);
    assert_eq!(receipt["entry_ids"].as_array().unwrap().len(), 2);
    assert!(receipt["pending_request_ids"]
        .as_array()
        .unwrap()
        .is_empty());

    // Retrying the same reviewed Save returns the same receipt and posts nothing.
    let (status, retry) = send_json(
        &app,
        "POST",
        "/api/draft/save",
        json!({ "revision": reviewed_revision }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{retry}");
    assert_eq!(retry, receipt);

    let current = state(&app).await;
    let hdfc = current["accounts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|account| account["name"] == "HDFC")
        .unwrap();
    assert_eq!(hdfc["balance_minor"], 476000);
    let checking = current["accounts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|account| account["name"] == "Checking")
        .unwrap();
    assert_eq!(checking["balance_minor"], 92000);

    let cleared = draft(&app).await;
    assert!(cleared["rows"].as_array().unwrap().is_empty());
    assert!(cleared["prompts"].as_array().unwrap().is_empty());
    assert!(cleared["revision"].as_i64().unwrap() > reviewed_revision);
}

#[tokio::test]
async fn unresolved_rows_and_stale_revisions_cannot_save() {
    let (app, _pool, _directory) = setup().await;
    let account = create_account(&app, "Cash", 10000).await;
    add_row(
        &app,
        expense_row("row-1", "Lunch", 100, Value::Number(account.into())),
    )
    .await;
    let current = draft(&app).await;
    let revision = current["revision"].as_i64().unwrap();
    add_row(&app, expense_row("row-2", "Unclear", 200, json!("Mystery"))).await;
    let next = draft(&app).await;

    let (status, body) = send_json(
        &app,
        "POST",
        "/api/draft/save",
        json!({ "revision": next["revision"] }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body["error"].as_str().unwrap().contains("unresolved"));

    // A stale revision never posts the newer rows.
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/draft/save",
        json!({ "revision": revision }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(state(&app).await["totals"]["expense_minor"], 0);
}

#[tokio::test]
async fn forced_mid_batch_failure_rolls_back_everything_and_keeps_the_draft() {
    let (app, pool, _directory) = setup().await;
    let account = create_account(&app, "Cash", 10000).await;
    add_row(
        &app,
        expense_row("row-1", "Lunch", 500, Value::Number(account.into())),
    )
    .await;
    let current = draft(&app).await;
    let revision = current["revision"].as_i64().unwrap();

    sqlx::query(
        "CREATE TRIGGER fail_lunch BEFORE INSERT ON entries
         WHEN NEW.name = 'Lunch'
         BEGIN SELECT RAISE(ABORT, 'forced failure'); END",
    )
    .execute(&pool)
    .await
    .unwrap();

    let (status, body) = send_json(
        &app,
        "POST",
        "/api/draft/save",
        json!({ "revision": revision }),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert_eq!(state(&app).await["totals"]["expense_minor"], 0);
    let kept = draft(&app).await;
    assert_eq!(kept["revision"], revision);
    assert_eq!(kept["rows"].as_array().unwrap().len(), 1);

    sqlx::query("DROP TRIGGER fail_lunch")
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/draft/save",
        json!({ "revision": revision }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(state(&app).await["totals"]["expense_minor"], 500);
}

#[tokio::test]
async fn prompts_and_discard_are_private_and_scoped_to_one_member() {
    let (app, _pool, _directory) = setup_household().await;
    let (status, body) = send_json_as(
        &app,
        1,
        "POST",
        "/api/accounts",
        json!({ "name": "Mine", "opening_balance_minor": 10000 }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    // New accounts are owned by their creator, so no assignment is needed.

    let (status, body) = send_json_as(
        &app,
        1,
        "POST",
        "/api/draft/prompt",
        json!({
            "text": "Yesterday I paid ₹240 for lunch",
            "client_prompt_id": "submission-1",
            "local_date": "2026-11-27"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let mine = get_as(&app, 1, "/api/draft").await.1;
    assert_eq!(mine["prompts"].as_array().unwrap().len(), 1);

    let (status, other) = get_as(&app, 2, "/api/draft").await;
    assert_eq!(status, StatusCode::OK, "{other}");
    assert!(other["prompts"].as_array().unwrap().is_empty());
    assert!(other["rows"].as_array().unwrap().is_empty());

    // Repeating a submission does not duplicate the transcript entry or rows.
    let (status, retried) = send_json_as(
        &app,
        1,
        "POST",
        "/api/draft/prompt",
        json!({
            "text": "Yesterday I paid ₹240 for lunch",
            "client_prompt_id": "submission-1",
            "local_date": "2026-11-27"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{retried}");
    let mine = get_as(&app, 1, "/api/draft").await.1;
    assert_eq!(mine["prompts"].as_array().unwrap().len(), 1);

    let (status, body) = send_json_as(&app, 1, "POST", "/api/draft/discard", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let mine = get_as(&app, 1, "/api/draft").await.1;
    assert!(mine["prompts"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn current_balance_is_reconciled_backwards_to_an_opening_balance() {
    let (app, _pool, _directory) = setup().await;
    add_row(
        &app,
        json!({
            "row_id": "new-1",
            "kind": "new_account",
            "name": "HDFC",
            "current_balance_minor": 500000,
            "manual": true
        }),
    )
    .await;
    add_row(
        &app,
        expense_row("row-1", "Lunch", 20000, Value::String("HDFC".to_string())),
    )
    .await;
    let current = draft(&app).await;
    let revision = current["revision"].as_i64().unwrap();
    let (status, receipt) = send_json(
        &app,
        "POST",
        "/api/draft/save",
        json!({ "revision": revision }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    let accounts = state(&app).await["accounts"].clone();
    let hdfc = accounts.as_array().unwrap()[0].clone();
    assert_eq!(hdfc["opening_balance_minor"], 520000);
    assert_eq!(hdfc["balance_minor"], 500000);
}
