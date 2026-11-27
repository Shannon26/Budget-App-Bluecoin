use super::*;

async fn create_label(app: &Router, name: &str) -> i64 {
    let (status, body) = send_json(app, "POST", "/api/labels", json!({ "name": name })).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_i64().unwrap()
}

#[tokio::test]
async fn labels_are_validated_reusable_renameable_and_legacy_entries_stay_unlabelled() {
    let (app, _pool, _directory) = setup().await;
    let (status, body) = send_json(&app, "GET", "/api/labels", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!([]));
    for name in ["", "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"] {
        let (status, body) = send_json(&app, "POST", "/api/labels", json!({ "name": name })).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }

    let work = create_label(&app, "Work").await;
    let (status, body) = send_json(&app, "POST", "/api/labels", json!({ "name": " work " })).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["id"], work);
    let personal = create_label(&app, "Personal").await;
    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/labels/{work}"),
        json!({ "name": "Business" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["name"], "Business");
    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/labels/{work}"),
        json!({ "name": "personal" }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    let account = create_account(&app, "Legacy labels", 100_000).await;
    let entry = create_entry(&app, "expense", account, 1_000, "Legacy entry", None).await;
    let current = state(&app).await;
    assert_eq!(current["labels"].as_array().unwrap().len(), 2);
    let legacy = current["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == entry)
        .unwrap();
    assert_eq!(legacy["labels"], json!([]));
    assert_eq!(current["accounts"][0]["balance_minor"], 99_000);
    assert_eq!(current["totals"]["expense_minor"], 1_000);

    let (status, _, _, csv) = send_csv(&app, "/api/ledger/export").await;
    assert_eq!(status, StatusCode::OK);
    let csv = String::from_utf8(csv).unwrap();
    assert_eq!(
        csv.lines().next().unwrap(),
        "Date,Type,Name,Account,Source Account,Destination Account,Category,Subcategory,Amount,Currency,Status,Note"
    );
    assert!(!csv.contains("Business"));

    let (status, body) = send_json(&app, "GET", "/api/labels", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body.as_array().unwrap().len(), 2);
    assert!(body
        .as_array()
        .unwrap()
        .iter()
        .any(|label| label["id"] == work));
    assert!(body
        .as_array()
        .unwrap()
        .iter()
        .any(|label| label["id"] == personal));
}

#[tokio::test]
async fn transaction_labels_support_atomic_multi_assignment_omission_clear_duplicate_and_cleanup() {
    let (app, pool, _directory) = setup().await;
    let account = create_account(&app, "Label transaction account", 100_000).await;
    let work = create_label(&app, "Work").await;
    let personal = create_label(&app, "Personal").await;
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/transactions",
        json!({
            "kind": "expense",
            "account_id": account,
            "amount_minor": 2_000,
            "name": "Labelled expense",
            "labels": [work, personal, work]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let entry = body["id"].as_i64().unwrap();
    let current = state(&app).await;
    let labels = &current["entries"][0]["labels"];
    assert_eq!(labels.as_array().unwrap().len(), 2);
    assert_eq!(labels[0]["name"], "Personal");
    assert_eq!(labels[1]["name"], "Work");

    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/transactions/{entry}"),
        json!({
            "kind": "expense",
            "account_id": account,
            "amount_minor": 2_000,
            "name": "Labelled expense"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = send_json(
        &app,
        "GET",
        &format!("/api/transactions/{entry}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["labels"].as_array().unwrap().len(), 2);

    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/transactions/{entry}"),
        json!({
            "kind": "expense",
            "account_id": account,
            "amount_minor": 3_000,
            "name": "Invalid label update",
            "labels": [999_999]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, body) = send_json(
        &app,
        "GET",
        &format!("/api/transactions/{entry}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["amount_minor"], 2_000);
    assert_eq!(body["labels"].as_array().unwrap().len(), 2);

    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/transactions/{entry}"),
        json!({
            "kind": "expense",
            "account_id": account,
            "amount_minor": 2_000,
            "name": "One label",
            "labels": [personal]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/transactions/{entry}"),
        json!({
            "kind": "expense",
            "account_id": account,
            "amount_minor": 2_000,
            "name": "Cleared labels",
            "labels": []
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = send_json(
        &app,
        "GET",
        &format!("/api/transactions/{entry}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["labels"], json!([]));

    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/transactions/{entry}"),
        json!({
            "kind": "expense",
            "account_id": account,
            "amount_minor": 2_000,
            "name": "Restored labels",
            "labels": [work, personal]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = send_json(
        &app,
        "POST",
        &format!("/api/transactions/{entry}/duplicate"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let duplicate = body["id"].as_i64().unwrap();
    let (status, body) = send_json(
        &app,
        "GET",
        &format!("/api/transactions/{duplicate}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["labels"].as_array().unwrap().len(), 2);
    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/transactions/{entry}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let links: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entry_labels WHERE entry_id = ?")
        .bind(entry)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(links, 0);
    let duplicate_links: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM entry_labels WHERE entry_id = ?")
            .bind(duplicate)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(duplicate_links, 2);
}

#[tokio::test]
async fn transfer_labels_are_one_atomic_item_shown_on_both_history_sides_and_cleaned_up() {
    let (app, pool, _directory) = setup().await;
    let checking = create_account(&app, "Transfer labels checking", 100_000).await;
    let savings = create_account(&app, "Transfer labels savings", 20_000).await;
    let work = create_label(&app, "Shared").await;
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/transfers",
        json!({
            "from_account_id": checking,
            "to_account_id": savings,
            "amount_minor": 5_000,
            "name": "Labelled transfer",
            "labels": [work]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let transfer_id = body["id"].as_i64().unwrap();
    let current = state(&app).await;
    assert_eq!(current["entries"][0]["labels"][0]["name"], "Shared");
    for account in [checking, savings] {
        let (status, body) = send_json(
            &app,
            "GET",
            &format!("/api/accounts/{account}/history"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["entries"][0]["labels"][0]["name"], "Shared");
    }

    let (status, body) = send_json(
        &app,
        "PUT",
        &format!("/api/transfers/{transfer_id}"),
        json!({
            "from_account_id": checking,
            "to_account_id": savings,
            "amount_minor": 5_000,
            "name": "Labelled transfer"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = send_json(
        &app,
        "POST",
        &format!("/api/transfers/{transfer_id}/duplicate"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let duplicate = body["id"].as_i64().unwrap();
    let duplicate_links: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM transfer_labels WHERE transfer_id = ?")
            .bind(duplicate)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(duplicate_links, 1);

    let before = state(&app).await;
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/transfers",
        json!({
            "from_account_id": checking,
            "to_account_id": savings,
            "amount_minor": 1_000,
            "name": "Invalid transfer label",
            "labels": [999_999]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let after = state(&app).await;
    assert_eq!(after["accounts"], before["accounts"]);
    assert_eq!(after["entries"].as_array().unwrap().len(), 2);

    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/transfers/{transfer_id}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let links: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM transfer_labels WHERE transfer_id = ?")
            .bind(transfer_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(links, 0);
}
