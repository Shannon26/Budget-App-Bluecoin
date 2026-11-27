use super::*;

async fn set_overall_budget(app: &Router, month: &str, amount_minor: i64) -> (StatusCode, Value) {
    send_json(
        app,
        "PUT",
        &format!("/api/overall-budget?month={month}"),
        json!({ "amount_minor": amount_minor }),
    )
    .await
}

async fn remove_overall_budget(app: &Router, month: &str) -> (StatusCode, Value) {
    send_json(
        app,
        "DELETE",
        &format!("/api/overall-budget?month={month}"),
        json!({}),
    )
    .await
}

fn overall_budget(body: &Value) -> &Value {
    &body["overall_budget"]
}

#[tokio::test]
async fn category_budget_limits_enforce_parent_child_and_category_move_rules() {
    let (app, _pool, _directory) = setup().await;
    let food = category_id(&app, "expense", "Food & Drink").await;
    let dining = category_id(&app, "expense", "Dining Out").await;
    let shopping = category_id(&app, "expense", "Shopping").await;

    let (status, body) = update_category_shape(&app, dining, None, Some(Some(food))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = set_budget(&app, food, "2026-11", 500_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body) = set_budget(&app, dining, "2026-11", 600_000).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        body["error"],
        "subcategory budget cannot exceed its parent category budget for the same month"
    );
    assert_eq!(
        budget_row(&monthly_budgets(&app, "2026-11").await, dining)["limit_minor"],
        Value::Null
    );

    let (status, body) = set_budget(&app, dining, "2026-11", 150_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = set_budget(&app, food, "2026-11", 50_000).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        body["error"],
        "parent category budget cannot be lower than a subcategory budget for the same month"
    );
    assert_eq!(
        budget_row(&monthly_budgets(&app, "2026-11").await, food)["limit_minor"],
        500_000
    );

    let (status, body) = set_budget(&app, food, "2026-11", 150_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = set_budget(&app, dining, "2026-11", 150_001).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        budget_row(&monthly_budgets(&app, "2026-11").await, dining)["limit_minor"],
        150_000
    );

    let (status, body) = update_category_shape(&app, food, Some("income"), Some(None)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        body["error"],
        "category type cannot change while it has saved budgets"
    );

    let (status, body) = set_budget(&app, shopping, "2026-11", 50_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = update_category_shape(&app, dining, None, Some(Some(shopping))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        body["error"],
        "subcategory budget cannot exceed its new parent category budget for the same month"
    );
    assert_eq!(
        budget_row(&monthly_budgets(&app, "2026-11").await, dining)["parent_id"],
        food
    );

    let (status, body) = set_budget(&app, shopping, "2026-11", 200_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = update_category_shape(&app, dining, None, Some(Some(shopping))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        budget_row(&monthly_budgets(&app, "2026-11").await, dining)["parent_id"],
        shopping
    );
    let (status, body) = update_category_shape(&app, dining, None, Some(None)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        budget_row(&monthly_budgets(&app, "2026-11").await, dining)["parent_id"],
        Value::Null
    );
}

#[tokio::test]
async fn category_budgets_use_calendar_boundaries_and_recalculate_after_entry_changes() {
    let (app, _pool, _directory) = setup().await;
    let checking = create_account(&app, "Boundary checking", 1_000_000).await;
    let shopping = category_id(&app, "expense", "Shopping").await;
    let (status, body) = set_budget(&app, shopping, "2026-11", 100_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let october = create_entry_at(
        &app,
        "expense",
        checking,
        10_000,
        "October boundary",
        Some(shopping),
        "2026-10-31T23:59:59",
    )
    .await;
    let november_first = create_entry_at(
        &app,
        "expense",
        checking,
        20_000,
        "November first",
        Some(shopping),
        "2026-11-01T00:00:00",
    )
    .await;
    let november_last = create_entry_at(
        &app,
        "expense",
        checking,
        30_000,
        "November last",
        Some(shopping),
        "2026-11-30T23:59:59",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        checking,
        40_000,
        "December boundary",
        Some(shopping),
        "2026-12-01T00:00:00",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        checking,
        5_000,
        "Leap day",
        Some(shopping),
        "2024-02-29T23:59:59",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        checking,
        6_000,
        "After leap day",
        Some(shopping),
        "2024-03-01T00:00:00",
    )
    .await;

    assert_eq!(
        monthly_budgets(&app, "2026-10").await["total_spent_minor"],
        10_000
    );
    let november = monthly_budgets(&app, "2026-11").await;
    assert_eq!(november["total_spent_minor"], 50_000);
    assert_eq!(budget_row(&november, shopping)["spent_minor"], 50_000);
    assert_eq!(
        monthly_budgets(&app, "2026-12").await["total_spent_minor"],
        40_000
    );
    assert_eq!(
        monthly_budgets(&app, "2024-02").await["total_spent_minor"],
        5_000
    );
    assert_eq!(
        monthly_budgets(&app, "2024-03").await["total_spent_minor"],
        6_000
    );

    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/transactions/{november_first}"),
        json!({
            "kind": "expense",
            "account_id": checking,
            "amount_minor": 25_000,
            "name": "November first edited",
            "category_id": shopping,
            "occurred_at": "2026-11-01T00:00:00"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        monthly_budgets(&app, "2026-11").await["total_spent_minor"],
        55_000
    );

    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/transactions/{november_last}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        monthly_budgets(&app, "2026-11").await["total_spent_minor"],
        25_000
    );
    assert_eq!(
        monthly_budgets(&app, "2026-10").await["total_spent_minor"],
        10_000
    );

    // Keep the fixture IDs referenced so accidental boundary omissions are explicit.
    assert_ne!(october, november_first);
}

#[tokio::test]
async fn category_budgets_validate_inputs_and_keep_archived_history_visible() {
    let (app, pool, _directory) = setup().await;
    let checking = create_account(&app, "Archive budget checking", 100_000).await;
    let shopping = category_id(&app, "expense", "Shopping").await;
    let salary = category_id(&app, "income", "Salary").await;

    for month in ["2026-13", "2026-00", "2026-1", "=2026-11", "+2026-11"] {
        let (status, body) = send_json(
            &app,
            "GET",
            &format!("/api/category-budgets?month={month}"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        let (status, body) = set_budget(&app, shopping, month, 100_000).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
    for amount in [0, -1] {
        let (status, body) = set_budget(&app, shopping, "2026-11", amount).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
    let (status, body) = set_budget(&app, salary, "2026-11", 100_000).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        body["error"],
        "budgets can only be set for expense categories"
    );
    let (status, body) = set_budget(&app, 999_999, "2026-11", 100_000).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let budget_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM category_budgets")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(budget_count, 0);

    let (status, body) = set_budget(&app, shopping, "2026-11", 100_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    create_entry_at(
        &app,
        "expense",
        checking,
        12_345,
        "Archived budget expense",
        Some(shopping),
        "2026-11-20T12:00:00",
    )
    .await;
    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/categories/{shopping}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["archived"], true);

    let november = monthly_budgets(&app, "2026-11").await;
    assert!(budget_row(&november, shopping)["archived_at"].is_string());
    assert_eq!(budget_row(&november, shopping)["limit_minor"], 100_000);
    assert_eq!(budget_row(&november, shopping)["spent_minor"], 12_345);
    let (status, body) = set_budget(&app, shopping, "2026-12", 100_000).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, body) = set_budget(&app, shopping, "2026-11", 150_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(budget_row(&body, shopping)["limit_minor"], 150_000);
    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/category-budgets/{shopping}?month=2026-11"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(budget_row(&body, shopping)["limit_minor"], Value::Null);

    let (status, body) = send_json(
        &app,
        "POST",
        "/api/categories",
        json!({
            "name": "Temporary budget category",
            "kind": "expense",
            "parent_id": null,
            "color": "#426b8a",
            "icon": "•",
            "sort_order": 999
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let temporary = body["id"].as_i64().unwrap();
    let (status, body) = set_budget(&app, temporary, "2026-11", 100_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/categories/{temporary}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["archived"], true);
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM category_budgets WHERE category_id = ?")
            .bind(temporary)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(remaining, 1);

    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/category-budgets/{temporary}?month=2026-11"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/categories/{temporary}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["archived"], false);
}

#[tokio::test]
async fn concurrent_category_move_and_parent_budget_lower_cannot_leave_an_invalid_hierarchy() {
    let (app, pool, _directory) = setup().await;
    let dining = category_id(&app, "expense", "Dining Out").await;
    let shopping = category_id(&app, "expense", "Shopping").await;
    let current = state(&app).await;
    let dining_category = current["categories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|category| category["id"] == dining)
        .unwrap()
        .clone();
    let original_parent_id = dining_category["parent_id"].as_i64();
    let move_request = CategoryRequest {
        name: dining_category["name"].as_str().unwrap().to_string(),
        kind: dining_category["kind"].as_str().unwrap().to_string(),
        parent_id: Some(shopping),
        color: dining_category["color"].as_str().unwrap().to_string(),
        icon: dining_category["icon"].as_str().unwrap().to_string(),
        sort_order: dining_category["sort_order"].as_i64().unwrap(),
    };
    let (status, body) = set_budget(&app, shopping, "2026-11", 100_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = set_budget(&app, dining, "2026-11", 80_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let store = Store::new(pool);
    let (move_result, budget_result) = tokio::join!(
        store.update_category(dining, move_request),
        store.set_category_budget(shopping, "2026-11", 40_000),
    );
    let move_succeeded = move_result.is_ok();
    let budget_succeeded = budget_result.is_ok();
    assert_ne!(move_succeeded, budget_succeeded);

    if let Err(error) = move_result {
        match error {
            AppError::BadRequest(message) => assert_eq!(
                message,
                "subcategory budget cannot exceed its new parent category budget for the same month"
            ),
            other => panic!("unexpected category move error: {other:?}"),
        }
    }
    if let Err(error) = budget_result {
        match error {
            AppError::BadRequest(message) => assert_eq!(
                message,
                "parent category budget cannot be lower than a subcategory budget for the same month"
            ),
            other => panic!("unexpected parent budget error: {other:?}"),
        }
    }

    let final_categories = store.categories().await.unwrap();
    let final_parent_id = final_categories
        .iter()
        .find(|category| category.id == dining)
        .unwrap()
        .parent_id;
    let final_budgets = store.monthly_category_budgets("2026-11").await.unwrap();
    let parent_limit = final_budgets
        .categories
        .iter()
        .find(|category| category.category_id == shopping)
        .unwrap()
        .limit_minor
        .unwrap();
    let child_limit = final_budgets
        .categories
        .iter()
        .find(|category| category.category_id == dining)
        .unwrap()
        .limit_minor
        .unwrap();
    assert_eq!(child_limit, 80_000);
    if move_succeeded {
        assert_eq!(final_parent_id, Some(shopping));
        assert_eq!(parent_limit, 100_000);
        assert!(child_limit <= parent_limit);
    } else {
        assert_eq!(final_parent_id, original_parent_id);
        assert_eq!(parent_limit, 40_000);
    }
}

#[tokio::test]
async fn concurrent_parent_and_child_budget_saves_cannot_leave_an_invalid_hierarchy() {
    let (app, pool, _directory) = setup().await;
    let food = category_id(&app, "expense", "Food & Drink").await;
    let dining = category_id(&app, "expense", "Dining Out").await;
    let (status, body) = update_category_shape(&app, dining, None, Some(Some(food))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = set_budget(&app, food, "2026-11", 100_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = set_budget(&app, dining, "2026-11", 20_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let store = Store::new(pool);
    let (parent_result, child_result) = tokio::join!(
        store.set_category_budget(food, "2026-11", 40_000),
        store.set_category_budget(dining, "2026-11", 80_000),
    );
    let parent_succeeded = parent_result.is_ok();
    let child_succeeded = child_result.is_ok();
    assert_ne!(parent_succeeded, child_succeeded);

    if let Err(error) = parent_result {
        match error {
            AppError::BadRequest(message) => assert_eq!(
                message,
                "parent category budget cannot be lower than a subcategory budget for the same month"
            ),
            other => panic!("unexpected parent save error: {other:?}"),
        }
    }
    if let Err(error) = child_result {
        match error {
            AppError::BadRequest(message) => assert_eq!(
                message,
                "subcategory budget cannot exceed its parent category budget for the same month"
            ),
            other => panic!("unexpected child save error: {other:?}"),
        }
    }

    let final_budgets = store.monthly_category_budgets("2026-11").await.unwrap();
    let parent_limit = final_budgets
        .categories
        .iter()
        .find(|category| category.category_id == food)
        .unwrap()
        .limit_minor
        .unwrap();
    let child_limit = final_budgets
        .categories
        .iter()
        .find(|category| category.category_id == dining)
        .unwrap()
        .limit_minor
        .unwrap();
    assert!(child_limit <= parent_limit);
    if parent_succeeded {
        assert_eq!(parent_limit, 40_000);
        assert_eq!(child_limit, 20_000);
    } else {
        assert_eq!(parent_limit, 100_000);
        assert_eq!(child_limit, 80_000);
    }
}

#[tokio::test]
async fn monthly_budgets_flag_invalid_date_expenses_without_counting_their_month() {
    let (app, pool, _directory) = setup().await;
    let checking = create_account(&app, "Invalid date budget checking", 100_000).await;
    let shopping = category_id(&app, "expense", "Shopping").await;
    create_entry_at(
        &app,
        "expense",
        checking,
        1_000,
        "Valid monthly expense",
        Some(shopping),
        "2026-11-10T12:00:00",
    )
    .await;
    for (name, invalid) in [
        ("Formula invalid date", "=2026-11-10"),
        ("Impossible invalid date", "2026-02-30T12:00:00"),
    ] {
        create_entry_at(
            &app,
            "expense",
            checking,
            500,
            name,
            Some(shopping),
            "2026-11-11T12:00:00",
        )
        .await;
        sqlx::query("UPDATE entries SET occurred_at = ? WHERE name = ?")
            .bind(invalid)
            .bind(name)
            .execute(&pool)
            .await
            .unwrap();
    }

    let november = monthly_budgets(&app, "2026-11").await;
    assert_eq!(november["total_spent_minor"], 1_000);
    assert_eq!(november["expense_count"], 1);
    assert_eq!(november["invalid_date_expense_count"], 2);
    assert_eq!(november["invalid_date_spent_minor"], 1_000);
    assert_eq!(budget_row(&november, shopping)["spent_minor"], 1_000);
    assert_eq!(state(&app).await["totals"]["expense_minor"], 2_000);
}

#[tokio::test]
async fn monthly_category_budgets_roll_up_children_without_double_counting_overall_spend() {
    let (app, pool, _directory) = setup().await;
    let checking = create_account(&app, "Budget checking", 1_000_000).await;
    let savings = create_account(&app, "Budget savings", 100_000).await;
    let food = category_id(&app, "expense", "Food & Drink").await;
    let groceries = category_id(&app, "expense", "Groceries").await;
    let dining = category_id(&app, "expense", "Dining Out").await;
    // This isolated fixture uses the requested Food & Drink → Dining Out example.
    sqlx::query("UPDATE categories SET parent_id = ? WHERE id = ?")
        .bind(food)
        .bind(dining)
        .execute(&pool)
        .await
        .unwrap();

    let (status, body) = set_budget(&app, food, "2026-11", 500_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = set_budget(&app, dining, "2026-11", 150_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = set_budget(&app, dining, "2026-12", 200_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    create_entry_at(
        &app,
        "expense",
        checking,
        100_000,
        "Budget groceries",
        Some(groceries),
        "2026-11-05T12:00:00",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        checking,
        180_000,
        "Budget dining",
        Some(dining),
        "2026-11-06T12:00:00",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        checking,
        30_000,
        "Budget uncategorized",
        None,
        "2026-11-07T12:00:00",
    )
    .await;
    create_entry_at(
        &app,
        "income",
        checking,
        500_000,
        "Budget income does not spend",
        None,
        "2026-11-08T12:00:00",
    )
    .await;
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/transfers",
        json!({
            "from_account_id": checking,
            "to_account_id": savings,
            "amount_minor": 100_000,
            "name": "Budget transfer does not spend",
            "occurred_at": "2026-11-09T12:00:00"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let november = monthly_budgets(&app, "2026-11").await;
    assert_eq!(november["total_spent_minor"], 310_000);
    assert_eq!(november["uncategorized_spent_minor"], 30_000);
    assert_eq!(november["expense_count"], 3);
    assert_eq!(budget_row(&november, food)["limit_minor"], 500_000);
    assert_eq!(budget_row(&november, food)["direct_spent_minor"], 0);
    assert_eq!(budget_row(&november, food)["spent_minor"], 280_000);
    assert_eq!(budget_row(&november, food)["remaining_minor"], 220_000);
    assert_eq!(budget_row(&november, groceries)["spent_minor"], 100_000);
    assert_eq!(budget_row(&november, dining)["limit_minor"], 150_000);
    assert_eq!(budget_row(&november, dining)["spent_minor"], 180_000);
    assert_eq!(budget_row(&november, dining)["remaining_minor"], -30_000);

    let december = monthly_budgets(&app, "2026-12").await;
    assert_eq!(december["total_spent_minor"], 0);
    assert_eq!(budget_row(&december, dining)["limit_minor"], 200_000);
    assert_eq!(budget_row(&november, dining)["limit_minor"], 150_000);

    create_entry_at(
        &app,
        "expense",
        checking,
        20_000,
        "Direct parent budget expense",
        Some(food),
        "2026-12-15T12:00:00",
    )
    .await;
    let december = monthly_budgets(&app, "2026-12").await;
    assert_eq!(december["total_spent_minor"], 20_000);
    assert_eq!(budget_row(&december, food)["direct_spent_minor"], 20_000);
    assert_eq!(budget_row(&december, food)["spent_minor"], 20_000);
    assert_eq!(budget_row(&december, dining)["spent_minor"], 0);

    // A fresh read after unrelated state reload keeps the persisted monthly limits.
    state(&app).await;
    let reloaded = monthly_budgets(&app, "2026-11").await;
    assert_eq!(budget_row(&reloaded, food)["limit_minor"], 500_000);
    assert_eq!(budget_row(&reloaded, dining)["limit_minor"], 150_000);

    let (status, body) = set_budget(&app, food, "2026-11", 400_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(budget_row(&body, food)["remaining_minor"], 120_000);
    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/category-budgets/{dining}?month=2026-11"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(budget_row(&body, dining)["limit_minor"], Value::Null);
    assert_eq!(budget_row(&body, dining)["spent_minor"], 180_000);
}

#[tokio::test]
async fn overall_budget_uses_raw_monthly_expenses_and_stays_independent_of_category_limits() {
    let (app, _pool, _directory) = setup().await;
    let checking = create_account(&app, "Overall budget checking", 1_000_000).await;
    let savings = create_account(&app, "Overall budget savings", 0).await;
    let report_hidden = create_account(&app, "Overall budget report hidden", 0).await;
    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/accounts/{report_hidden}"),
        json!({ "hide_from_reports": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let food = category_id(&app, "expense", "Food & Drink").await;
    let groceries = category_id(&app, "expense", "Groceries").await;
    let dining = category_id(&app, "expense", "Dining Out").await;
    let (status, body) = update_category_shape(&app, dining, None, Some(Some(food))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = set_budget(&app, food, "2026-11", 500_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = set_budget(&app, dining, "2026-11", 150_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = set_overall_budget(&app, "2026-11", 250_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(overall_budget(&body)["spent_minor"], 0);
    assert_eq!(overall_budget(&body)["remaining_minor"], 250_000);

    let groceries_entry = create_entry_at(
        &app,
        "expense",
        checking,
        100_000,
        "Overall groceries",
        Some(groceries),
        "2026-11-05T12:00:00",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        checking,
        180_000,
        "Overall dining",
        Some(dining),
        "2026-11-06T12:00:00",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        checking,
        30_000,
        "Overall uncategorized",
        None,
        "2026-11-07T12:00:00",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        report_hidden,
        5_000,
        "Overall report-hidden expense",
        Some(groceries),
        "2026-11-08T12:00:00",
    )
    .await;
    create_entry_at(
        &app,
        "income",
        checking,
        500_000,
        "Overall income excluded",
        None,
        "2026-11-09T12:00:00",
    )
    .await;
    let (status, body) = transfer(&app, checking, savings, 100_000).await;
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

    let november = monthly_budgets(&app, "2026-11").await;
    assert_eq!(november["total_spent_minor"], 315_000);
    assert_eq!(november["uncategorized_spent_minor"], 30_000);
    assert_eq!(november["expense_count"], 4);
    assert_eq!(overall_budget(&november)["limit_minor"], 250_000);
    assert_eq!(overall_budget(&november)["spent_minor"], 315_000);
    assert_eq!(overall_budget(&november)["remaining_minor"], -65_000);
    assert_eq!(budget_row(&november, food)["spent_minor"], 285_000);
    assert!(budget_row(&november, dining)["archived_at"].is_string());
    assert_eq!(budget_row(&november, dining)["spent_minor"], 180_000);
    assert_eq!(budget_row(&november, dining)["limit_minor"], 150_000);

    let (status, body) = set_overall_budget(&app, "2026-11", 400_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(overall_budget(&body)["remaining_minor"], 85_000);
    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/transactions/{groceries_entry}"),
        json!({
            "kind": "expense",
            "account_id": checking,
            "amount_minor": 120_000,
            "name": "Overall groceries edited",
            "category_id": groceries,
            "occurred_at": "2026-11-05T12:00:00"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let edited = monthly_budgets(&app, "2026-11").await;
    assert_eq!(overall_budget(&edited)["spent_minor"], 335_000);
    assert_eq!(overall_budget(&edited)["remaining_minor"], 65_000);

    let (status, body) = remove_overall_budget(&app, "2026-11").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(overall_budget(&body)["limit_minor"], Value::Null);
    assert_eq!(overall_budget(&body)["remaining_minor"], Value::Null);
    assert_eq!(budget_row(&body, food)["limit_minor"], 500_000);
    assert_eq!(budget_row(&body, dining)["limit_minor"], 150_000);

    let (status, body) = set_overall_budget(&app, "2026-12", 150_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(overall_budget(&body)["spent_minor"], 0);
    assert_eq!(overall_budget(&body)["remaining_minor"], 150_000);
    assert_eq!(
        overall_budget(&monthly_budgets(&app, "2026-11").await)["limit_minor"],
        Value::Null
    );
}

#[tokio::test]
async fn overall_budget_validates_inputs_and_rolls_back_overflowing_summary_mutations() {
    let (app, pool, _directory) = setup().await;
    let checking = create_account(&app, "Overall budget validation", 0).await;
    for month in ["2026-13", "2026-00", "2026-1", "=2026-11", "+2026-11"] {
        let (status, body) = send_json(
            &app,
            "GET",
            &format!("/api/category-budgets?month={month}"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        let (status, body) = set_overall_budget(&app, month, 100_000).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{month}: {body}");
        let (status, body) = remove_overall_budget(&app, month).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{month}: {body}");
    }
    for amount in [0, -1] {
        let (status, body) = set_overall_budget(&app, "2026-11", amount).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"], "amount must be greater than zero");
    }
    let cap_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM overall_budgets")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cap_count, 0);

    let (status, body) = set_overall_budget(&app, "2026-11", 100_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let original = sqlx::query_as::<_, (i64, String)>(
        "SELECT amount_minor, updated_at FROM overall_budgets WHERE month = '2026-11'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    for (name, amount) in [
        ("Overall overflow maximum", i64::MAX),
        ("Overall overflow extra", 1),
    ] {
        create_entry_at(
            &app,
            "expense",
            checking,
            amount,
            name,
            None,
            "2026-11-20T12:00:00",
        )
        .await;
    }

    let (status, body) = set_overall_budget(&app, "2026-11", 200_000).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        body["error"],
        "monthly expense total is outside the supported integer range"
    );
    let after_save = sqlx::query_as::<_, (i64, String)>(
        "SELECT amount_minor, updated_at FROM overall_budgets WHERE month = '2026-11'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after_save, original);

    let (status, body) = remove_overall_budget(&app, "2026-11").await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        body["error"],
        "monthly expense total is outside the supported integer range"
    );
    let after_remove = sqlx::query_as::<_, (i64, String)>(
        "SELECT amount_minor, updated_at FROM overall_budgets WHERE month = '2026-11'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after_remove, original);

    let (status, body) = set_overall_budget(&app, "2030-01", i64::MAX).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(overall_budget(&body)["limit_minor"], i64::MAX);
    assert_eq!(overall_budget(&body)["remaining_minor"], i64::MAX);
}

#[tokio::test]
async fn overall_budget_migration_preserves_legacy_budgets_and_entries_without_seeding_a_cap() {
    let (app, pool, _directory) = setup().await;
    let checking = create_account(&app, "Pre overall budget checking", 0).await;
    let shopping = category_id(&app, "expense", "Shopping").await;
    let (status, body) = set_budget(&app, shopping, "2026-11", 50_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    create_entry_at(
        &app,
        "expense",
        checking,
        12_345,
        "Pre overall budget expense",
        Some(shopping),
        "2026-11-10T12:00:00",
    )
    .await;

    sqlx::query("DROP TABLE overall_budgets")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../../migrations/009_overall_budget.sql"))
        .execute(&pool)
        .await
        .unwrap();
    let app = build_router(pool.clone());

    let migrated = monthly_budgets(&app, "2026-11").await;
    assert_eq!(migrated["total_spent_minor"], 12_345);
    assert_eq!(overall_budget(&migrated)["limit_minor"], Value::Null);
    assert_eq!(overall_budget(&migrated)["remaining_minor"], Value::Null);
    assert_eq!(budget_row(&migrated, shopping)["limit_minor"], 50_000);
    assert_eq!(budget_row(&migrated, shopping)["spent_minor"], 12_345);

    let (status, body) = set_overall_budget(&app, "2026-11", 20_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(overall_budget(&body)["limit_minor"], 20_000);
    assert_eq!(overall_budget(&body)["remaining_minor"], 7_655);
    assert_eq!(state(&app).await["totals"]["expense_minor"], 12_345);
}

#[tokio::test]
async fn overflowing_budget_snapshots_roll_back_save_and_remove() {
    let (app, pool, _directory) = setup().await;
    let checking = create_account(&app, "Atomic overflow checking", 0).await;
    let shopping = category_id(&app, "expense", "Shopping").await;
    let (status, body) = set_budget(&app, shopping, "2026-11", 100_000).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let original = sqlx::query_as::<_, (i64, String)>(
        "SELECT amount_minor, updated_at FROM category_budgets
         WHERE category_id = ? AND month = '2026-11'",
    )
    .bind(shopping)
    .fetch_one(&pool)
    .await
    .unwrap();
    for (name, amount) in [
        ("Atomic overflow maximum", i64::MAX),
        ("Atomic overflow extra", 1),
    ] {
        create_entry_at(
            &app,
            "expense",
            checking,
            amount,
            name,
            None,
            "2026-11-20T12:00:00",
        )
        .await;
    }

    let (status, body) = set_budget(&app, shopping, "2026-11", 200_000).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        body["error"],
        "monthly expense total is outside the supported integer range"
    );
    let after_save = sqlx::query_as::<_, (i64, String)>(
        "SELECT amount_minor, updated_at FROM category_budgets
         WHERE category_id = ? AND month = '2026-11'",
    )
    .bind(shopping)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after_save, original);

    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/category-budgets/{shopping}?month=2026-11"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        body["error"],
        "monthly expense total is outside the supported integer range"
    );
    let after_remove = sqlx::query_as::<_, (i64, String)>(
        "SELECT amount_minor, updated_at FROM category_budgets
         WHERE category_id = ? AND month = '2026-11'",
    )
    .bind(shopping)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after_remove, original);
}
