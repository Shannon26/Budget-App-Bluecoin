use super::*;

async fn owned_account(app: &Router, member: i64, name: &str, opening: i64) -> i64 {
    let (status, body) = send_json_as(
        app,
        member,
        "POST",
        "/api/accounts",
        json!({ "name": name, "opening_balance_minor": opening }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_i64().unwrap()
}

/// Save a cross-member transfer from member 1's draft. The request stays
/// pending and has no ledger or balance effect until approval.
async fn submit_cross_member_transfer(app: &Router, from: i64, to: i64, amount: i64) -> Value {
    let current = get_as(app, 1, "/api/draft").await.1;
    let revision = current["revision"].as_i64().unwrap();
    let (status, body) = send_json_as(
        app,
        1,
        "POST",
        "/api/draft/rows",
        json!({
            "row_id": "transfer-1",
            "kind": "transfer",
            "name": "Shared expenses",
            "amount_minor": amount,
            "date": "2026-11-23",
            "account_id": from,
            "to_account_id": to,
            "manual": true
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, receipt) = send_json_as(
        app,
        1,
        "POST",
        "/api/draft/save",
        json!({ "revision": revision + 1 }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(
        receipt["pending_request_ids"].as_array().unwrap().len(),
        1,
        "{receipt}"
    );
    receipt
}

fn balance_of(body: &Value, name: &str) -> i64 {
    body["accounts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|account| account["name"] == name)
        .unwrap()["balance_minor"]
        .as_i64()
        .unwrap()
}

#[tokio::test]
async fn approval_posts_two_legs_once_and_protects_the_posted_transfer() {
    let (app, pool, _directory) = setup_household().await;
    let source = owned_account(&app, 1, "Sender", 500000).await;
    let destination = owned_account(&app, 2, "Receiver", 100000).await;
    submit_cross_member_transfer(&app, source, destination, 50000).await;

    let (status, outgoing) = get_as(&app, 1, "/api/transfer-requests").await;
    assert_eq!(status, StatusCode::OK, "{outgoing}");
    assert_eq!(outgoing["outgoing_count"], 1);
    assert_eq!(outgoing["outgoing"][0]["status"], "pending");
    let request_id = outgoing["outgoing"][0]["id"].as_i64().unwrap();

    let (status, incoming) = get_as(&app, 2, "/api/transfer-requests").await;
    assert_eq!(status, StatusCode::OK, "{incoming}");
    assert_eq!(incoming["incoming_count"], 1);

    // Pending requests have no effect on balances or the posted ledger.
    let before = get_as(&app, 1, "/api/state").await.1;
    assert_eq!(balance_of(&before, "Sender"), 500000);
    assert_eq!(balance_of(&before, "Receiver"), 100000);
    let (status, ledger) = get_as(&app, 1, "/api/ledger").await;
    assert_eq!(status, StatusCode::OK, "{ledger}");
    assert_eq!(ledger["total_count"], 0);

    // Only the recipient may approve, and only once.
    let (status, _) = send_json_as(
        &app,
        1,
        "POST",
        &format!("/api/transfer-requests/{request_id}/approve"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, decision) = send_json_as(
        &app,
        2,
        "POST",
        &format!("/api/transfer-requests/{request_id}/approve"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{decision}");
    assert_eq!(decision["request"]["status"], "approved");
    let transfer_id = decision["posted_transfer_id"].as_i64().unwrap();

    let (status, repeated) = send_json_as(
        &app,
        2,
        "POST",
        &format!("/api/transfer-requests/{request_id}/approve"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{repeated}");

    let after = get_as(&app, 1, "/api/state").await.1;
    assert_eq!(balance_of(&after, "Sender"), 450000);
    assert_eq!(balance_of(&after, "Receiver"), 150000);
    assert_eq!(after["totals"]["income_minor"], 0);
    assert_eq!(after["totals"]["expense_minor"], 0);

    let posted = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM entries WHERE transfer_id = ? AND occurred_at = ?",
    )
    .bind(transfer_id)
    .bind("2026-11-23T12:00:00")
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(posted, 2);

    // Direct edits of a posted cross-member transfer are blocked for both.
    for member in [1, 2] {
        let (status, _) = send_json_as(
            &app,
            member,
            "PUT",
            &format!("/api/transfers/{transfer_id}"),
            json!({
                "from_account_id": source,
                "to_account_id": destination,
                "amount_minor": 60000
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
    }
}

#[tokio::test]
async fn decline_and_cancel_leave_balances_untouched() {
    let (app, _pool, _directory) = setup_household().await;
    let source = owned_account(&app, 1, "Sender", 500000).await;
    let destination = owned_account(&app, 2, "Receiver", 100000).await;

    submit_cross_member_transfer(&app, source, destination, 20000).await;
    let requests = get_as(&app, 1, "/api/transfer-requests").await.1;
    let declined_id = requests["outgoing"][0]["id"].as_i64().unwrap();
    let (status, body) = send_json_as(
        &app,
        2,
        "POST",
        &format!("/api/transfer-requests/{declined_id}/decline"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["request"]["status"], "declined");

    submit_cross_member_transfer(&app, source, destination, 30000).await;
    let requests = get_as(&app, 1, "/api/transfer-requests").await.1;
    let canceled_id = requests["outgoing"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["status"] == "pending")
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    let (status, body) = send_json_as(
        &app,
        1,
        "POST",
        &format!("/api/transfer-requests/{canceled_id}/cancel"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["request"]["status"], "canceled");

    let current = get_as(&app, 1, "/api/state").await.1;
    assert_eq!(balance_of(&current, "Sender"), 500000);
    assert_eq!(balance_of(&current, "Receiver"), 100000);
    let (status, ledger) = get_as(&app, 1, "/api/ledger").await;
    assert_eq!(status, StatusCode::OK, "{ledger}");
    assert_eq!(ledger["total_count"], 0);
}

#[tokio::test]
async fn an_approved_reversal_is_a_separate_request_and_cannot_be_repeated() {
    let (app, pool, _directory) = setup_household().await;
    let source = owned_account(&app, 1, "Sender", 500000).await;
    let destination = owned_account(&app, 2, "Receiver", 100000).await;
    submit_cross_member_transfer(&app, source, destination, 50000).await;
    let requests = get_as(&app, 1, "/api/transfer-requests").await.1;
    let request_id = requests["outgoing"][0]["id"].as_i64().unwrap();
    let decision = send_json_as(
        &app,
        2,
        "POST",
        &format!("/api/transfer-requests/{request_id}/approve"),
        json!({}),
    )
    .await
    .1;
    let transfer_id = decision["posted_transfer_id"].as_i64().unwrap();

    // The current owner of the original destination initiates the reversal.
    let (status, reversal) = send_json_as(
        &app,
        2,
        "POST",
        &format!("/api/transfers/{transfer_id}/reverse"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reversal}");
    assert_eq!(reversal["source_account_id"], destination);
    assert_eq!(reversal["destination_account_id"], source);
    assert_eq!(reversal["status"], "pending");
    let reversal_id = reversal["id"].as_i64().unwrap();

    let (status, decision) = send_json_as(
        &app,
        1,
        "POST",
        &format!("/api/transfer-requests/{reversal_id}/approve"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{decision}");

    // Two posted reversals of the same original are not allowed.
    let (status, repeated) = send_json_as(
        &app,
        2,
        "POST",
        &format!("/api/transfers/{transfer_id}/reverse"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{repeated}");

    let current = get_as(&app, 1, "/api/state").await.1;
    assert_eq!(balance_of(&current, "Sender"), 500000);
    assert_eq!(balance_of(&current, "Receiver"), 100000);
    let approval_date =
        sqlx::query_scalar::<_, String>("SELECT decided_at FROM transfer_requests WHERE id = ?")
            .bind(reversal_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!approval_date.is_empty());
}
