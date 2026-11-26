use super::*;

#[tokio::test]
async fn category_crud_prevents_cycles_and_deletes_unreferenced_categories() {
    let (app, _pool, _directory) = setup().await;
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/categories",
        json!({
            "name": "Pets",
            "kind": "expense",
            "parent_id": null,
            "color": "#426b8a",
            "icon": "🐾",
            "sort_order": 110
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let parent = body["id"].as_i64().unwrap();
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/categories",
        json!({
            "name": "Vet",
            "kind": "expense",
            "parent_id": parent,
            "color": "#426b8a",
            "icon": "🩺",
            "sort_order": 111
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let child = body["id"].as_i64().unwrap();

    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/categories/{parent}"),
        json!({
            "name": "Pets",
            "kind": "expense",
            "parent_id": child,
            "color": "#426b8a",
            "icon": "🐾",
            "sort_order": 110
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/categories/{child}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["archived"], false);
    let current = state(&app).await;
    assert!(!current["categories"]
        .as_array()
        .unwrap()
        .iter()
        .any(|category| category["id"] == child));
}

#[tokio::test]
async fn category_hierarchy_stays_two_levels_and_moves_preserve_assignments() {
    let (app, _pool, _directory) = setup().await;
    let food = category_id(&app, "expense", "Food & Drink").await;
    let shopping = category_id(&app, "expense", "Shopping").await;
    let seeded_dining = category_id(&app, "expense", "Dining Out").await;

    // Recreate the exact Food → Dining Out branch through the API.
    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/categories/{seeded_dining}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["archived"], false);
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/categories",
        json!({
            "name": "Dining Out",
            "kind": "expense",
            "parent_id": food,
            "color": "#b65d35",
            "icon": "🍴",
            "sort_order": 11
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let dining = body["id"].as_i64().unwrap();
    assert_eq!(body["parent_id"], food);

    // A third level is rejected.
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/categories",
        json!({
            "name": "Invalid third level",
            "kind": "expense",
            "parent_id": dining,
            "color": "#b65d35",
            "icon": "×",
            "sort_order": 12
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        body["error"],
        "subcategories can only be created under top-level categories"
    );

    // Food already has subcategories, so it cannot be moved beneath Shopping.
    let before = state(&app).await;
    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/categories/{food}"),
        json!({
            "name": "Food & Drink",
            "kind": "expense",
            "parent_id": shopping,
            "color": "#b65d35",
            "icon": "🍽",
            "sort_order": 10
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"], "Move this category’s subcategories first.");
    let after = state(&app).await;
    assert_eq!(after["categories"], before["categories"]);

    // Dining Out has no children, so it can move under another valid top-level category.
    let checking = create_account(&app, "Checking", 100_000).await;
    let expense = create_entry(&app, "expense", checking, 25_000, "Dinner", Some(dining)).await;
    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/categories/{dining}"),
        json!({
            "name": "Dining Out",
            "kind": "expense",
            "parent_id": shopping,
            "color": "#b65d35",
            "icon": "🍴",
            "sort_order": 11
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["parent_id"], shopping);
    let current = state(&app).await;
    assert_eq!(current["entries"][0]["id"], expense);
    assert_eq!(current["entries"][0]["category_id"], dining);
    assert_eq!(current["entries"][0]["category_name"], "Dining Out");
    assert!(current["category_totals"]
        .as_array()
        .unwrap()
        .iter()
        .any(|total| total["category_id"] == dining && total["total_minor"] == 25_000));

    // A childless category can also become top-level.
    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/categories/{dining}"),
        json!({
            "name": "Dining Out",
            "kind": "expense",
            "parent_id": null,
            "color": "#b65d35",
            "icon": "🍴",
            "sort_order": 11
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["parent_id"], Value::Null);
}

#[tokio::test]
async fn editing_an_entry_with_an_archived_category_preserves_the_assignment() {
    let (app, _pool, _directory) = setup().await;
    let checking = create_account(&app, "Checking", 100_000).await;
    let dining = category_id(&app, "expense", "Dining Out").await;
    let expense = create_entry(&app, "expense", checking, 25_000, "Dinner", Some(dining)).await;
    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/categories/{dining}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/transactions/{expense}"),
        json!({
            "kind": "expense",
            "account_id": checking,
            "amount_minor": 30_000,
            "name": "Dinner updated",
            "category_id": dining
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let current = state(&app).await;
    assert_eq!(current["entries"][0]["category_id"], dining);
    assert_eq!(current["entries"][0]["category_name"], "Dining Out");
}

#[tokio::test]
async fn five_concurrent_subcategory_creates_do_not_starve_the_connection_pool() {
    let (app, pool, _directory) = setup().await;
    let food = category_id(&app, "expense", "Food & Drink").await;
    let store = Store::new(pool);
    let create = |name: &str| {
        store.create_category(CategoryRequest {
            name: name.to_string(),
            kind: "expense".to_string(),
            parent_id: Some(food),
            color: "#426b8a".to_string(),
            icon: "•".to_string(),
            sort_order: 900,
        })
    };
    let (first, second, third, fourth, fifth) = tokio::join!(
        create("Concurrent subcategory one"),
        create("Concurrent subcategory two"),
        create("Concurrent subcategory three"),
        create("Concurrent subcategory four"),
        create("Concurrent subcategory five"),
    );

    let mut ids = HashSet::new();
    for category in [first, second, third, fourth, fifth] {
        let category = category.expect("concurrent category creation should succeed");
        assert_eq!(category.parent_id, Some(food));
        assert!(ids.insert(category.id));
    }
    assert_eq!(ids.len(), 5);
}

#[tokio::test]
async fn referenced_category_archives_and_preserves_history() {
    let (app, _pool, _directory) = setup().await;
    let checking = create_account(&app, "Checking", 100_000).await;
    let dining = category_id(&app, "expense", "Dining Out").await;
    create_entry(&app, "expense", checking, 25_000, "Dinner", Some(dining)).await;

    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/categories/{dining}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["archived"], true);

    let current = state(&app).await;
    let category = current["categories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|category| category["id"] == dining)
        .unwrap();
    assert!(!category["archived_at"].is_null());
    assert!(current["category_totals"]
        .as_array()
        .unwrap()
        .iter()
        .any(|total| total["category_id"] == dining && total["total_minor"] == 25_000));
    assert!(current["entries"][0]["category_name"] == "Dining Out");

    let (status, body) = send_json(
        &app,
        "POST",
        "/api/transactions",
        json!({
            "kind": "expense",
            "account_id": checking,
            "amount_minor": 1_000,
            "name": "New dinner",
            "category_id": dining
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn starter_catalog_supports_dining_and_salary() {
    let (app, _pool, _directory) = setup().await;
    let dining = category_id(&app, "expense", "Dining Out").await;
    let salary = category_id(&app, "income", "Salary").await;
    assert!(dining > 0);
    assert!(salary > 0);
    let current = state(&app).await;
    assert!(current["categories"].as_array().unwrap().len() >= 50);
}
