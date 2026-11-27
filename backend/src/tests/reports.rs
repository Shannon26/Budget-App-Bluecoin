use super::*;

async fn report(app: &Router, month: &str) -> (StatusCode, Value) {
    send_json(
        app,
        "GET",
        &format!("/api/reports/net-earnings?month={month}"),
        json!({}),
    )
    .await
}

async fn create_report_category(
    app: &Router,
    kind: &str,
    name: &str,
    parent_id: Option<i64>,
) -> i64 {
    let (status, body) = send_json(
        app,
        "POST",
        "/api/categories",
        json!({
            "name": name,
            "kind": kind,
            "parent_id": parent_id,
            "color": "#426b8a",
            "icon": "•",
            "sort_order": 100
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_i64().unwrap()
}

fn report_category<'a>(body: &'a Value, name: &str) -> &'a Value {
    body["categories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|category| category["name"] == name)
        .unwrap()
}

#[tokio::test]
async fn net_earnings_uses_half_open_month_bounds_prior_year_and_excludes_transfers() {
    let (app, _pool, _directory) = setup().await;
    let account_id = create_account(&app, "Report account", 999_999).await;
    let other_id = create_account(&app, "Report other", 888_888).await;
    create_entry_at(
        &app,
        "income",
        account_id,
        20_000,
        "Prior start",
        None,
        "2025-12-01",
    )
    .await;
    create_entry_at(
        &app,
        "income",
        account_id,
        10_000,
        "Prior end",
        None,
        "2025-12-31T23:59:59Z",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        account_id,
        2_500,
        "Current start",
        None,
        "2026-01-01",
    )
    .await;
    create_entry_at(
        &app,
        "income",
        account_id,
        50_000,
        "Current end",
        None,
        "2026-01-31T23:59:59+05:30",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        account_id,
        99_900,
        "Next month",
        None,
        "2026-02-01",
    )
    .await;
    let (status, body) = transfer(&app, account_id, other_id, 7_777).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let (status, body) = report(&app, "2026-01").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["month"], "2026-01");
    assert_eq!(body["prior_month"], "2025-12");
    assert_eq!(body["current"]["start_date"], "2026-01-01");
    assert_eq!(body["current"]["end_date_exclusive"], "2026-02-01");
    assert_eq!(body["current"]["income_minor"], 50_000);
    assert_eq!(body["current"]["expense_minor"], 2_500);
    assert_eq!(body["current"]["net_minor"], 47_500);
    assert_eq!(body["current"]["income_count"], 1);
    assert_eq!(body["current"]["expense_count"], 1);
    assert_eq!(body["prior"]["start_date"], "2025-12-01");
    assert_eq!(body["prior"]["end_date_exclusive"], "2026-01-01");
    assert_eq!(body["prior"]["income_minor"], 30_000);
    assert_eq!(body["prior"]["expense_minor"], 0);
    assert_eq!(body["prior"]["net_minor"], 30_000);
    assert_eq!(body["prior"]["income_count"], 2);
    assert_eq!(body["prior"]["expense_count"], 0);
    assert_eq!(body["invalid_date_entry_count"], 0);
}

#[tokio::test]
async fn net_earnings_includes_leap_day_and_returns_real_empty_zeroes() {
    let (app, _pool, _directory) = setup().await;
    let account_id = create_account(&app, "Leap report", 0).await;
    create_entry_at(
        &app,
        "income",
        account_id,
        100,
        "Leap start",
        None,
        "2024-02-01",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        account_id,
        40,
        "Leap day",
        None,
        "2024-02-29",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        account_id,
        999,
        "March",
        None,
        "2024-03-01",
    )
    .await;
    create_entry_at(
        &app,
        "income",
        account_id,
        50,
        "January",
        None,
        "2024-01-31",
    )
    .await;

    let (status, body) = report(&app, "2024-02").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["prior_month"], "2024-01");
    assert_eq!(body["current"]["income_minor"], 100);
    assert_eq!(body["current"]["expense_minor"], 40);
    assert_eq!(body["current"]["net_minor"], 60);
    assert_eq!(body["prior"]["income_minor"], 50);

    let (status, empty) = report(&app, "2030-05").await;
    assert_eq!(status, StatusCode::OK, "{empty}");
    assert_eq!(empty["current"]["income_minor"], 0);
    assert_eq!(empty["current"]["expense_minor"], 0);
    assert_eq!(empty["current"]["net_minor"], 0);
    assert_eq!(empty["current"]["income_count"], 0);
    assert_eq!(empty["current"]["expense_count"], 0);
    assert_eq!(empty["prior"]["income_minor"], 0);
    assert_eq!(empty["prior"]["expense_minor"], 0);
    assert_eq!(empty["invalid_date_entry_count"], 0);
    let uncategorized = report_category(&empty, "Uncategorized");
    assert_eq!(uncategorized["category_id"], Value::Null);
    assert_eq!(uncategorized["current"]["expense"]["direct_minor"], 0);
    assert_eq!(uncategorized["prior"]["income"]["rollup_minor"], 0);
}

#[tokio::test]
async fn report_visibility_excludes_report_hidden_accounts_but_not_picker_hidden_accounts() {
    let (app, _pool, _directory) = setup().await;
    let visible = create_account(&app, "Visible report", 0).await;
    let picker_hidden = create_account(&app, "Picker hidden", 0).await;
    let report_hidden = create_account(&app, "Report hidden", 0).await;
    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/accounts/{picker_hidden}"),
        json!({ "hide_from_selection": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/accounts/{report_hidden}"),
        json!({ "hide_from_reports": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    create_entry_at(
        &app,
        "income",
        visible,
        1_000,
        "Visible income",
        None,
        "2026-11-10",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        picker_hidden,
        200,
        "Picker expense",
        None,
        "2026-11-11",
    )
    .await;
    create_entry_at(
        &app,
        "income",
        report_hidden,
        400,
        "Hidden income",
        None,
        "2026-11-12",
    )
    .await;
    let (status, body) = transfer(&app, visible, picker_hidden, 500).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let (status, body) = report(&app, "2026-11").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["current"]["income_minor"], 1_000);
    assert_eq!(body["current"]["expense_minor"], 200);
    assert_eq!(body["current"]["net_minor"], 800);
    assert_eq!(body["current"]["income_count"], 1);
    assert_eq!(body["current"]["expense_count"], 1);

    // Report-only visibility must not alter ordinary state, ledger, or CSV semantics.
    let current = state(&app).await;
    assert_eq!(current["accounts"].as_array().unwrap().len(), 3);
    assert_eq!(current["totals"]["income_minor"], 1_400);
    assert_eq!(current["totals"]["expense_minor"], 200);
    let (status, ledger) =
        send_json(&app, "GET", "/api/ledger?limit=200&offset=0", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{ledger}");
    assert_eq!(ledger["total_count"], 4);
    let (status, _, _, csv) = send_csv(&app, "/api/ledger/export").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(String::from_utf8(csv).unwrap().lines().count(), 5);
}

#[tokio::test]
async fn category_direct_rollup_archived_uncategorized_and_budget_cross_check() {
    let (app, _pool, _directory) = setup().await;
    let account_id = create_account(&app, "Category report", 0).await;
    let expense_parent = create_report_category(&app, "expense", "Report Expenses", None).await;
    let expense_child =
        create_report_category(&app, "expense", "Report Child", Some(expense_parent)).await;
    let archived_id = create_report_category(&app, "expense", "Report Archived", None).await;
    let income_parent = create_report_category(&app, "income", "Report Income", None).await;
    let income_child =
        create_report_category(&app, "income", "Report Salary", Some(income_parent)).await;

    create_entry_at(
        &app,
        "expense",
        account_id,
        100,
        "Parent direct",
        Some(expense_parent),
        "2026-11-01",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        account_id,
        200,
        "Child expense",
        Some(expense_child),
        "2026-11-02",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        account_id,
        300,
        "Archived expense",
        Some(archived_id),
        "2026-11-03",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        account_id,
        400,
        "Uncategorized expense",
        None,
        "2026-11-04",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        account_id,
        50,
        "Prior child",
        Some(expense_child),
        "2026-10-15",
    )
    .await;
    create_entry_at(
        &app,
        "income",
        account_id,
        700,
        "Income parent",
        Some(income_parent),
        "2026-11-05",
    )
    .await;
    create_entry_at(
        &app,
        "income",
        account_id,
        800,
        "Income child",
        Some(income_child),
        "2026-11-06",
    )
    .await;
    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/categories/{archived_id}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["archived"], true);

    let (status, body) = report(&app, "2026-11").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["current"]["income_minor"], 1_500);
    assert_eq!(body["current"]["expense_minor"], 1_000);
    assert_eq!(body["current"]["net_minor"], 500);
    assert_eq!(body["current"]["income_count"], 2);
    assert_eq!(body["current"]["expense_count"], 4);
    assert_eq!(body["prior"]["expense_minor"], 50);

    let parent = report_category(&body, "Report Expenses");
    assert_eq!(parent["current"]["expense"]["direct_minor"], 100);
    assert_eq!(parent["current"]["expense"]["rollup_minor"], 300);
    assert_eq!(parent["current"]["expense"]["direct_count"], 1);
    assert_eq!(parent["current"]["expense"]["rollup_count"], 2);
    assert_eq!(parent["prior"]["expense"]["direct_minor"], 0);
    assert_eq!(parent["prior"]["expense"]["rollup_minor"], 50);
    let child = report_category(&body, "Report Child");
    assert_eq!(child["current"]["expense"]["direct_minor"], 200);
    assert_eq!(child["current"]["expense"]["rollup_minor"], 200);
    let archived = report_category(&body, "Report Archived");
    assert_eq!(archived["current"]["expense"]["direct_minor"], 300);
    assert!(archived["archived_at"].is_string());
    let uncategorized = report_category(&body, "Uncategorized");
    assert_eq!(uncategorized["current"]["expense"]["direct_minor"], 400);
    assert_eq!(uncategorized["current"]["expense"]["rollup_minor"], 400);
    let income = report_category(&body, "Report Income");
    assert_eq!(income["current"]["income"]["direct_minor"], 700);
    assert_eq!(income["current"]["income"]["rollup_minor"], 1_500);
    assert_eq!(income["current"]["expense"]["rollup_minor"], 0);

    let names = body["categories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|category| category["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(
        names.iter().position(|name| *name == "Report Expenses")
            < names.iter().position(|name| *name == "Report Child")
    );

    // Where inclusion rules match, the report expense total/count equals the budget fixture.
    let budgets = monthly_budgets(&app, "2026-11").await;
    assert_eq!(
        budgets["total_spent_minor"],
        body["current"]["expense_minor"]
    );
    assert_eq!(budgets["expense_count"], body["current"]["expense_count"]);
    assert_eq!(budget_row(&budgets, expense_parent)["spent_minor"], 300);
    assert_eq!(budget_row(&budgets, expense_child)["spent_minor"], 200);
    assert_eq!(budget_row(&budgets, archived_id)["spent_minor"], 300);
}

#[tokio::test]
async fn invalid_legacy_dates_are_counted_but_excluded_from_both_periods() {
    let (app, pool, _directory) = setup().await;
    let account_id = create_account(&app, "Invalid date report", 0).await;
    let hidden_id = create_account(&app, "Hidden invalid date", 0).await;
    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/accounts/{hidden_id}"),
        json!({ "hide_from_reports": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    create_entry_at(
        &app,
        "expense",
        account_id,
        100,
        "Valid",
        None,
        "2026-11-10",
    )
    .await;
    for (kind, amount, occurred_at) in [
        ("income", 999, "not-a-date"),
        ("expense", 888, "2026-02-30T12:00:00Z"),
    ] {
        sqlx::query(
            "INSERT INTO entries (kind, account_id, amount_minor, note, occurred_at, created_at, transfer_id)
             VALUES (?, ?, ?, 'legacy invalid', ?, '2026-11-01T00:00:00Z', NULL)",
        )
        .bind(kind)
        .bind(account_id)
        .bind(amount)
        .bind(occurred_at)
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO entries (kind, account_id, amount_minor, note, occurred_at, created_at, transfer_id)
         VALUES ('income', ?, 777, 'hidden invalid', 'bad-date', '2026-11-01T00:00:00Z', NULL)",
    )
    .bind(hidden_id)
    .execute(&pool)
    .await
    .unwrap();

    let (status, body) = report(&app, "2026-11").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["invalid_date_entry_count"], 2);
    assert_eq!(body["current"]["income_minor"], 0);
    assert_eq!(body["current"]["expense_minor"], 100);
    assert_eq!(body["current"]["expense_count"], 1);
    assert_eq!(body["current"]["net_minor"], -100);
    assert_eq!(body["prior"]["income_minor"], 0);
    assert_eq!(body["prior"]["expense_minor"], 0);
}

#[tokio::test]
async fn report_month_extremes_are_safe_and_budget_bounds_are_unchanged() {
    let (app, _pool, _directory) = setup().await;

    let (status, body) = report(&app, "0000-01").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body["error"].as_str().unwrap().contains("previous month"));

    let (status, body) = report(&app, "9999-12").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("representable calendar boundaries"));

    let (status, earliest) = report(&app, "0001-02").await;
    assert_eq!(status, StatusCode::OK, "{earliest}");
    assert_eq!(earliest["prior_month"], "0001-01");
    assert_eq!(earliest["current"]["start_date"], "0001-02-01");
    assert_eq!(earliest["current"]["end_date_exclusive"], "0001-03-01");
    assert_eq!(earliest["prior"]["end_date_exclusive"], "0001-02-01");

    let (status, latest) = report(&app, "9999-11").await;
    assert_eq!(status, StatusCode::OK, "{latest}");
    assert_eq!(latest["prior_month"], "9999-10");
    assert_eq!(latest["current"]["start_date"], "9999-11-01");
    assert_eq!(latest["current"]["end_date_exclusive"], "9999-12-01");
    assert_eq!(latest["prior"]["end_date_exclusive"], "9999-11-01");

    // The existing budget helper keeps its internal 9999-12-32 comparison sentinel.
    let (status, budgets) = send_json(
        &app,
        "GET",
        "/api/category-budgets?month=9999-12",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{budgets}");
    assert_eq!(budgets["month"], "9999-12");
}

#[tokio::test]
async fn report_overflow_and_invalid_month_are_rejected() {
    let (app, pool, _directory) = setup().await;
    let account_id = create_account(&app, "Overflow report", 0).await;
    for index in 0..2 {
        sqlx::query(
            "INSERT INTO entries (kind, account_id, amount_minor, note, occurred_at, created_at, transfer_id)
             VALUES ('income', ?, ?, 'overflow', '2026-11-10', '2026-11-10T00:00:00Z', NULL)",
        )
        .bind(account_id)
        .bind(i64::MAX)
        .bind(format!("overflow {index}"))
        .execute(&pool)
        .await
        .unwrap();
    }
    let (status, body) = report(&app, "2026-11").await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(body["error"].as_str().unwrap().contains("integer range"));

    for month in ["2026-13", "2026-00", "0001-01", "not-a-month"] {
        let (status, body) = report(&app, month).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{month}: {body}");
    }
}
