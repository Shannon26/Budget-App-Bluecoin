use super::*;

#[tokio::test]
async fn csv_export_omits_invalid_legacy_dates_without_leaking_formula_text() {
    let (app, pool, _directory) = setup().await;
    let checking = create_account(&app, "Legacy date checking", 100_000).await;
    let savings = create_account(&app, "Legacy date savings", 20_000).await;

    for (name, date) in [
        ("Legacy formula date", "2026-11-20"),
        ("Legacy impossible date", "2026-11-20"),
        ("Valid timestamp", "2026-11-21T23:59:59Z"),
        ("Valid date only", "2026-11-22"),
    ] {
        let (status, body) = send_json(
            &app,
            "POST",
            "/api/transactions",
            json!({
                "kind": "expense",
                "account_id": checking,
                "amount_minor": 100,
                "name": name,
                "occurred_at": date
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }
    for (name, invalid) in [
        ("Legacy formula date", "=2026-11-20"),
        ("Legacy impossible date", "2026-02-30T12:00:00"),
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
            "amount_minor": 100,
            "name": "Legacy invalid transfer",
            "occurred_at": "2026-11-23T12:00:00"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let transfer_id = body["id"].as_i64().unwrap();
    sqlx::query("UPDATE transfers SET occurred_at = ? WHERE id = ?")
        .bind("not-a-date")
        .bind(transfer_id)
        .execute(&pool)
        .await
        .unwrap();

    let (status, _content_type, _content_disposition, bytes) =
        send_csv(&app, "/api/ledger/export").await;
    assert_eq!(status, StatusCode::OK);
    let csv = String::from_utf8(bytes).unwrap();
    let lines: Vec<&str> = csv.lines().collect();
    assert!(lines
        .iter()
        .find(|line| line.contains("Legacy formula date"))
        .unwrap()
        .starts_with(",Expense,"));
    assert!(lines
        .iter()
        .find(|line| line.contains("Legacy impossible date"))
        .unwrap()
        .starts_with(",Expense,"));
    assert!(lines
        .iter()
        .find(|line| line.contains("Legacy invalid transfer"))
        .unwrap()
        .starts_with(",Transfer,"));
    assert!(lines
        .iter()
        .find(|line| line.contains("Valid timestamp"))
        .unwrap()
        .starts_with("2026-11-21,Expense,"));
    assert!(lines
        .iter()
        .find(|line| line.contains("Valid date only"))
        .unwrap()
        .starts_with("2026-11-22,Expense,"));
    assert!(!csv.contains("=2026-11-20"));
    assert!(!csv.contains("2026-02-30"));
    assert!(!csv.contains("not-a-date"));
}

#[tokio::test]
async fn csv_export_preserves_categories_transfers_and_special_text() {
    let (app, _pool, _directory) = setup().await;
    let checking = create_account(&app, "=Cash, \"Main\"", 100_000).await;
    let savings = create_account(&app, "Savings", 20_000).await;
    let dining = category_id(&app, "expense", "Dining Out").await;

    let (status, body) = send_json(
        &app,
        "POST",
        "/api/transactions",
        json!({
            "kind": "expense",
            "account_id": checking,
            "amount_minor": 1234,
            "name": "=SUM(1,2), \"lunch\"\nsecond line",
            "category_id": dining,
            "reconciled": true,
            "note": "@note\nwith, \"quotes\"",
            "occurred_at": "2026-11-20T10:00:00"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/categories/{dining}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["archived"], true);

    let (status, _content_type, _content_disposition, bytes) =
        send_csv(&app, "/api/ledger/export?search=SUM").await;
    assert_eq!(status, StatusCode::OK);
    let csv = String::from_utf8(bytes).unwrap();
    let expected = r#"Date,Type,Name,Account,Source Account,Destination Account,Category,Subcategory,Amount,Currency,Status,Note
2026-11-20,Expense,"'=SUM(1,2), ""lunch""
second line","'=Cash, ""Main""",,,Entertainment,Dining Out,12.34,INR,Reconciled,"'@note
with, ""quotes"""
"#;
    assert_eq!(csv, expected);

    let (status, body) = send_json(
        &app,
        "POST",
        "/api/transactions",
        json!({
            "kind": "income",
            "account_id": checking,
            "amount_minor": 500000,
            "name": "CSV Salary",
            "occurred_at": "2026-11-20T11:00:00"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let (status, body) = send_json(
        &app,
        "POST",
        "/api/transfers",
        json!({
            "from_account_id": checking,
            "to_account_id": savings,
            "amount_minor": 25000,
            "name": "+CSV transfer, \"move\"",
            "note": "-transfer note",
            "occurred_at": "2026-11-20T12:00:00"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let (status, _content_type, _content_disposition, bytes) =
        send_csv(&app, &format!("/api/ledger/export?account_id={checking}")).await;
    assert_eq!(status, StatusCode::OK);
    let csv = String::from_utf8(bytes).unwrap();
    assert_eq!(
        csv.lines()
            .filter(|line| line.contains(",Transfer,"))
            .count(),
        1
    );
    assert!(csv.contains(
        "2026-11-20,Transfer,\"'+CSV transfer, \"\"move\"\"\",,\"'=Cash, \"\"Main\"\"\",Savings,,,250.00,INR,Unreconciled,'-transfer note"
    ));
    assert!(csv.contains(
        "2026-11-20,Income,CSV Salary,\"'=Cash, \"\"Main\"\"\",,,Uncategorized,,5000.00,INR,Unreconciled,"
    ));
    assert!(!csv.contains("transfer_in"));
    assert!(!csv.contains("transfer_out"));
}

#[tokio::test]
async fn csv_export_returns_every_filtered_row_and_ignores_pagination() {
    let (app, pool, _directory) = setup().await;
    let checking = create_account(&app, "CSV pagination account", 100_000).await;
    sqlx::query(
        "WITH RECURSIVE numbers(value) AS (
             SELECT 1 UNION ALL SELECT value + 1 FROM numbers WHERE value < 201
         )
         INSERT INTO entries
             (kind, account_id, amount_minor, name, category_id, reconciled, note, occurred_at, created_at, transfer_id)
         SELECT 'expense', ?, 100, 'CSV pagination ' || value, NULL, 0, '',
                '2026-11-20T12:00:00', '2026-11-20T12:00:00Z', NULL
         FROM numbers",
    )
    .bind(checking)
    .execute(&pool)
    .await
    .unwrap();
    create_entry(&app, "expense", checking, 999, "Excluded row", None).await;

    let (status, first) = send_json(
        &app,
        "GET",
        &format!("/api/ledger?account_id={checking}&search=CSV%20pagination&limit=1&offset=200"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["total_count"], 201);
    assert_eq!(first["entries"].as_array().unwrap().len(), 1);

    let (status, content_type, content_disposition, bytes) = send_csv(
        &app,
        &format!(
            "/api/ledger/export?account_id={checking}&search=CSV%20pagination&limit=1&offset=200"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type, "text/csv; charset=utf-8");
    assert_eq!(
        content_disposition,
        "attachment; filename=\"paisa-ledger.csv\""
    );
    let csv = String::from_utf8(bytes).unwrap();
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 202);
    assert_eq!(lines[0], csv::LEDGER_CSV_HEADERS.join(","));
    assert!(lines[1].contains("CSV pagination 201,"));
    assert!(lines[201].contains("CSV pagination 1,"));
    assert!(lines
        .iter()
        .skip(1)
        .all(|line| line.contains(",Expense,") && line.contains(",1.00,INR,Unreconciled,")));
    assert!(!csv.contains("Excluded row"));
}
