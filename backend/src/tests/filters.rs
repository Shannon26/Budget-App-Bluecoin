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
