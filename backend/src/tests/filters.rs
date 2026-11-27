use super::*;

#[tokio::test]
async fn saved_filters_persist_and_reconciliation_is_filterable() {
    let (app, _pool, _directory) = setup().await;
    let checking = create_account(&app, "Checking", 100_000).await;
    let dining = category_id(&app, "expense", "Dining Out").await;
    let reconciled = create_entry(
        &app,
        "expense",
        checking,
        25_000,
        "Reconciled dinner",
        Some(dining),
    )
    .await;
    create_entry(&app, "expense", checking, 1_000, "Open snack", Some(dining)).await;

    let (status, body) = send_json(
        &app,
        "PATCH",
        &format!("/api/transactions/{reconciled}/reconciliation"),
        json!({ "reconciled": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body) = send_json(
        &app,
        "POST",
        "/api/filters",
        json!({
            "name": "Reconciled dining",
            "definition": {
                "category_id": dining,
                "reconciled": true
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert!(body["id"].as_i64().unwrap() > 0);

    let current = state(&app).await;
    assert_eq!(current["saved_filters"].as_array().unwrap().len(), 1);
    let (status, body) = send_json(
        &app,
        "GET",
        &format!("/api/ledger?category_id={dining}&reconciled=true"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["entries"].as_array().unwrap().len(), 1);
    assert_eq!(body["entries"][0]["id"], reconciled);
}

#[tokio::test]
async fn saved_filter_update_renames_without_replacing_and_explicitly_replaces_criteria() {
    let (app, _pool, _directory) = setup().await;
    let checking = create_account(&app, "Filter update checking", 100_000).await;
    let dining = category_id(&app, "expense", "Dining Out").await;
    let reconciled = create_entry(
        &app,
        "expense",
        checking,
        25_000,
        "Filter reconciled dinner",
        Some(dining),
    )
    .await;
    let open = create_entry(
        &app,
        "expense",
        checking,
        1_000,
        "Filter open snack",
        Some(dining),
    )
    .await;
    let (status, body) = send_json(
        &app,
        "PATCH",
        &format!("/api/transactions/{reconciled}/reconciliation"),
        json!({ "reconciled": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let original_definition = json!({
        "category_id": dining,
        "reconciled": true
    });
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/filters",
        json!({
            "name": "Original saved filter",
            "definition": original_definition
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let filter_id = body["id"].as_i64().unwrap();

    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/filters/{filter_id}"),
        json!({
            "name": "Renamed saved filter",
            "definition": original_definition
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["name"], "Renamed saved filter");
    assert_eq!(body["definition"]["category_id"], dining);
    assert_eq!(body["definition"]["reconciled"], true);
    assert!(body["definition"]["search"].is_null());
    let current = state(&app).await;
    assert_eq!(
        current["saved_filters"][0]["definition"]["category_id"],
        dining
    );
    assert_eq!(
        current["saved_filters"][0]["definition"]["reconciled"],
        true
    );
    assert!(current["saved_filters"][0]["definition"]["search"].is_null());

    let replacement = json!({
        "search": "open",
        "category_id": dining,
        "reconciled": false
    });
    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/filters/{filter_id}"),
        json!({
            "name": "Replacement saved filter",
            "definition": replacement
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["definition"]["search"], "open");
    assert_eq!(body["definition"]["category_id"], dining);
    assert_eq!(body["definition"]["reconciled"], false);
    let (status, body) = send_json(
        &app,
        "GET",
        "/api/ledger?search=open&reconciled=false",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["entries"].as_array().unwrap().len(), 1);
    assert_eq!(body["entries"][0]["id"], open);
    let (status, _, _, csv) =
        send_csv(&app, "/api/ledger/export?search=open&reconciled=false").await;
    assert_eq!(status, StatusCode::OK);
    let csv = String::from_utf8(csv).unwrap();
    assert_eq!(csv.lines().count(), 2);
    assert!(csv.contains("Filter open snack"));
    assert!(!csv.contains("Filter reconciled dinner"));

    for invalid in [
        json!({"name": "", "definition": replacement}),
        json!({"name": "Invalid date", "definition": {"date_from": "2026-02-30"}}),
        json!({"name": "Invalid amount", "definition": {"amount_min_minor": -1}}),
    ] {
        let (status, body) =
            send_json(&app, "PUT", &format!("/api/filters/{filter_id}"), invalid).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
    let current = state(&app).await;
    assert_eq!(
        current["saved_filters"][0]["name"],
        "Replacement saved filter"
    );
    assert_eq!(current["saved_filters"][0]["definition"]["search"], "open");
    assert_eq!(
        current["saved_filters"][0]["definition"]["category_id"],
        dining
    );
    assert_eq!(
        current["saved_filters"][0]["definition"]["reconciled"],
        false
    );
    let (status, body) = send_json(
        &app,
        "PUT",
        "/api/filters/999999",
        json!({"name": "Missing", "definition": {}}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

#[tokio::test]
async fn deleting_a_saved_filter_never_deletes_transactions_or_changes_balances() {
    let (app, _pool, _directory) = setup().await;
    let checking = create_account(&app, "Filter delete checking", 100_000).await;
    let entry_id = create_entry(
        &app,
        "expense",
        checking,
        2_500,
        "Filter deletion survivor",
        None,
    )
    .await;
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/filters",
        json!({"name": "Disposable definition", "definition": {"search": "survivor"}}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let filter_id = body["id"].as_i64().unwrap();
    let before = state(&app).await;
    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/filters/{filter_id}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let after = state(&app).await;
    assert_eq!(after["saved_filters"], json!([]));
    assert_eq!(after["entries"], before["entries"]);
    assert_eq!(after["accounts"], before["accounts"]);
    assert_eq!(after["totals"], before["totals"]);
    assert!(after["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["id"] == entry_id));
}
