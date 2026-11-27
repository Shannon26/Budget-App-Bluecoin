use super::*;

fn query_value(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
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

#[tokio::test]
async fn account_history_applies_filters_and_always_pins_the_path_account() {
    let (app, _pool, _directory) = setup().await;
    let checking = create_account(&app, "History checking", 100_000).await;
    let savings = create_account(&app, "History savings", 20_000).await;
    let dining = category_id(&app, "expense", "Dining Out").await;
    let salary = category_id(&app, "income", "Salary").await;
    let target = create_entry_at(
        &app,
        "expense",
        checking,
        25_000,
        "Lunch",
        Some(dining),
        "2026-11-10T12:00:00",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        checking,
        5_000,
        "Snack",
        Some(dining),
        "2026-11-11T12:00:00",
    )
    .await;
    create_entry_at(
        &app,
        "income",
        checking,
        100_000,
        "Salary",
        Some(salary),
        "2026-11-12T12:00:00",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        savings,
        25_000,
        "Lunch",
        Some(dining),
        "2026-11-10T12:00:00",
    )
    .await;
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/transfers",
        json!({
            "from_account_id": checking,
            "to_account_id": savings,
            "amount_minor": 20_000,
            "name": "Card payment",
            "occurred_at": "2026-11-13T12:00:00"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let (status, filtered) = send_json(
        &app,
        "GET",
        &format!(
            "/api/accounts/{checking}/history?search=Lunch&date_from=2026-11-01&date_to=2026-11-30&account_id={checking}&category_id={dining}&reconciled=false&amount_min_minor=20000&amount_max_minor=30000"
        ),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{filtered}");
    assert_eq!(filtered["total_count"], 1);
    assert_eq!(filtered["entries"][0]["id"], target);
    assert_eq!(filtered["entries"][0]["account_id"], checking);

    let (status, transfer_page) = send_json(
        &app,
        "GET",
        &format!("/api/accounts/{checking}/history?search=Card%20payment&account_id={checking}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{transfer_page}");
    assert_eq!(transfer_page["total_count"], 1);
    assert_eq!(transfer_page["entries"][0]["kind"], "transfer_out");
    assert_eq!(transfer_page["entries"][0]["account_id"], checking);
    assert_eq!(
        transfer_page["entries"][0]["counterpart_account_id"],
        savings
    );

    let (status, omitted) = send_json(
        &app,
        "GET",
        &format!("/api/accounts/{checking}/history"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{omitted}");
    let (status, matching) = send_json(
        &app,
        "GET",
        &format!("/api/accounts/{checking}/history?account_id={checking}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{matching}");
    assert_eq!(omitted["total_count"], 4);
    assert_eq!(omitted["entries"], matching["entries"]);
    for entry in omitted["entries"].as_array().unwrap() {
        assert_eq!(entry["account_id"], checking);
    }

    let (status, body) = send_json(
        &app,
        "GET",
        &format!("/api/accounts/{checking}/history?account_id={savings}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        body["error"],
        "account_id filter must match the account path"
    );
}

#[tokio::test]
async fn account_history_filtered_pagination_keeps_full_day_totals_and_stable_order() {
    let (app, pool, _directory) = setup().await;
    let checking = create_account(&app, "History page checking", 100_000).await;
    let savings = create_account(&app, "History page savings", 20_000).await;
    sqlx::query(
        "WITH RECURSIVE numbers(value) AS (
             SELECT 1 UNION ALL SELECT value + 1 FROM numbers WHERE value < 201
         )
         INSERT INTO entries
             (kind, account_id, amount_minor, name, category_id, reconciled, note, occurred_at, created_at, transfer_id)
         SELECT 'expense', ?, 100, 'HistoryPage expense ' || value, NULL, 0, '',
                '2026-11-20T12:00:00', '2026-11-20T12:00:00Z', NULL
         FROM numbers",
    )
    .bind(checking)
    .execute(&pool)
    .await
    .unwrap();
    create_entry_at(
        &app,
        "expense",
        savings,
        999,
        "HistoryPage other account",
        None,
        "2026-11-20T12:00:00",
    )
    .await;
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/transfers",
        json!({
            "from_account_id": checking,
            "to_account_id": savings,
            "amount_minor": 500,
            "name": "HistoryPage transfer",
            "occurred_at": "2026-11-20T12:00:00"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let (status, first) = send_json(
        &app,
        "GET",
        &format!(
            "/api/accounts/{checking}/history?search=HistoryPage&account_id={checking}&limit=200&offset=0"
        ),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["total_count"], 202);
    assert_eq!(first["entries"].as_array().unwrap().len(), 200);
    assert_eq!(first["next_offset"], 200);
    assert_eq!(first["daily_totals"].as_array().unwrap().len(), 1);
    assert_eq!(first["daily_totals"][0]["date"], "2026-11-20");
    assert_eq!(first["daily_totals"][0]["expense_minor"], 20_100);
    assert_eq!(first["daily_totals"][0]["transfer_minor"], 500);

    let (status, second) = send_json(
        &app,
        "GET",
        &format!("/api/accounts/{checking}/history?search=HistoryPage&limit=200&offset=200"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(second["entries"].as_array().unwrap().len(), 2);
    assert_eq!(second["next_offset"], Value::Null);

    let mut ids = Vec::new();
    for page in [&first, &second] {
        for entry in page["entries"].as_array().unwrap() {
            assert_eq!(entry["account_id"], checking);
            ids.push(format!(
                "{}-{}",
                entry["kind"].as_str().unwrap(),
                entry["id"]
            ));
        }
    }
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 202);

    let (status, unfiltered) = send_json(
        &app,
        "GET",
        &format!("/api/accounts/{checking}/history"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{unfiltered}");
    assert_eq!(unfiltered["total_count"], 202);
    assert_eq!(unfiltered["daily_totals"][0]["expense_minor"], 20_100);
    assert_eq!(unfiltered["daily_totals"][0]["transfer_minor"], 500);
}

#[tokio::test]
async fn ledger_date_filters_validate_ranges_and_include_both_boundary_days() {
    let (app, pool, _directory) = setup().await;
    let checking = create_account(&app, "Date boundary checking", 100_000).await;
    let savings = create_account(&app, "Date boundary savings", 20_000).await;
    for (name, date) in [
        ("Before boundary", "2024-02-28T23:59:59Z"),
        ("Leap date", "2024-02-29"),
        ("Leap late local", "2024-02-29T23:59:59.999"),
        ("March local", "2024-03-01T00:00:00"),
        ("March RFC3339", "2024-03-01T23:59:59+05:30"),
        ("After boundary", "2024-03-02"),
    ] {
        create_entry_at(&app, "expense", checking, 100, name, None, date).await;
    }
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/transfers",
        json!({
            "from_account_id": checking,
            "to_account_id": savings,
            "amount_minor": 200,
            "name": "Boundary transfer",
            "occurred_at": "2024-03-01T23:59:59Z"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    // date_only trims legacy padding; no lexical SQL prefix filter may discard these.
    sqlx::query("UPDATE entries SET occurred_at = ? WHERE name = 'Leap late local'")
        .bind(" 2024-02-29T23:59:59.999 ")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE transfers SET occurred_at = ?")
        .bind(" 2024-03-01T23:59:59Z ")
        .execute(&pool)
        .await
        .unwrap();

    let query = "date_from=2024-02-29&date_to=2024-03-01";
    let (status, ledger) = send_json(&app, "GET", &format!("/api/ledger?{query}"), json!({})).await;
    assert_eq!(status, StatusCode::OK, "{ledger}");
    assert_eq!(ledger["total_count"], 5);
    assert_eq!(ledger["daily_totals"][0]["date"], "2024-02-29");
    assert_eq!(ledger["daily_totals"][0]["expense_minor"], 200);
    assert_eq!(ledger["daily_totals"][1]["date"], "2024-03-01");
    assert_eq!(ledger["daily_totals"][1]["expense_minor"], 200);
    assert_eq!(ledger["daily_totals"][1]["transfer_minor"], 200);
    let names = ledger["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["name"].as_str().unwrap())
        .collect::<HashSet<_>>();
    assert_eq!(
        names,
        HashSet::from([
            "Leap date",
            "Leap late local",
            "March local",
            "March RFC3339",
            "Boundary transfer",
        ])
    );
    let (status, history) = send_json(
        &app,
        "GET",
        &format!("/api/accounts/{checking}/history?{query}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{history}");
    assert_eq!(history["total_count"], 5);
    assert_eq!(history["daily_totals"][0]["date"], "2024-02-29");
    assert_eq!(history["daily_totals"][1]["date"], "2024-03-01");
    assert_eq!(history["daily_totals"][1]["transfer_minor"], 200);
    let (status, _content_type, _content_disposition, bytes) =
        send_csv(&app, &format!("/api/ledger/export?{query}")).await;
    assert_eq!(status, StatusCode::OK);
    let csv = String::from_utf8(bytes).unwrap();
    assert_eq!(csv.lines().count(), 6);
    assert!(csv.contains("Leap date"));
    assert!(csv.contains("March RFC3339"));
    assert!(!csv.contains("Before boundary"));
    assert!(!csv.contains("After boundary"));

    for (parameter, value, message) in [
        (
            "date_from",
            "",
            "date_from must be a valid calendar date in YYYY-MM-DD format",
        ),
        (
            "date_from",
            "2024-2-29",
            "date_from must be a valid calendar date in YYYY-MM-DD format",
        ),
        (
            "date_to",
            "2023-02-29",
            "date_to must be a valid calendar date in YYYY-MM-DD format",
        ),
        (
            "date_from",
            "2024-02-29T00:00:00",
            "date_from must be a valid calendar date in YYYY-MM-DD format",
        ),
    ] {
        let invalid = format!("{parameter}={}", query_value(value));
        for uri in [
            format!("/api/ledger?{invalid}"),
            format!("/api/accounts/{checking}/history?{invalid}"),
            format!("/api/ledger/export?{invalid}"),
        ] {
            let (status, body) = send_json(&app, "GET", &uri, json!({})).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {body}");
            assert_eq!(body["error"], message);
        }
    }
    for uri in [
        "/api/ledger?date_from=2024-03-02&date_to=2024-03-01".to_string(),
        format!("/api/accounts/{checking}/history?date_from=2024-03-02&date_to=2024-03-01"),
        "/api/ledger/export?date_from=2024-03-02&date_to=2024-03-01".to_string(),
    ] {
        let (status, body) = send_json(&app, "GET", &uri, json!({})).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {body}");
        assert_eq!(body["error"], "date_from cannot be later than date_to");
    }
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/filters",
        json!({"name": "Invalid saved range", "definition": {"date_from": "2024-02-30"}}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        body["error"],
        "date_from must be a valid calendar date in YYYY-MM-DD format"
    );
}

#[tokio::test]
async fn active_date_filters_exclude_invalid_legacy_timestamps_but_unfiltered_reads_preserve_them()
{
    let (app, pool, _directory) = setup().await;
    let checking = create_account(&app, "Legacy filter checking", 100_000).await;
    let savings = create_account(&app, "Legacy filter savings", 20_000).await;
    for (name, date) in [
        ("Valid local", "2026-11-20T12:00:00"),
        ("Valid RFC3339", "2026-11-20T23:59:59Z"),
        ("Valid date only", "2026-11-20"),
        ("Invalid time", "2026-11-20T12:00:00"),
        ("Impossible date", "2026-11-20T12:00:00"),
    ] {
        create_entry_at(&app, "expense", checking, 100, name, None, date).await;
    }
    for (name, invalid) in [
        ("Invalid time", "2026-11-20T25:00:00"),
        ("Impossible date", "2026-02-30T12:00:00"),
    ] {
        sqlx::query("UPDATE entries SET occurred_at = ? WHERE name = ?")
            .bind(invalid)
            .bind(name)
            .execute(&pool)
            .await
            .unwrap();
    }
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/transfers",
        json!({
            "from_account_id": checking,
            "to_account_id": savings,
            "amount_minor": 300,
            "name": "Invalid legacy transfer",
            "occurred_at": "2026-11-20T12:00:00"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    sqlx::query("UPDATE transfers SET occurred_at = 'not-a-date'")
        .execute(&pool)
        .await
        .unwrap();

    let (status, unfiltered) = send_json(
        &app,
        "GET",
        &format!("/api/accounts/{checking}/history"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{unfiltered}");
    assert_eq!(unfiltered["total_count"], 6);
    let (status, filtered) = send_json(
        &app,
        "GET",
        &format!("/api/accounts/{checking}/history?date_from=2026-11-20&date_to=2026-11-20"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{filtered}");
    assert_eq!(filtered["total_count"], 3);
    assert_eq!(filtered["daily_totals"][0]["expense_minor"], 300);
    let (status, ledger) = send_json(
        &app,
        "GET",
        "/api/ledger?date_from=2026-11-20&date_to=2026-11-20",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{ledger}");
    assert_eq!(ledger["total_count"], 3);

    let (status, _content_type, _content_disposition, bytes) =
        send_csv(&app, "/api/ledger/export").await;
    assert_eq!(status, StatusCode::OK);
    let csv = String::from_utf8(bytes).unwrap();
    assert!(csv.contains(",Expense,Invalid time,"));
    assert!(csv.contains(",Expense,Impossible date,"));
    assert!(csv.contains(",Transfer,Invalid legacy transfer,"));
    assert!(!csv.contains("2026-11-20T25:00:00"));
    assert!(!csv.contains("not-a-date"));
    let (status, _content_type, _content_disposition, bytes) = send_csv(
        &app,
        "/api/ledger/export?date_from=2026-11-20&date_to=2026-11-20",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let csv = String::from_utf8(bytes).unwrap();
    assert_eq!(csv.lines().count(), 4);
    assert!(csv.contains("Valid local"));
    assert!(csv.contains("Valid RFC3339"));
    assert!(csv.contains("Valid date only"));
    assert!(!csv.contains("Invalid time"));
    assert!(!csv.contains("Impossible date"));
    assert!(!csv.contains("Invalid legacy transfer"));
}

#[tokio::test]
async fn search_is_a_literal_case_insensitive_substring_for_ledger_history_and_csv() {
    let (app, _pool, _directory) = setup().await;
    let checking = create_account(&app, "Literal search checking", 100_000).await;
    let savings = create_account(&app, "Literal search savings", 20_000).await;
    create_entry(&app, "expense", checking, 100, "Literal 50%_\\ Match", None).await;
    create_entry(&app, "expense", checking, 200, "Literal 50XX Match", None).await;
    create_entry(&app, "expense", checking, 300, "Note target", None).await;
    create_entry(&app, "expense", checking, 400, "Note decoy", None).await;
    create_entry(
        &app,
        "expense",
        checking,
        700,
        "Électricité MiXeD target",
        None,
    )
    .await;
    create_entry(
        &app,
        "expense",
        checking,
        800,
        "électricité mixed decoy",
        None,
    )
    .await;
    sqlx::query("UPDATE entries SET note = 'note PATH\\%_tail' WHERE name = 'Note target'")
        .execute(&_pool)
        .await
        .unwrap();
    sqlx::query("UPDATE entries SET note = 'note pathXXtail' WHERE name = 'Note decoy'")
        .execute(&_pool)
        .await
        .unwrap();
    for (name, amount) in [("Transfer 50%_\\ Match", 500), ("Transfer 50XX Match", 600)] {
        let (status, body) = send_json(
            &app,
            "POST",
            "/api/transfers",
            json!({
                "from_account_id": checking,
                "to_account_id": savings,
                "amount_minor": amount,
                "name": name,
                "occurred_at": "2026-11-20T12:00:00"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }

    for (search, expected_main, expected_history) in [("50%_\\", 2, 2), ("PATH\\%_", 1, 1)] {
        let query = format!("search={}", query_value(search));
        let (status, ledger) =
            send_json(&app, "GET", &format!("/api/ledger?{query}"), json!({})).await;
        assert_eq!(status, StatusCode::OK, "{ledger}");
        assert_eq!(ledger["total_count"], expected_main);
        assert!(!ledger["entries"].as_array().unwrap().iter().any(|entry| {
            entry["name"].as_str().unwrap().contains("50XX")
                || entry["name"].as_str().unwrap().contains("pathXX")
        }));
        let (status, history) = send_json(
            &app,
            "GET",
            &format!("/api/accounts/{checking}/history?{query}"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{history}");
        assert_eq!(history["total_count"], expected_history);
        let (status, _content_type, _content_disposition, bytes) =
            send_csv(&app, &format!("/api/ledger/export?{query}")).await;
        assert_eq!(status, StatusCode::OK);
        let csv = String::from_utf8(bytes).unwrap();
        assert_eq!(csv.lines().count(), expected_main + 1);
        assert!(!csv.contains("50XX"));
        assert!(!csv.contains("pathXX"));
        if expected_main == 2 {
            assert_eq!(csv.matches(",Transfer,").count(), 1);
            assert!(csv.contains("Literal 50%_\\ Match"));
            assert!(csv.contains("Transfer 50%_\\ Match"));
        } else {
            assert!(csv.contains("Note target"));
            assert!(!csv.contains("Note decoy"));
        }
    }
    let (status, history) = send_json(
        &app,
        "GET",
        &format!(
            "/api/accounts/{savings}/history?search={}",
            query_value("50%_\\")
        ),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{history}");
    assert_eq!(history["total_count"], 1);
    assert_eq!(history["entries"][0]["kind"], "transfer_in");

    for (search, expected_name) in [
        ("Électricité mixed", "Électricité MiXeD target"),
        ("électricité MIXED", "électricité mixed decoy"),
    ] {
        let query = format!("search={}", query_value(search));
        let (status, ledger) =
            send_json(&app, "GET", &format!("/api/ledger?{query}"), json!({})).await;
        assert_eq!(status, StatusCode::OK, "{ledger}");
        assert_eq!(ledger["total_count"], 1);
        assert_eq!(ledger["entries"][0]["name"], expected_name);
        let (status, history) = send_json(
            &app,
            "GET",
            &format!("/api/accounts/{checking}/history?{query}"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{history}");
        assert_eq!(history["total_count"], 1);
        assert_eq!(history["entries"][0]["name"], expected_name);
        let (status, _content_type, _content_disposition, bytes) =
            send_csv(&app, &format!("/api/ledger/export?{query}")).await;
        assert_eq!(status, StatusCode::OK);
        let csv = String::from_utf8(bytes).unwrap();
        assert_eq!(csv.lines().count(), 2);
        assert!(csv.contains(expected_name));
    }
}

#[tokio::test]
async fn extreme_positive_daily_and_group_totals_fit_exactly_or_return_controlled_errors() {
    let (app, _pool, _directory) = setup().await;
    let checking = create_account(&app, "Overflow checking", i64::MAX).await;
    create_entry_at(
        &app,
        "expense",
        checking,
        i64::MAX,
        "Overflow maximum",
        None,
        "2026-11-20T12:00:00",
    )
    .await;
    let (status, exact) = send_json(
        &app,
        "GET",
        "/api/ledger?search=Overflow%20maximum",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{exact}");
    assert_eq!(exact["daily_totals"][0]["expense_minor"], i64::MAX);
    let current = state(&app).await;
    assert_eq!(current["totals"]["expense_minor"], i64::MAX);

    create_entry_at(
        &app,
        "expense",
        checking,
        1,
        "Overflow extra",
        None,
        "2026-11-20T12:00:00",
    )
    .await;
    for uri in [
        "/api/ledger?search=Overflow".to_string(),
        format!("/api/accounts/{checking}/history?search=Overflow"),
    ] {
        let (status, body) = send_json(&app, "GET", &uri, json!({})).await;
        assert_eq!(status, StatusCode::CONFLICT, "{uri}: {body}");
        assert_eq!(
            body["error"],
            "daily ledger expense total is outside the supported integer range"
        );
    }
    let (status, body) = send_json(
        &app,
        "GET",
        "/api/category-budgets?month=2026-11",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        body["error"],
        "monthly expense total is outside the supported integer range"
    );
    let (status, body) = send_json(&app, "GET", "/api/state", json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        body["error"],
        "ledger total is outside the supported integer range"
    );
    let (status, _content_type, _content_disposition, bytes) =
        send_csv(&app, "/api/ledger/export?search=Overflow").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(String::from_utf8(bytes).unwrap().lines().count(), 3);
}
