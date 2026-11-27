use super::*;
use crate::store::Store;

async fn setup_stub() -> (Router, SqlitePool, TempDir) {
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
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (
        build_router_with(
            pool.clone(),
            IdentityConfig::default(),
            AssistantClient::stub(),
        ),
        pool,
        directory,
    )
}

#[tokio::test]
async fn stub_prepares_a_row_and_a_targeted_follow_up_preserves_manual_edits() {
    let (app, pool, _directory) = setup_stub().await;
    let account = create_account(&app, "Cash", 10000).await;

    // An unrelated manual row is edited before the assistant runs.
    let (status, _) = send_json(
        &app,
        "POST",
        "/api/draft/rows",
        json!({
            "row_id": "manual-1",
            "kind": "expense",
            "name": "Manual coffee",
            "amount_minor": 7500,
            "date": "2026-11-26",
            "account_id": account,
            "manual": true
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, response) = send_json(
        &app,
        "POST",
        "/api/draft/prompt",
        json!({
            "text": "Yesterday I paid ₹240 for lunch",
            "client_prompt_id": "submission-1",
            "local_date": "2026-11-27"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["draft"]["rows"].as_array().unwrap().len(), 2);
    assert_eq!(response["draft"]["prompts"].as_array().unwrap().len(), 2);
    let proposed = response["draft"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["source"] == "assistant")
        .unwrap()
        .clone();
    assert_eq!(proposed["amount_minor"], 24000);
    assert!(proposed["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|issue| issue.as_str().unwrap().contains("account")));

    // Apply a follow-up to exactly one row through the store seam the route
    // uses. The manual row must not change.
    let store = Store::new(pool);
    let mut edited = proposed.clone();
    edited["account_id"] = json!(account);
    let proposal = AssistantProposal {
        rows: Vec::new(),
        edits: vec![AssistantRowEdit {
            row_id: edited["row_id"].as_str().unwrap().to_string(),
            row: serde_json::from_value(edited).unwrap(),
        }],
        message: "Assigned the account".to_string(),
        clarification: None,
    };
    let response = store
        .apply_proposal(1, "submission-2", proposal, None)
        .await
        .unwrap();
    let manual = response
        .draft
        .rows
        .iter()
        .find(|row| row.row_id == "manual-1")
        .unwrap();
    assert_eq!(manual.amount_minor, Some(7500));
    assert_eq!(manual.name, "Manual coffee");
    assert!(manual.manual);
    let edited = response
        .draft
        .rows
        .iter()
        .find(|row| row.row_id != "manual-1")
        .unwrap();
    assert_eq!(edited.account_id, Some(account));

    // Manual save still works after the assistant prepared the draft.
    let (status, receipt) = send_json(
        &app,
        "POST",
        "/api/draft/save",
        json!({ "revision": response.draft.revision }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(receipt["entry_ids"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn assistant_context_is_an_explicit_allowlist_without_private_transcript() {
    let (app, pool, _directory) = setup_stub().await;
    let account = create_account(&app, "HDFC", 10000).await;
    let hidden = create_account(&app, "Other", 5000).await;
    let _ = hidden;
    send_json(
        &app,
        "POST",
        "/api/draft/prompt",
        json!({
            "text": "a private one-off instruction",
            "client_prompt_id": "private-prompt",
            "local_date": "2026-11-27"
        }),
    )
    .await;
    send_json(
        &app,
        "POST",
        "/api/draft/rows",
        json!({
            "row_id": "row-1",
            "kind": "expense",
            "name": "Lunch",
            "amount_minor": 100,
            "date": "2026-11-27",
            "account_id": account,
            "manual": true
        }),
    )
    .await;

    let store = Store::new(pool);
    let draft = store.load_draft(1).await.unwrap();
    let (accounts, categories) = store.draft_context(1, &draft, false).await.unwrap();
    assert!(!accounts.iter().any(|account| account.name == "Other"));
    assert_eq!(accounts.len(), 1);
    let request = AssistantRequest {
        text: "add coffee".to_string(),
        local_date: draft.local_date.clone(),
        rows: draft.rows.clone(),
        account_names: accounts,
        category_names: categories,
    };
    let serialized = serde_json::to_string(&request).unwrap();
    assert!(!serialized.contains("private one-off instruction"));
    assert!(serialized.contains("HDFC"));
    assert_eq!(request.rows.len(), 1);
}

#[tokio::test]
async fn disabled_assistant_keeps_the_prompt_and_allows_manual_completion() {
    let (app, _pool, _directory) = setup().await;
    let account = create_account(&app, "Cash", 10000).await;
    let (status, response) = send_json(
        &app,
        "POST",
        "/api/draft/prompt",
        json!({
            "text": "Yesterday I paid ₹240 for lunch",
            "client_prompt_id": "outage-1",
            "local_date": "2026-11-27"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert!(response["draft"]["prompts"].as_array().unwrap().len() >= 1);
    assert!(response["message"]
        .as_str()
        .unwrap()
        .contains("unavailable"));

    send_json(
        &app,
        "POST",
        "/api/draft/rows",
        json!({
            "row_id": "manual-1",
            "kind": "expense",
            "name": "Lunch",
            "amount_minor": 24000,
            "date": "2026-11-27",
            "account_id": account,
            "manual": true
        }),
    )
    .await;
    let current = send_json(&app, "GET", "/api/draft", json!({})).await.1;
    let (status, receipt) = send_json(
        &app,
        "POST",
        "/api/draft/save",
        json!({ "revision": current["revision"] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(state(&app).await["totals"]["expense_minor"], 24000);
}
