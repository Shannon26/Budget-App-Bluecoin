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

async fn items_report(app: &Router, month: &str) -> (StatusCode, Value) {
    send_json(
        app,
        "GET",
        &format!("/api/reports/items-summary?month={month}"),
        json!({}),
    )
    .await
}

async fn label_report(app: &Router, month: &str) -> (StatusCode, Value) {
    send_json(
        app,
        "GET",
        &format!("/api/reports/labels?month={month}"),
        json!({}),
    )
    .await
}

async fn create_report_label(app: &Router, name: &str) -> i64 {
    let (status, body) = send_json(app, "POST", "/api/labels", json!({ "name": name })).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_i64().unwrap()
}

async fn create_labelled_entry_at(
    app: &Router,
    kind: &str,
    account_id: i64,
    amount_minor: i64,
    name: &str,
    labels: &[i64],
    occurred_at: &str,
) -> i64 {
    let (status, body) = send_json(
        app,
        "POST",
        "/api/transactions",
        json!({
            "kind": kind,
            "account_id": account_id,
            "amount_minor": amount_minor,
            "name": name,
            "labels": labels,
            "occurred_at": occurred_at
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_i64().unwrap()
}

fn label_report_row<'a>(body: &'a Value, label_id: Option<i64>) -> &'a Value {
    body["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["label_id"] == json!(label_id))
        .unwrap()
}

async fn daily_report(app: &Router, anchor_date: &str) -> (StatusCode, Value) {
    send_json(
        app,
        "GET",
        &format!("/api/reports/daily-summary?anchor_date={anchor_date}"),
        json!({}),
    )
    .await
}

async fn balance_report(app: &Router, month: &str) -> (StatusCode, Value) {
    send_json(
        app,
        "GET",
        &format!("/api/reports/balance-sheet?month={month}"),
        json!({}),
    )
    .await
}

async fn create_typed_account(
    app: &Router,
    name: &str,
    account_type: &str,
    opening_balance_minor: i64,
    opening_date: Option<&str>,
) -> i64 {
    let (status, body) = send_json(
        app,
        "POST",
        "/api/accounts",
        json!({
            "name": name,
            "currency": "INR",
            "account_type": account_type,
            "opening_balance_minor": opening_balance_minor,
            "opening_date": opening_date
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_i64().unwrap()
}

fn balance_group<'a>(body: &'a Value, key: &str) -> &'a Value {
    body["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["key"] == key)
        .unwrap()
}

fn balance_account<'a>(body: &'a Value, account_id: i64) -> &'a Value {
    body["groups"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|group| group["accounts"].as_array().unwrap())
        .find(|account| account["account_id"] == account_id)
        .unwrap()
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

#[tokio::test]
async fn items_summary_groups_exact_names_ranks_ties_and_reconciles_totals() {
    let (app, _pool, _directory) = setup().await;
    let account_id = create_account(&app, "Items opening balance", 999_999).await;
    for (kind, amount, name) in [
        ("income", 1_000, "Payee"),
        ("income", 2_500, "Payee"),
        ("income", 2_000, "Tie Alpha"),
        ("income", 2_000, "Tie Beta"),
        ("income", 500, "payee"),
        ("expense", 400, "Shop"),
        ("expense", 600, "Shop"),
        ("expense", 1_000, "Rent"),
        ("expense", 100, "Snack"),
    ] {
        create_entry_at(
            &app,
            kind,
            account_id,
            amount,
            name,
            None,
            "2026-11-15T12:00:00",
        )
        .await;
    }

    let (status, body) = items_report(&app, "2026-11").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["month"], "2026-11");
    assert_eq!(body["current"]["start_date"], "2026-11-01");
    assert_eq!(body["current"]["end_date_exclusive"], "2026-12-01");
    assert_eq!(body["current"]["income_minor"], 8_000);
    assert_eq!(body["current"]["income_count"], 5);
    assert_eq!(body["current"]["expense_minor"], 2_100);
    assert_eq!(body["current"]["expense_count"], 4);
    assert_eq!(body["current"]["net_minor"], 5_900);
    let income = body["income_items"].as_array().unwrap();
    assert_eq!(income.len(), 4);
    assert_eq!(income[0]["name"], "Payee");
    assert_eq!(income[0]["rank"], 1);
    assert_eq!(income[0]["total_minor"], 3_500);
    assert_eq!(income[0]["entry_count"], 2);
    assert_eq!(income[1]["name"], "Tie Alpha");
    assert_eq!(income[1]["rank"], 2);
    assert_eq!(income[2]["name"], "Tie Beta");
    assert_eq!(income[2]["rank"], 2);
    assert_eq!(income[3]["name"], "payee");
    assert_eq!(income[3]["rank"], 4);
    assert_eq!(income[3]["total_minor"], 500);
    let expense = body["expense_items"].as_array().unwrap();
    assert_eq!(expense.len(), 3);
    assert_eq!(expense[0]["name"], "Rent");
    assert_eq!(expense[0]["rank"], 1);
    assert_eq!(expense[1]["name"], "Shop");
    assert_eq!(expense[1]["rank"], 1);
    assert_eq!(expense[1]["total_minor"], 1_000);
    assert_eq!(expense[1]["entry_count"], 2);
    assert_eq!(expense[2]["name"], "Snack");
    assert_eq!(expense[2]["rank"], 3);
    assert_eq!(
        income
            .iter()
            .map(|item| item["total_minor"].as_i64().unwrap())
            .sum::<i64>(),
        body["current"]["income_minor"].as_i64().unwrap()
    );
    assert_eq!(
        expense
            .iter()
            .map(|item| item["total_minor"].as_i64().unwrap())
            .sum::<i64>(),
        body["current"]["expense_minor"].as_i64().unwrap()
    );
}

#[tokio::test]
async fn items_summary_uses_report_period_visibility_invalid_dates_and_transfer_rules() {
    let (app, pool, _directory) = setup().await;
    let visible = create_account(&app, "Items visible", 0).await;
    let picker_hidden = create_account(&app, "Items picker hidden", 0).await;
    let report_hidden = create_account(&app, "Items report hidden", 0).await;
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
        100,
        "Boundary",
        None,
        "2026-10-31T23:59:59Z",
    )
    .await;
    create_entry_at(&app, "income", visible, 200, "Boundary", None, "2026-11-01").await;
    create_entry_at(
        &app,
        "expense",
        visible,
        300,
        "Boundary",
        None,
        "2026-11-30T23:59:59+05:30",
    )
    .await;
    create_entry_at(
        &app,
        "income",
        visible,
        999,
        "Next month",
        None,
        "2026-12-01",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        picker_hidden,
        50,
        "Picker only",
        None,
        "2026-11-10",
    )
    .await;
    create_entry_at(
        &app,
        "income",
        report_hidden,
        9_999,
        "Hidden payee",
        None,
        "2026-11-10",
    )
    .await;
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/transfers",
        json!({
            "from_account_id": visible,
            "to_account_id": picker_hidden,
            "amount_minor": 777,
            "name": "Excluded transfer",
            "occurred_at": "2026-11-10T12:00:00"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    for (account_id, kind, name, invalid) in [
        (visible, "income", "Invalid visible income", "not-a-date"),
        (
            visible,
            "expense",
            "Invalid visible expense",
            "2026-02-30T12:00:00Z",
        ),
        (report_hidden, "income", "Invalid hidden", "bad-date"),
    ] {
        sqlx::query(
            "INSERT INTO entries (kind, account_id, amount_minor, name, note, occurred_at, created_at, transfer_id)
             VALUES (?, ?, 100, ?, 'legacy invalid', ?, '2026-11-01T00:00:00Z', NULL)",
        )
        .bind(kind)
        .bind(account_id)
        .bind(name)
        .bind(invalid)
        .execute(&pool)
        .await
        .unwrap();
    }

    let (status, body) = items_report(&app, "2026-11").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["current"]["income_minor"], 200);
    assert_eq!(body["current"]["expense_minor"], 350);
    assert_eq!(body["current"]["income_count"], 1);
    assert_eq!(body["current"]["expense_count"], 2);
    assert_eq!(body["invalid_date_entry_count"], 2);
    assert_eq!(body["income_items"].as_array().unwrap().len(), 1);
    assert_eq!(body["income_items"][0]["name"], "Boundary");
    assert_eq!(body["expense_items"].as_array().unwrap().len(), 2);
    assert!(!body.to_string().contains("Hidden payee"));
    assert!(!body.to_string().contains("Excluded transfer"));
    assert!(!body.to_string().contains("Next month"));

    let (status, prior) = items_report(&app, "2026-10").await;
    assert_eq!(status, StatusCode::OK, "{prior}");
    assert_eq!(prior["current"]["income_minor"], 100);
    assert_eq!(prior["current"]["expense_minor"], 0);
    assert_eq!(prior["income_items"][0]["name"], "Boundary");
}

#[tokio::test]
async fn items_summary_empty_invalid_month_and_overflow_are_explicit() {
    let (app, pool, _directory) = setup().await;
    let account_id = create_account(&app, "Items empty and overflow", 0).await;
    let (status, empty) = items_report(&app, "2030-05").await;
    assert_eq!(status, StatusCode::OK, "{empty}");
    assert_eq!(empty["current"]["income_minor"], 0);
    assert_eq!(empty["current"]["expense_minor"], 0);
    assert_eq!(empty["income_items"], json!([]));
    assert_eq!(empty["expense_items"], json!([]));
    assert_eq!(empty["invalid_date_entry_count"], 0);

    for month in ["2026-13", "2026-00", "0001-01", "not-a-month"] {
        let (status, body) = items_report(&app, month).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{month}: {body}");
    }

    for _ in 0..2 {
        sqlx::query(
            "INSERT INTO entries (kind, account_id, amount_minor, name, note, occurred_at, created_at, transfer_id)
             VALUES ('income', ?, ?, 'Overflow payee', '', '2026-11-10', '2026-11-10T00:00:00Z', NULL)",
        )
        .bind(account_id)
        .bind(i64::MAX)
        .execute(&pool)
        .await
        .unwrap();
    }
    let (status, body) = items_report(&app, "2026-11").await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        body["error"],
        "report income item total is outside the supported integer range"
    );
}

#[tokio::test]
async fn label_report_conserves_paise_with_stable_id_leftovers_renames_and_no_labels_identity() {
    let (app, _pool, _directory) = setup().await;
    let account = create_account(&app, "Label allocation opening balance", 900_000).await;
    let first = create_report_label(&app, "Zulu").await;
    let second = create_report_label(&app, "Alpha").await;
    let user_no_labels = create_report_label(&app, "No Labels").await;
    let unused = create_report_label(&app, "Unused").await;

    create_entry_at(
        &app,
        "expense",
        account,
        101,
        "Unlabelled",
        None,
        "2026-11-01",
    )
    .await;
    create_labelled_entry_at(
        &app,
        "income",
        account,
        1_000,
        "Single label",
        &[user_no_labels],
        "2026-11-02",
    )
    .await;
    create_labelled_entry_at(
        &app,
        "expense",
        account,
        100,
        "Three labels",
        &[second, first, user_no_labels],
        "2026-11-03",
    )
    .await;
    create_labelled_entry_at(
        &app,
        "income",
        account,
        5,
        "Two labels",
        &[second, first],
        "2026-11-04",
    )
    .await;
    create_labelled_entry_at(
        &app,
        "expense",
        account,
        2,
        "More labels than paise",
        &[user_no_labels, second, first],
        "2026-11-05",
    )
    .await;

    let (status, body) = label_report(&app, "2026-11").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["month"], "2026-11");
    assert_eq!(body["current"]["start_date"], "2026-11-01");
    assert_eq!(body["current"]["end_date_exclusive"], "2026-12-01");
    assert_eq!(body["current"]["income_minor"], 1_005);
    assert_eq!(body["current"]["expense_minor"], 203);
    assert_eq!(body["current"]["net_minor"], 802);
    assert_eq!(body["current"]["income_count"], 2);
    assert_eq!(body["current"]["expense_count"], 3);
    assert_eq!(body["invalid_date_entry_count"], 0);

    let names = body["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(names, ["Alpha", "No Labels", "Unused", "Zulu", "No Labels"]);
    assert_eq!(label_report_row(&body, Some(first))["income_minor"], 3);
    assert_eq!(label_report_row(&body, Some(first))["expense_minor"], 35);
    assert_eq!(label_report_row(&body, Some(first))["net_minor"], -32);
    assert_eq!(label_report_row(&body, Some(second))["income_minor"], 2);
    assert_eq!(label_report_row(&body, Some(second))["expense_minor"], 34);
    assert_eq!(
        label_report_row(&body, Some(user_no_labels))["income_minor"],
        1_000
    );
    assert_eq!(
        label_report_row(&body, Some(user_no_labels))["expense_minor"],
        33
    );
    assert_eq!(
        label_report_row(&body, Some(user_no_labels))["is_no_labels"],
        false
    );
    assert_eq!(label_report_row(&body, Some(unused))["income_minor"], 0);
    assert_eq!(label_report_row(&body, Some(unused))["expense_minor"], 0);
    assert_eq!(label_report_row(&body, None)["name"], "No Labels");
    assert_eq!(label_report_row(&body, None)["is_no_labels"], true);
    assert_eq!(label_report_row(&body, None)["income_minor"], 0);
    assert_eq!(label_report_row(&body, None)["expense_minor"], 101);
    assert_eq!(label_report_row(&body, None)["net_minor"], -101);

    let income_sum = body["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["income_minor"].as_i64().unwrap())
        .sum::<i64>();
    let expense_sum = body["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["expense_minor"].as_i64().unwrap())
        .sum::<i64>();
    assert_eq!(
        income_sum,
        body["current"]["income_minor"].as_i64().unwrap()
    );
    assert_eq!(
        expense_sum,
        body["current"]["expense_minor"].as_i64().unwrap()
    );

    // Renaming changes display order, never the stable-ID leftover allocation.
    for (id, name) in [(first, "Aardvark"), (second, "Zebra")] {
        let (status, body) = send_json(
            &app,
            "PUT",
            &format!("/api/labels/{id}"),
            json!({ "name": name }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    let (status, renamed) = label_report(&app, "2026-11").await;
    assert_eq!(status, StatusCode::OK, "{renamed}");
    assert_eq!(renamed["rows"][0]["label_id"], first);
    assert_eq!(renamed["rows"][0]["name"], "Aardvark");
    assert_eq!(renamed["rows"][0]["income_minor"], 3);
    assert_eq!(renamed["rows"][0]["expense_minor"], 35);
    assert_eq!(label_report_row(&renamed, Some(second))["name"], "Zebra");
    assert_eq!(label_report_row(&renamed, Some(second))["income_minor"], 2);
    assert_eq!(
        label_report_row(&renamed, Some(second))["expense_minor"],
        34
    );
    assert_eq!(
        label_report_row(&renamed, Some(user_no_labels))["is_no_labels"],
        false
    );
    assert_eq!(label_report_row(&renamed, None)["is_no_labels"], true);
}

#[tokio::test]
async fn label_report_matches_net_earnings_visibility_dates_transfers_and_invalid_date_rules() {
    let (app, pool, _directory) = setup().await;
    let visible = create_account(&app, "Label report visible", 1_000_000).await;
    let picker_hidden = create_account(&app, "Label report picker hidden", 0).await;
    let report_hidden = create_account(&app, "Label report report hidden", 0).await;
    let destination = create_account(&app, "Label report transfer destination", 0).await;
    for (account_id, update) in [
        (picker_hidden, json!({ "hide_from_selection": true })),
        (report_hidden, json!({ "hide_from_reports": true })),
    ] {
        let (status, body) =
            send_json(&app, "PUT", &format!("/api/accounts/{account_id}"), update).await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    let work = create_report_label(&app, "Work").await;

    create_labelled_entry_at(
        &app,
        "income",
        visible,
        999,
        "Prior boundary",
        &[work],
        "2026-10-31T23:59:59Z",
    )
    .await;
    create_labelled_entry_at(
        &app,
        "expense",
        visible,
        11,
        "Current start",
        &[work],
        "2026-11-01",
    )
    .await;
    create_labelled_entry_at(
        &app,
        "income",
        visible,
        22,
        "Current end",
        &[work],
        "2026-11-30T23:59:59+05:30",
    )
    .await;
    create_labelled_entry_at(
        &app,
        "expense",
        visible,
        33,
        "Next month",
        &[work],
        "2026-12-01",
    )
    .await;
    create_labelled_entry_at(
        &app,
        "expense",
        picker_hidden,
        44,
        "Picker only",
        &[work],
        "2026-11-10",
    )
    .await;
    create_labelled_entry_at(
        &app,
        "income",
        report_hidden,
        55,
        "Report hidden",
        &[work],
        "2026-11-10",
    )
    .await;
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/transfers",
        json!({
            "from_account_id": visible,
            "to_account_id": destination,
            "amount_minor": 66,
            "name": "Excluded labelled transfer",
            "labels": [work],
            "occurred_at": "2026-11-10T12:00:00"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    for (kind, name, occurred_at) in [
        ("income", "Invalid visible income", "not-a-date"),
        ("expense", "Invalid visible expense", "2026-02-30T12:00:00Z"),
    ] {
        sqlx::query(
            "INSERT INTO entries (kind, account_id, amount_minor, name, note, occurred_at, created_at, transfer_id)
             VALUES (?, ?, 100, ?, 'legacy invalid', ?, '2026-11-01T00:00:00Z', NULL)",
        )
        .bind(kind)
        .bind(visible)
        .bind(name)
        .bind(occurred_at)
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO entries (kind, account_id, amount_minor, name, note, occurred_at, created_at, transfer_id)
         VALUES ('income', ?, 100, 'Invalid hidden', 'legacy invalid', 'bad-date', '2026-11-01T00:00:00Z', NULL)",
    )
    .bind(report_hidden)
    .execute(&pool)
    .await
    .unwrap();

    let (status, body) = label_report(&app, "2026-11").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["current"]["income_minor"], 22);
    assert_eq!(body["current"]["expense_minor"], 55);
    assert_eq!(body["current"]["net_minor"], -33);
    assert_eq!(body["current"]["income_count"], 1);
    assert_eq!(body["current"]["expense_count"], 2);
    assert_eq!(body["invalid_date_entry_count"], 2);
    assert_eq!(label_report_row(&body, Some(work))["income_minor"], 22);
    assert_eq!(label_report_row(&body, Some(work))["expense_minor"], 55);
    assert_eq!(label_report_row(&body, None)["expense_minor"], 0);
    assert!(!body.to_string().contains("Excluded labelled transfer"));

    let (status, net) = report(&app, "2026-11").await;
    assert_eq!(status, StatusCode::OK, "{net}");
    assert_eq!(
        net["current"]["income_minor"],
        body["current"]["income_minor"]
    );
    assert_eq!(
        net["current"]["expense_minor"],
        body["current"]["expense_minor"]
    );
    assert_eq!(net["current"]["net_minor"], body["current"]["net_minor"]);

    let (status, prior) = label_report(&app, "2026-10").await;
    assert_eq!(status, StatusCode::OK, "{prior}");
    assert_eq!(prior["current"]["income_minor"], 999);
    assert_eq!(prior["current"]["expense_minor"], 0);
    assert_eq!(label_report_row(&prior, Some(work))["income_minor"], 999);
}

#[tokio::test]
async fn label_report_empty_month_bounds_and_overflow_are_explicit() {
    let (app, _pool, _directory) = setup().await;
    let account = create_account(&app, "Label report empty and overflow", 0).await;
    let work = create_report_label(&app, "Overflow label").await;

    let (status, empty) = label_report(&app, "2030-05").await;
    assert_eq!(status, StatusCode::OK, "{empty}");
    assert_eq!(empty["current"]["income_minor"], 0);
    assert_eq!(empty["current"]["expense_minor"], 0);
    assert_eq!(empty["current"]["net_minor"], 0);
    assert_eq!(empty["current"]["income_count"], 0);
    assert_eq!(empty["current"]["expense_count"], 0);
    assert_eq!(empty["invalid_date_entry_count"], 0);
    assert_eq!(empty["rows"].as_array().unwrap().len(), 2);
    assert_eq!(label_report_row(&empty, Some(work))["income_minor"], 0);
    assert_eq!(label_report_row(&empty, Some(work))["expense_minor"], 0);
    assert_eq!(label_report_row(&empty, None)["is_no_labels"], true);
    assert_eq!(label_report_row(&empty, None)["income_minor"], 0);
    assert_eq!(label_report_row(&empty, None)["expense_minor"], 0);

    for month in ["2026-13", "2026-00", "0001-01", "9999-12", "not-a-month"] {
        let (status, body) = label_report(&app, month).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{month}: {body}");
    }
    for month in ["0001-02", "9999-11"] {
        let (status, body) = label_report(&app, month).await;
        assert_eq!(status, StatusCode::OK, "{month}: {body}");
        assert_eq!(body["month"], month);
    }

    for index in 0..2 {
        create_labelled_entry_at(
            &app,
            "income",
            account,
            i64::MAX,
            &format!("Overflow {index}"),
            &[work],
            "2026-11-10",
        )
        .await;
    }
    let (status, body) = label_report(&app, "2026-11").await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(body["error"].as_str().unwrap().contains("integer range"));
}

#[tokio::test]
async fn daily_summary_includes_boundaries_zero_days_and_full_window_averages() {
    let (app, _pool, _directory) = setup().await;
    let account_id = create_account(&app, "Daily boundary account", 0).await;
    for (amount, date) in [
        (99_000, "2025-12-08T23:59:59Z"),
        (15, "2025-12-09"),
        (10, "2025-12-31T23:59:59"),
        (100, "2026-01-01"),
        (200, "2026-01-03T12:00:00"),
        (300, "2026-01-07T23:59:59+05:30"),
        (999, "2026-01-08"),
    ] {
        create_entry_at(
            &app,
            "expense",
            account_id,
            amount,
            "Daily boundary",
            None,
            date,
        )
        .await;
    }

    let (status, body) = daily_report(&app, "2026-01-07").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["anchor_date"], "2026-01-07");
    assert_eq!(body["seven_day_start_date"], "2026-01-01");
    assert_eq!(body["thirty_day_start_date"], "2025-12-09");
    assert_eq!(body["average_7_minor"], 86);
    assert_eq!(body["average_30_minor"], 21);
    let days = body["days"].as_array().unwrap();
    assert_eq!(days.len(), 7);
    assert_eq!(days[0]["date"], "2026-01-01");
    assert_eq!(days[0]["expense_minor"], 100);
    assert_eq!(days[1]["expense_minor"], 0);
    assert_eq!(days[2]["expense_minor"], 200);
    assert_eq!(days[3]["expense_minor"], 0);
    assert_eq!(days[4]["expense_minor"], 0);
    assert_eq!(days[5]["expense_minor"], 0);
    assert_eq!(days[6]["date"], "2026-01-07");
    assert_eq!(days[6]["expense_minor"], 300);
}

#[tokio::test]
async fn daily_summary_rounds_half_paise_up_and_returns_real_empty_zeroes() {
    let (app, _pool, _directory) = setup().await;
    let account_id = create_account(&app, "Daily rounding account", 0).await;
    create_entry_at(
        &app,
        "expense",
        account_id,
        15,
        "Half paise average",
        None,
        "2030-01-01",
    )
    .await;

    let (status, body) = daily_report(&app, "2030-01-30").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["thirty_day_start_date"], "2030-01-01");
    assert_eq!(body["average_30_minor"], 1);
    assert_eq!(body["average_7_minor"], 0);
    assert!(body["days"]
        .as_array()
        .unwrap()
        .iter()
        .all(|day| day["expense_minor"] == 0));

    let (status, empty) = daily_report(&app, "2030-02-28").await;
    assert_eq!(status, StatusCode::OK, "{empty}");
    assert_eq!(empty["average_7_minor"], 0);
    assert_eq!(empty["average_30_minor"], 0);
    assert_eq!(empty["days"].as_array().unwrap().len(), 7);
    assert!(empty["days"]
        .as_array()
        .unwrap()
        .iter()
        .all(|day| day["expense_minor"] == 0));
}

#[tokio::test]
async fn daily_summary_uses_posted_expenses_visibility_and_invalid_date_rules() {
    let (app, pool, _directory) = setup().await;
    let visible = create_account(&app, "Daily visible", 999_999).await;
    let picker_hidden = create_account(&app, "Daily picker hidden", 0).await;
    let report_hidden = create_account(&app, "Daily report hidden", 0).await;
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
        "expense",
        visible,
        100,
        "Visible expense",
        None,
        "2026-11-27",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        picker_hidden,
        200,
        "Picker expense",
        None,
        "2026-11-27T12:00:00",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        report_hidden,
        9_999,
        "Hidden expense",
        None,
        "2026-11-27",
    )
    .await;
    create_entry_at(
        &app,
        "income",
        visible,
        5_000,
        "Income excluded",
        None,
        "2026-11-27",
    )
    .await;
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/transfers",
        json!({
            "from_account_id": visible,
            "to_account_id": picker_hidden,
            "amount_minor": 777,
            "name": "Transfer excluded",
            "occurred_at": "2026-11-27T12:00:00"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    for (account_id, kind, name, invalid) in [
        (visible, "expense", "Invalid visible expense", "not-a-date"),
        (
            visible,
            "income",
            "Invalid visible income",
            "2026-02-30T12:00:00Z",
        ),
        (report_hidden, "expense", "Invalid hidden", "bad-date"),
    ] {
        sqlx::query(
            "INSERT INTO entries (kind, account_id, amount_minor, name, note, occurred_at, created_at, transfer_id)
             VALUES (?, ?, 100, ?, 'legacy invalid', ?, '2026-11-01T00:00:00Z', NULL)",
        )
        .bind(kind)
        .bind(account_id)
        .bind(name)
        .bind(invalid)
        .execute(&pool)
        .await
        .unwrap();
    }

    let (status, body) = daily_report(&app, "2026-11-27").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["days"][6]["expense_minor"], 300);
    assert_eq!(body["average_7_minor"], 43);
    assert_eq!(body["average_30_minor"], 10);
    assert_eq!(body["invalid_date_entry_count"], 2);
    let current = state(&app).await;
    assert_eq!(current["totals"]["income_minor"], 5_100);
    assert_eq!(current["totals"]["expense_minor"], 10_499);
}

#[tokio::test]
async fn daily_summary_rejects_invalid_or_early_anchors_and_controls_overflow() {
    let (app, pool, _directory) = setup().await;
    for anchor in [
        "",
        "not-a-date",
        "2026-02-30",
        "2026-1-01",
        "2026-01-01T00:00:00",
    ] {
        let (status, body) = daily_report(&app, anchor).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{anchor}: {body}");
        assert_eq!(
            body["error"],
            "anchor_date must be a valid calendar date in YYYY-MM-DD format"
        );
    }
    for anchor in ["0000-01-01", "0000-01-29"] {
        let (status, body) = daily_report(&app, anchor).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{anchor}: {body}");
        assert!(body["error"]
            .as_str()
            .unwrap()
            .contains("complete 30-day window"));
    }
    let (status, year_zero) = daily_report(&app, "0000-12-31").await;
    assert_eq!(status, StatusCode::OK, "{year_zero}");
    assert_eq!(year_zero["thirty_day_start_date"], "0000-12-02");
    let (status, earliest) = daily_report(&app, "0000-01-30").await;
    assert_eq!(status, StatusCode::OK, "{earliest}");
    assert_eq!(earliest["thirty_day_start_date"], "0000-01-01");
    assert_eq!(earliest["seven_day_start_date"], "0000-01-24");
    let (status, latest) = daily_report(&app, "9999-12-31").await;
    assert_eq!(status, StatusCode::OK, "{latest}");
    assert_eq!(latest["days"][6]["date"], "9999-12-31");

    let account_id = create_account(&app, "Daily overflow", 0).await;
    for _ in 0..2 {
        sqlx::query(
            "INSERT INTO entries (kind, account_id, amount_minor, name, note, occurred_at, created_at, transfer_id)
             VALUES ('expense', ?, ?, 'Overflow day', '', '2026-11-27', '2026-11-27T00:00:00Z', NULL)",
        )
        .bind(account_id)
        .bind(i64::MAX)
        .execute(&pool)
        .await
        .unwrap();
    }
    let (status, body) = daily_report(&app, "2026-11-27").await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        body["error"],
        "daily expense total is outside the supported integer range"
    );
}

#[tokio::test]
async fn balance_sheet_reconciles_signed_groups_opening_rules_transfers_and_visibility() {
    let (app, pool, _directory) = setup().await;
    let checking = create_typed_account(
        &app,
        "Balance checking",
        "checking",
        100_000,
        Some("2026-11-15"),
    )
    .await;
    let savings =
        create_typed_account(&app, "Balance legacy savings", "savings", 50_000, None).await;
    let negative_asset = create_typed_account(
        &app,
        "Balance negative asset",
        "cash",
        0,
        Some("2026-01-01"),
    )
    .await;
    let card_debt = create_typed_account(
        &app,
        "Balance card debt",
        "credit_card",
        0,
        Some("2026-01-01"),
    )
    .await;
    let card_credit = create_typed_account(
        &app,
        "Balance card credit",
        "credit_card",
        0,
        Some("2026-01-01"),
    )
    .await;
    let unclassified = create_account(&app, "Balance unclassified", 7_000).await;
    let picker_hidden =
        create_typed_account(&app, "Balance picker hidden", "checking", 1_000, None).await;
    let report_hidden =
        create_typed_account(&app, "Balance report hidden", "checking", 999_999, None).await;
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

    for (kind, account_id, amount, name, date) in [
        (
            "expense",
            checking,
            1_000,
            "Before known opening",
            "2026-10-20",
        ),
        (
            "expense",
            checking,
            1_000,
            "Current checking expense",
            "2026-11-01",
        ),
        (
            "income",
            savings,
            10_000,
            "Legacy savings income",
            "2026-10-10",
        ),
        (
            "expense",
            savings,
            5_000,
            "Legacy savings expense",
            "2026-11-05",
        ),
        (
            "expense",
            negative_asset,
            5_000,
            "Negative asset",
            "2026-10-05",
        ),
        (
            "expense",
            card_debt,
            20_000,
            "Card prior debt",
            "2026-10-10",
        ),
        (
            "expense",
            card_debt,
            5_000,
            "Card current debt",
            "2026-11-05",
        ),
        (
            "income",
            unclassified,
            2_000,
            "Unclassified income",
            "2026-11-10",
        ),
    ] {
        create_entry_at(&app, kind, account_id, amount, name, None, date).await;
    }
    for (from, to, amount, name, date) in [
        (checking, card_debt, 15_000, "Card payment", "2026-11-10"),
        (
            checking,
            card_credit,
            3_000,
            "Card credit transfer",
            "2026-11-11",
        ),
    ] {
        let (status, body) = send_json(
            &app,
            "POST",
            "/api/transfers",
            json!({
                "from_account_id": from,
                "to_account_id": to,
                "amount_minor": amount,
                "name": name,
                "occurred_at": format!("{date}T12:00:00")
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }
    for (account_id, kind, name) in [
        (checking, "expense", "Invalid visible expense"),
        (unclassified, "income", "Invalid visible income"),
        (report_hidden, "expense", "Invalid hidden expense"),
    ] {
        sqlx::query(
            "INSERT INTO entries (kind, account_id, amount_minor, name, note, occurred_at, created_at, transfer_id)
             VALUES (?, ?, 100, ?, 'legacy invalid', 'not-a-date', '2026-11-01T00:00:00Z', NULL)",
        )
        .bind(kind)
        .bind(account_id)
        .bind(name)
        .execute(&pool)
        .await
        .unwrap();
    }

    let (status, body) = balance_report(&app, "2026-11").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["prior_date"], "2026-10-31");
    assert_eq!(body["current_date"], "2026-11-30");
    assert_eq!(body["invalid_date_entry_count"], 2);
    assert_eq!(body["legacy_opening_account_count"], 3);
    assert_eq!(balance_account(&body, checking)["prior_minor"], -1_000);
    assert_eq!(balance_account(&body, checking)["current_minor"], 80_000);
    assert_eq!(balance_account(&body, savings)["prior_minor"], 60_000);
    assert_eq!(balance_account(&body, savings)["current_minor"], 55_000);
    assert_eq!(
        balance_account(&body, negative_asset)["current_minor"],
        -5_000
    );
    assert_eq!(balance_account(&body, card_debt)["prior_minor"], -20_000);
    assert_eq!(balance_account(&body, card_debt)["current_minor"], -10_000);
    assert_eq!(balance_account(&body, card_credit)["current_minor"], 3_000);
    assert_eq!(balance_account(&body, unclassified)["prior_minor"], 7_000);
    assert_eq!(balance_account(&body, unclassified)["current_minor"], 9_000);
    assert_eq!(
        balance_account(&body, picker_hidden)["current_minor"],
        1_000
    );
    assert!(!body.to_string().contains("Balance report hidden"));

    assert_eq!(
        balance_group(&body, "asset")["prior_subtotal_minor"],
        55_000
    );
    assert_eq!(
        balance_group(&body, "asset")["current_subtotal_minor"],
        131_000
    );
    assert_eq!(
        balance_group(&body, "liability")["prior_subtotal_minor"],
        -20_000
    );
    assert_eq!(
        balance_group(&body, "liability")["current_subtotal_minor"],
        -7_000
    );
    assert_eq!(
        balance_group(&body, "unclassified")["prior_subtotal_minor"],
        7_000
    );
    assert_eq!(
        balance_group(&body, "unclassified")["current_subtotal_minor"],
        9_000
    );
    assert_eq!(body["prior_net_worth_minor"], 42_000);
    assert_eq!(body["current_net_worth_minor"], 133_000);
}

#[tokio::test]
async fn balance_sheet_opening_date_edits_gate_only_the_baseline_not_posted_history() {
    let (app, _pool, _directory) = setup().await;
    let account_id = create_typed_account(
        &app,
        "Opening date correction",
        "checking",
        100_000,
        Some("2026-01-01"),
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        account_id,
        1_000,
        "Posted before metadata correction",
        None,
        "2026-10-20",
    )
    .await;
    let (status, before) = balance_report(&app, "2026-11").await;
    assert_eq!(status, StatusCode::OK, "{before}");
    assert_eq!(balance_account(&before, account_id)["prior_minor"], 99_000);
    assert_eq!(
        balance_account(&before, account_id)["current_minor"],
        99_000
    );

    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/accounts/{account_id}"),
        json!({ "opening_date": "2026-12-01" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, after) = balance_report(&app, "2026-11").await;
    assert_eq!(status, StatusCode::OK, "{after}");
    assert_eq!(balance_account(&after, account_id)["prior_minor"], -1_000);
    assert_eq!(balance_account(&after, account_id)["current_minor"], -1_000);
}

#[tokio::test]
async fn balance_sheet_empty_invalid_month_and_overflow_are_explicit() {
    let (app, _pool, _directory) = setup().await;
    let (status, empty) = balance_report(&app, "2030-05").await;
    assert_eq!(status, StatusCode::OK, "{empty}");
    assert_eq!(empty["groups"].as_array().unwrap().len(), 3);
    assert!(empty["groups"]
        .as_array()
        .unwrap()
        .iter()
        .all(|group| group["accounts"] == json!([])));
    assert_eq!(empty["prior_net_worth_minor"], 0);
    assert_eq!(empty["current_net_worth_minor"], 0);
    for month in ["2026-13", "2026-00", "0001-01", "not-a-month"] {
        let (status, body) = balance_report(&app, month).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{month}: {body}");
    }

    create_account(&app, "Balance overflow one", i64::MAX).await;
    create_account(&app, "Balance overflow two", i64::MAX).await;
    let (status, body) = balance_report(&app, "2026-11").await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(body["error"].as_str().unwrap().contains("integer range"));
}
