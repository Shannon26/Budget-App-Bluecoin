use super::*;
use sqlx::migrate::Migrator;
use std::fs;

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

async fn setup_populated_database_before_migration(
    last_migration: usize,
) -> (Router, SqlitePool, TempDir) {
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

    let migrations_directory = directory.path().join("migrations");
    fs::create_dir(&migrations_directory).unwrap();
    for (name, sql) in [
        (
            "001_init.sql",
            include_str!("../../migrations/001_init.sql"),
        ),
        (
            "002_categories_and_ledger.sql",
            include_str!("../../migrations/002_categories_and_ledger.sql"),
        ),
        (
            "003_fix_dining_category.sql",
            include_str!("../../migrations/003_fix_dining_category.sql"),
        ),
        (
            "004_category_budgets.sql",
            include_str!("../../migrations/004_category_budgets.sql"),
        ),
        (
            "005_account_types.sql",
            include_str!("../../migrations/005_account_types.sql"),
        ),
    ]
    .into_iter()
    .take(last_migration)
    {
        fs::write(migrations_directory.join(name), sql).unwrap();
    }
    let pre_account_type_migrator = Migrator::new(migrations_directory.clone()).await.unwrap();
    pre_account_type_migrator.run(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO accounts (id, name, currency, opening_balance_minor, created_at)
         VALUES (41, 'Legacy Checking', 'INR', 100000, '2026-01-01T00:00:00Z'),
                (42, 'Legacy Savings', 'INR', 50000, '2026-01-02T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO transfers (id, from_account_id, to_account_id, amount_minor, note, occurred_at, created_at)
         VALUES (7, 41, 42, 10000, 'legacy transfer', '2026-02-01T12:00:00Z', '2026-02-01T12:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO entries (id, kind, account_id, amount_minor, note, occurred_at, created_at, transfer_id)
         VALUES (101, 'expense', 41, 2500, 'legacy expense', '2026-02-02T12:00:00Z', '2026-02-02T12:00:00Z', NULL),
                (102, 'income', 42, 5000, 'legacy income', '2026-02-03T12:00:00Z', '2026-02-03T12:00:00Z', NULL),
                (103, 'transfer_out', 41, 10000, 'legacy transfer', '2026-02-01T12:00:00Z', '2026-02-01T12:00:00Z', 7),
                (104, 'transfer_in', 42, 10000, 'legacy transfer', '2026-02-01T12:00:00Z', '2026-02-01T12:00:00Z', 7)",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (build_router(pool.clone()), pool, directory)
}

async fn setup_populated_pre_account_type_database() -> (Router, SqlitePool, TempDir) {
    setup_populated_database_before_migration(4).await
}

async fn setup_populated_pre_account_metadata_database() -> (Router, SqlitePool, TempDir) {
    setup_populated_database_before_migration(5).await
}

#[tokio::test]
async fn populated_pre_migration_accounts_become_unclassified_without_data_changes() {
    let (app, pool, _directory) = setup_populated_pre_account_type_database().await;
    let current = state(&app).await;

    assert_eq!(current["accounts"].as_array().unwrap().len(), 2);
    let checking = &current["accounts"][0];
    let savings = &current["accounts"][1];
    assert_eq!(checking["id"], 41);
    assert_eq!(checking["name"], "Legacy Checking");
    assert_eq!(checking["opening_balance_minor"], 100_000);
    assert_eq!(checking["balance_minor"], 87_500);
    assert_eq!(checking["account_type"], "unclassified");
    assert_eq!(checking["account_group"], "unclassified");
    assert_eq!(savings["id"], 42);
    assert_eq!(savings["name"], "Legacy Savings");
    assert_eq!(savings["opening_balance_minor"], 50_000);
    assert_eq!(savings["balance_minor"], 65_000);
    assert_eq!(savings["account_type"], "unclassified");
    assert_eq!(savings["account_group"], "unclassified");
    assert_eq!(current["totals"]["income_minor"], 5_000);
    assert_eq!(current["totals"]["expense_minor"], 2_500);

    let entry_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entries")
        .fetch_one(&pool)
        .await
        .unwrap();
    let transfer_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transfers")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(entry_count, 4);
    assert_eq!(transfer_count, 1);

    let (status, history) = send_json(
        &app,
        "GET",
        "/api/accounts/41/history?limit=200&offset=0",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{history}");
    assert_eq!(history["total_count"], 2);
    assert_eq!(history["entries"][0]["kind"], "expense");
    assert_eq!(history["entries"][1]["kind"], "transfer_out");

    let (status, _, _, csv) = send_csv(&app, "/api/ledger/export").await;
    assert_eq!(status, StatusCode::OK);
    let csv = String::from_utf8(csv).unwrap();
    assert_eq!(
        csv.lines().next().unwrap(),
        "Date,Type,Name,Account,Source Account,Destination Account,Category,Subcategory,Amount,Currency,Status,Note"
    );
    assert_eq!(csv.lines().count(), 4);
}

#[tokio::test]
async fn account_type_choices_round_trip_and_invalid_values_are_rejected() {
    let (app, _pool, _directory) = setup().await;
    let (status, cash) = send_json(
        &app,
        "POST",
        "/api/accounts",
        json!({
            "name": "Cash box",
            "currency": "INR",
            "opening_balance_minor": 10000,
            "account_type": "cash",
            "account_group": "liability"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{cash}");
    assert_eq!(cash["account_type"], "cash");
    assert_eq!(cash["account_group"], "asset");
    let cash_id = cash["id"].as_i64().unwrap();

    let (status, mortgage) = send_json(
        &app,
        "POST",
        "/api/accounts",
        json!({
            "name": "Home loan",
            "currency": "INR",
            "opening_balance_minor": -500000,
            "account_type": "mortgage"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{mortgage}");
    assert_eq!(mortgage["account_type"], "mortgage");
    assert_eq!(mortgage["account_group"], "liability");

    let (status, legacy) = send_json(
        &app,
        "POST",
        "/api/accounts",
        json!({
            "name": "Old client account",
            "currency": "INR",
            "opening_balance_minor": 0
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{legacy}");
    assert_eq!(legacy["account_type"], "unclassified");
    assert_eq!(legacy["account_group"], "unclassified");

    let current = state(&app).await;
    let definitions = current["account_types"].as_array().unwrap();
    let values: HashSet<&str> = definitions
        .iter()
        .map(|definition| definition["value"].as_str().unwrap())
        .collect();
    assert_eq!(values.len(), definitions.len());
    assert!(values.contains("cash"));
    assert!(values.contains("credit_card"));
    assert!(values.contains("mortgage"));
    assert!(values.contains("unclassified"));
    for definition in definitions {
        let value = definition["value"].as_str().unwrap();
        let expected_group = account_group_for(value);
        assert_eq!(definition["group"], expected_group);
    }

    let (status, updated) = send_json(
        &app,
        "PUT",
        &format!("/api/accounts/{cash_id}"),
        json!({ "account_type": "credit_card", "account_group": "asset" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["id"], cash_id);
    assert_eq!(updated["name"], "Cash box");
    assert_eq!(updated["opening_balance_minor"], 10_000);
    assert_eq!(updated["account_type"], "credit_card");
    assert_eq!(updated["account_group"], "liability");

    let (status, updated) = send_json(
        &app,
        "PUT",
        &format!("/api/accounts/{cash_id}"),
        json!({ "account_type": "cash" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["account_type"], "cash");
    assert_eq!(updated["account_group"], "asset");

    let (status, body) = send_json(
        &app,
        "POST",
        "/api/accounts",
        json!({
            "name": "Invalid type",
            "currency": "INR",
            "opening_balance_minor": 0,
            "account_type": "bank"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"], "account type is not supported");

    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/accounts/{cash_id}"),
        json!({ "account_type": "Cash" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"], "account type is not supported");
    let unchanged = state(&app).await;
    assert_eq!(unchanged["accounts"][0]["account_type"], "cash");
    assert_eq!(unchanged["accounts"][0]["account_group"], "asset");

    let (status, body) = send_json(
        &app,
        "PUT",
        "/api/accounts/999",
        json!({ "account_type": "cash" }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

#[tokio::test]
async fn account_type_edits_preserve_balances_history_entries_and_csv() {
    let (app, pool, _directory) = setup().await;
    let (status, checking) = send_json(
        &app,
        "POST",
        "/api/accounts",
        json!({
            "name": "Type edit checking",
            "currency": "INR",
            "opening_balance_minor": 100000,
            "account_type": "checking"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{checking}");
    let checking_id = checking["id"].as_i64().unwrap();
    let savings_id = create_account(&app, "Type edit savings", 50_000).await;
    create_entry(
        &app,
        "expense",
        checking_id,
        2_500,
        "Type edit expense",
        None,
    )
    .await;
    create_entry(&app, "income", savings_id, 5_000, "Type edit income", None).await;
    let (status, body) = transfer(&app, checking_id, savings_id, 10_000).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let before_state = state(&app).await;
    let (status, before_history) = send_json(
        &app,
        "GET",
        &format!("/api/accounts/{checking_id}/history?limit=200&offset=0"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _, ledger_csv) = send_csv(&app, "/api/ledger/export").await;
    assert_eq!(status, StatusCode::OK);

    let (status, updated) = send_json(
        &app,
        "PUT",
        &format!("/api/accounts/{checking_id}"),
        json!({ "account_type": "credit_card" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["account_group"], "liability");
    let (status, updated) = send_json(
        &app,
        "PUT",
        &format!("/api/accounts/{savings_id}"),
        json!({ "account_type": "loan" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["account_group"], "liability");

    let after_state = state(&app).await;
    assert_eq!(after_state["totals"], before_state["totals"]);
    assert_eq!(after_state["entries"], before_state["entries"]);
    for (before, after) in before_state["accounts"]
        .as_array()
        .unwrap()
        .iter()
        .zip(after_state["accounts"].as_array().unwrap())
    {
        assert_eq!(before["id"], after["id"]);
        assert_eq!(before["name"], after["name"]);
        assert_eq!(
            before["opening_balance_minor"],
            after["opening_balance_minor"]
        );
        assert_eq!(before["balance_minor"], after["balance_minor"]);
    }

    let (status, after_history) = send_json(
        &app,
        "GET",
        &format!("/api/accounts/{checking_id}/history?limit=200&offset=0"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(before_history, after_history);
    let (status, _, _, after_ledger_csv) = send_csv(&app, "/api/ledger/export").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ledger_csv, after_ledger_csv);

    let entry_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entries")
        .fetch_one(&pool)
        .await
        .unwrap();
    let transfer_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transfers")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(entry_count, 4);
    assert_eq!(transfer_count, 1);
}

#[tokio::test]
async fn populated_pre_metadata_migration_preserves_data_and_adds_legacy_defaults() {
    let (app, _pool, _directory) = setup_populated_pre_account_metadata_database().await;
    let current = state(&app).await;
    let checking = &current["accounts"][0];
    let savings = &current["accounts"][1];

    assert_eq!(checking["id"], 41);
    assert_eq!(checking["name"], "Legacy Checking");
    assert_eq!(checking["balance_minor"], 87_500);
    assert_eq!(checking["note"], "");
    assert_eq!(checking["opening_date"], Value::Null);
    assert_eq!(checking["hide_from_selection"], false);
    assert_eq!(checking["hide_from_reports"], false);
    assert_eq!(savings["id"], 42);
    assert_eq!(savings["balance_minor"], 65_000);
    assert_eq!(savings["note"], "");
    assert_eq!(savings["opening_date"], Value::Null);
    assert_eq!(savings["hide_from_selection"], false);
    assert_eq!(savings["hide_from_reports"], false);

    let (status, history) = send_json(
        &app,
        "GET",
        "/api/accounts/41/history?limit=200&offset=0",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{history}");
    assert_eq!(history["total_count"], 2);
    assert_eq!(history["entries"][0]["id"], 101);
    assert_eq!(history["entries"][1]["kind"], "transfer_out");
    assert_eq!(history["entries"][1]["transfer_id"], 7);

    let (status, _, _, csv) = send_csv(&app, "/api/ledger/export").await;
    assert_eq!(status, StatusCode::OK);
    let csv = String::from_utf8(csv).unwrap();
    assert_eq!(csv.lines().count(), 4);
    assert!(csv.contains("Legacy Checking"));
    assert!(csv.contains("Legacy Savings"));
}

#[tokio::test]
async fn account_metadata_round_trips_partial_updates_and_explicit_clearing() {
    let (app, _pool, _directory) = setup().await;
    let (status, created) = send_json(
        &app,
        "POST",
        "/api/accounts",
        json!({
            "name": "Metadata wallet",
            "currency": "INR",
            "opening_balance_minor": -12345,
            "account_type": "wallet",
            "note": "  Travel wallet  ",
            "opening_date": "2024-02-29",
            "hide_from_selection": true,
            "hide_from_reports": true
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_i64().unwrap();
    assert_eq!(created["note"], "Travel wallet");
    assert_eq!(created["opening_date"], "2024-02-29");
    assert_eq!(created["hide_from_selection"], true);
    assert_eq!(created["hide_from_reports"], true);
    assert_eq!(created["opening_balance_minor"], -12_345);
    assert_eq!(created["account_group"], "asset");
    assert!(created.get("balance_minor").is_none());

    let (status, updated) = send_json(
        &app,
        "PUT",
        &format!("/api/accounts/{id}"),
        json!({ "note": "Updated note" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["name"], "Metadata wallet");
    assert_eq!(updated["note"], "Updated note");
    assert_eq!(updated["opening_date"], "2024-02-29");
    assert_eq!(updated["account_type"], "wallet");
    assert_eq!(updated["hide_from_selection"], true);
    assert_eq!(updated["hide_from_reports"], true);
    assert_eq!(updated["opening_balance_minor"], -12_345);

    let (status, updated) = send_json(
        &app,
        "PUT",
        &format!("/api/accounts/{id}"),
        json!({ "name": "Renamed wallet" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["note"], "Updated note");
    assert_eq!(updated["opening_date"], "2024-02-29");
    assert_eq!(updated["hide_from_selection"], true);
    assert_eq!(updated["hide_from_reports"], true);

    // Old F01 clients still send only account_type; all F02 metadata must survive.
    let (status, updated) = send_json(
        &app,
        "PUT",
        &format!("/api/accounts/{id}"),
        json!({ "account_type": "cash" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["name"], "Renamed wallet");
    assert_eq!(updated["note"], "Updated note");
    assert_eq!(updated["opening_date"], "2024-02-29");
    assert_eq!(updated["account_type"], "cash");
    assert_eq!(updated["hide_from_selection"], true);
    assert_eq!(updated["hide_from_reports"], true);

    let (status, updated) = send_json(
        &app,
        "PUT",
        &format!("/api/accounts/{id}"),
        json!({
            "note": null,
            "opening_date": null,
            "hide_from_selection": false,
            "hide_from_reports": false
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["name"], "Renamed wallet");
    assert_eq!(updated["note"], "");
    assert_eq!(updated["opening_date"], Value::Null);
    assert_eq!(updated["hide_from_selection"], false);
    assert_eq!(updated["hide_from_reports"], false);
    assert_eq!(updated["account_type"], "cash");
    assert_eq!(updated["opening_balance_minor"], -12_345);

    let (status, updated) = send_json(
        &app,
        "PUT",
        &format!("/api/accounts/{id}"),
        json!({ "note": "Temporary", "opening_date": "2026-11-27" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    let (status, updated) = send_json(
        &app,
        "PUT",
        &format!("/api/accounts/{id}"),
        json!({ "note": "", "opening_date": "" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["note"], "");
    assert_eq!(updated["opening_date"], Value::Null);

    let (status, legacy) = send_json(
        &app,
        "POST",
        "/api/accounts",
        json!({
            "name": "Old metadata client",
            "currency": "INR",
            "opening_balance_minor": 0
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{legacy}");
    assert_eq!(legacy["note"], "");
    assert_eq!(legacy["opening_date"], Value::Null);
    assert_eq!(legacy["hide_from_selection"], false);
    assert_eq!(legacy["hide_from_reports"], false);
}

#[tokio::test]
async fn account_metadata_rejects_invalid_dates_without_writes() {
    let (app, pool, _directory) = setup().await;
    for opening_date in [
        "2023-02-29",
        "1900-02-29",
        "2026-02-30",
        "2026-13-01",
        "2026-1-01",
        "2026-01-1",
        "2026-01-01T12:00:00",
        " 2026-01-01",
        "2026-01-01 ",
        "   ",
        "=2026-01-01",
    ] {
        let (status, body) = send_json(
            &app,
            "POST",
            "/api/accounts",
            json!({
                "name": "Invalid opening date",
                "currency": "INR",
                "opening_balance_minor": 0,
                "opening_date": opening_date
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{opening_date}: {body}");
        assert_eq!(
            body["error"],
            "opening date must be a valid YYYY-MM-DD calendar date"
        );
    }
    let account_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(account_count, 0);

    let (status, created) = send_json(
        &app,
        "POST",
        "/api/accounts",
        json!({
            "name": "Leap date account",
            "currency": "INR",
            "opening_balance_minor": 100,
            "opening_date": "2024-02-29"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_i64().unwrap();

    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/accounts/{id}"),
        json!({ "opening_date": "2025-02-29", "note": "must not persist" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let current = state(&app).await;
    assert_eq!(current["accounts"][0]["opening_date"], "2024-02-29");
    assert_eq!(current["accounts"][0]["note"], "");
}

#[tokio::test]
async fn metadata_edits_and_hide_flags_preserve_balances_history_and_posted_dates() {
    let (app, pool, _directory) = setup().await;
    let (status, created) = send_json(
        &app,
        "POST",
        "/api/accounts",
        json!({
            "name": "Before rename",
            "currency": "INR",
            "opening_balance_minor": 100000,
            "account_type": "checking",
            "note": "Original note",
            "opening_date": "2020-01-01"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_i64().unwrap();
    let other_id = create_account(&app, "Metadata transfer target", 50_000).await;
    let entry_id = create_entry_at(
        &app,
        "expense",
        id,
        2_500,
        "Metadata history expense",
        None,
        "2026-02-02T12:00:00Z",
    )
    .await;
    let (status, body) = transfer(&app, id, other_id, 10_000).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let before_state = state(&app).await;
    let (status, before_history) = send_json(
        &app,
        "GET",
        &format!("/api/accounts/{id}/history?limit=200&offset=0"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, updated) = send_json(
        &app,
        "PUT",
        &format!("/api/accounts/{id}"),
        json!({
            "name": "After rename",
            "note": "Updated note",
            "opening_date": "2021-03-04",
            "hide_from_selection": true,
            "hide_from_reports": true
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["id"], id);
    assert_eq!(updated["name"], "After rename");
    assert_eq!(updated["opening_balance_minor"], 100_000);
    assert!(updated.get("balance_minor").is_none());

    let after_state = state(&app).await;
    assert_eq!(after_state["totals"], before_state["totals"]);
    for (before, after) in before_state["entries"]
        .as_array()
        .unwrap()
        .iter()
        .zip(after_state["entries"].as_array().unwrap())
    {
        assert_eq!(before["id"], after["id"]);
        assert_eq!(before["kind"], after["kind"]);
        assert_eq!(before["amount_minor"], after["amount_minor"]);
        assert_eq!(before["occurred_at"], after["occurred_at"]);
        assert_eq!(before["category_id"], after["category_id"]);
        assert_eq!(before["reconciled"], after["reconciled"]);
    }
    assert_eq!(after_state["accounts"][0]["balance_minor"], 87_500);
    assert_eq!(after_state["accounts"][0]["hide_from_selection"], true);
    assert_eq!(after_state["accounts"][0]["hide_from_reports"], true);

    let (status, after_history) = send_json(
        &app,
        "GET",
        &format!("/api/accounts/{id}/history?limit=200&offset=0"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(after_history["total_count"], before_history["total_count"]);
    let before_ids: Vec<i64> = before_history["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["id"].as_i64().unwrap())
        .collect();
    let after_ids: Vec<i64> = after_history["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["id"].as_i64().unwrap())
        .collect();
    assert_eq!(before_ids, after_ids);
    assert!(after_ids.contains(&entry_id));
    for entry in after_history["entries"].as_array().unwrap() {
        assert_eq!(entry["account_id"], id);
        assert_eq!(entry["account_name"], "After rename");
    }

    let (status, filtered) = send_json(
        &app,
        "GET",
        &format!("/api/ledger?account_id={id}&limit=200&offset=0"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{filtered}");
    assert_eq!(filtered["total_count"], 2);

    let (status, _, _, csv) = send_csv(&app, "/api/ledger/export").await;
    assert_eq!(status, StatusCode::OK);
    let csv = String::from_utf8(csv).unwrap();
    assert_eq!(
        csv.lines().next().unwrap(),
        "Date,Type,Name,Account,Source Account,Destination Account,Category,Subcategory,Amount,Currency,Status,Note"
    );
    assert_eq!(csv.lines().count(), 3);
    assert!(csv.contains("After rename"));
    assert!(csv.contains("25.00"));
    assert!(csv.contains("2026-02-02"));

    let stored_entry_date: String =
        sqlx::query_scalar("SELECT occurred_at FROM entries WHERE id = ?")
            .bind(entry_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored_entry_date, "2026-02-02T12:00:00Z");
}
