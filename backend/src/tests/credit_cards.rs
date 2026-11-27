use super::*;

async fn create_card(
    app: &Router,
    name: &str,
    opening_balance_minor: i64,
    opening_date: Option<&str>,
    cutoff_day: Option<i64>,
    due_day: Option<i64>,
    hide_from_selection: bool,
    hide_from_reports: bool,
) -> i64 {
    let (status, body) = send_json(
        app,
        "POST",
        "/api/accounts",
        json!({
            "name": name,
            "currency": "INR",
            "account_type": "credit_card",
            "opening_balance_minor": opening_balance_minor,
            "opening_date": opening_date,
            "credit_limit_minor": 500000,
            "cutoff_day": cutoff_day,
            "due_day": due_day,
            "hide_from_selection": hide_from_selection,
            "hide_from_reports": hide_from_reports
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_i64().unwrap()
}

async fn transfer_at(
    app: &Router,
    from: i64,
    to: i64,
    amount_minor: i64,
    name: &str,
    occurred_at: &str,
) -> i64 {
    let (status, body) = send_json(
        app,
        "POST",
        "/api/transfers",
        json!({
            "from_account_id": from,
            "to_account_id": to,
            "amount_minor": amount_minor,
            "name": name,
            "occurred_at": occurred_at
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_i64().unwrap()
}

async fn card_report(app: &Router, account_id: i64, anchor_date: &str) -> (StatusCode, Value) {
    send_json(
        app,
        "GET",
        &format!("/api/reports/credit-card?account_id={account_id}&anchor_date={anchor_date}"),
        json!({}),
    )
    .await
}

async fn account_balance(app: &Router, account_id: i64) -> i64 {
    state(app).await["accounts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|account| account["id"] == account_id)
        .unwrap()["balance_minor"]
        .as_i64()
        .unwrap()
}

async fn card_history_entry(app: &Router, account_id: i64, name: &str) -> Value {
    let (status, body) = send_json(
        app,
        "GET",
        &format!("/api/accounts/{account_id}/history?limit=200&offset=0"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"] == name)
        .unwrap()
        .clone()
}

#[tokio::test]
async fn credit_card_statement_separates_cutoff_balance_card_payments_and_later_credits() {
    let (app, _pool, _directory) = setup().await;
    let card = create_card(
        &app,
        "F09 statement card",
        0,
        Some("2026-01-01"),
        Some(18),
        Some(2),
        false,
        false,
    )
    .await;
    let bank = create_account(&app, "F09 payment bank", 1_000_000).await;
    for (kind, amount, name, date) in [
        (
            "expense",
            10_000,
            "Prior cutoff charge",
            "2026-10-18T23:59:59",
        ),
        (
            "expense",
            25_000,
            "Statement start charge",
            "2026-10-19T00:00:00",
        ),
        ("income", 3_000, "Recorded refund", "2026-11-05T12:00:00"),
        (
            "expense",
            12_550,
            "Cutoff day charge",
            "2026-11-18T23:59:59+05:30",
        ),
        ("expense", 7_000, "Next day charge", "2026-11-19T00:00:00"),
        ("income", 1_500, "Later refund", "2026-11-20T12:00:00"),
        ("expense", 4_000, "After due charge", "2026-12-03T12:00:00"),
    ] {
        create_entry_at(&app, kind, card, amount, name, None, date).await;
    }
    transfer_at(
        &app,
        bank,
        card,
        2_000,
        "Statement card repayment",
        "2026-11-06T12:00:00",
    )
    .await;
    transfer_at(
        &app,
        card,
        bank,
        10_000,
        "Pre-cutoff cash advance",
        "2026-11-10T12:00:00",
    )
    .await;
    let due_payment = transfer_at(
        &app,
        bank,
        card,
        5_000,
        "Due-window card repayment",
        "2026-11-19T12:00:00",
    )
    .await;
    transfer_at(
        &app,
        bank,
        card,
        2_500,
        "Due-day card repayment",
        "2026-12-02T23:59:59",
    )
    .await;
    transfer_at(
        &app,
        card,
        bank,
        1_200,
        "Post-cutoff cash advance",
        "2026-11-25T12:00:00",
    )
    .await;
    transfer_at(
        &app,
        card,
        bank,
        1_000,
        "After-due cash advance",
        "2026-12-03T00:00:00",
    )
    .await;

    let before_state = state(&app).await;
    let (_, _, _, before_csv) = send_csv(&app, "/api/ledger/export").await;
    let (status, body) = card_report(&app, card, "2026-11-27").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["account_id"], card);
    assert_eq!(body["current_balance_minor"], -56_750);
    assert_eq!(body["current_owed_minor"], 56_750);
    assert_eq!(body["current_credit_minor"], 0);
    assert_eq!(body["available_credit_minor"], 443_250);
    assert_eq!(body["cutoff_day"], 18);
    assert_eq!(body["due_day"], 2);
    assert_eq!(body["missing_settings"], json!([]));
    let statement = &body["statement"];
    assert_eq!(statement["start_date"], "2026-10-19");
    assert_eq!(statement["cutoff_date"], "2026-11-18");
    assert_eq!(statement["end_date_exclusive"], "2026-11-19");
    assert_eq!(statement["due_date"], "2026-12-02");
    assert_eq!(statement["payment_start_date"], "2026-11-19");
    assert_eq!(statement["payment_end_date_exclusive"], "2026-12-03");
    assert_eq!(statement["balance_before_period_minor"], -10_000);
    assert_eq!(statement["balance_at_cutoff_minor"], -52_550);
    assert_eq!(statement["owed_at_cutoff_minor"], 52_550);
    assert_eq!(statement["credit_at_cutoff_minor"], 0);
    assert_eq!(statement["charge_minor"], 37_550);
    assert_eq!(statement["non_transfer_credit_minor"], 3_000);
    assert_eq!(statement["statement_payment_minor"], 2_000);
    assert_eq!(statement["statement_advance_minor"], 10_000);
    assert_eq!(statement["post_cutoff_charge_minor"], 7_000);
    assert_eq!(statement["post_cutoff_non_transfer_credit_minor"], 1_500);
    assert_eq!(statement["post_cutoff_payment_minor"], 7_500);
    assert_eq!(statement["post_cutoff_advance_minor"], 1_200);
    assert_eq!(statement["charge_count"], 2);
    assert_eq!(statement["non_transfer_credit_count"], 1);
    assert_eq!(statement["statement_payment_count"], 1);
    assert_eq!(statement["statement_advance_count"], 1);
    assert_eq!(statement["post_cutoff_charge_count"], 1);
    assert_eq!(statement["post_cutoff_non_transfer_credit_count"], 1);
    assert_eq!(statement["post_cutoff_payment_count"], 2);
    assert_eq!(statement["post_cutoff_advance_count"], 1);
    assert_eq!(statement["invalid_date_entry_count"], 0);
    assert_eq!(statement["legacy_opening_balance"], false);
    assert_eq!(
        statement["statement_activities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|event| event["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![
            "Statement start charge",
            "Recorded refund",
            "Pre-cutoff cash advance",
            "Cutoff day charge",
        ]
    );
    assert_eq!(statement["statement_payments"].as_array().unwrap().len(), 1);
    assert_eq!(
        statement["statement_payments"][0]["name"],
        "Statement card repayment"
    );
    assert_eq!(
        statement["statement_payments"][0]["entry_kind"],
        "transfer_in"
    );
    assert_eq!(
        statement["statement_payments"][0]["counterpart_account_name"],
        "F09 payment bank"
    );
    assert_eq!(
        statement["post_cutoff_charges"][0]["name"],
        "Next day charge"
    );
    assert_eq!(
        statement["post_cutoff_advances"][0]["name"],
        "Post-cutoff cash advance"
    );
    assert_eq!(
        statement["post_cutoff_advances"][0]["entry_kind"],
        "transfer_out"
    );
    assert_eq!(
        statement["post_cutoff_payments"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        statement["post_cutoff_payments"][0]["transfer_id"],
        due_payment
    );
    assert_eq!(
        statement["post_cutoff_payments"][0]["entry_kind"],
        "transfer_in"
    );
    assert_eq!(
        statement["post_cutoff_payments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|event| event["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["Due-window card repayment", "Due-day card repayment"]
    );
    assert_eq!(
        statement["post_cutoff_non_transfer_credits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|event| event["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["Later refund"]
    );
    assert!(!statement["statement_activities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event["name"] == "Next day charge"
            || event["name"] == "Statement card repayment"));
    assert!(!statement["post_cutoff_payments"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event["name"] == "Pre-cutoff cash advance"
            || event["name"] == "After-due cash advance"
            || event["name"] == "Post-cutoff cash advance"));

    let current = state(&app).await;
    assert_eq!(
        current["accounts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|account| account["id"] == card)
            .unwrap()["balance_minor"],
        body["current_balance_minor"]
    );
    assert_eq!(state(&app).await, before_state);
    let (_, _, _, after_csv) = send_csv(&app, "/api/ledger/export").await;
    assert_eq!(after_csv, before_csv);
}

#[tokio::test]
async fn credit_card_transfer_direction_uses_canonical_card_side_and_window() {
    let (app, _pool, _directory) = setup().await;
    let card = create_card(
        &app,
        "F09 direction card",
        -100_000,
        Some("2026-01-01"),
        Some(18),
        Some(2),
        false,
        false,
    )
    .await;
    let bank = create_account(&app, "F09 direction bank", 100_000).await;
    assert_eq!(account_balance(&app, card).await, -100_000);

    transfer_at(
        &app,
        bank,
        card,
        25_000,
        "Statement repayment",
        "2026-11-10T12:00:00",
    )
    .await;
    assert_eq!(account_balance(&app, card).await, -75_000);
    let repayment = card_history_entry(&app, card, "Statement repayment").await;
    assert_eq!(repayment["kind"], "transfer_in");
    assert_eq!(repayment["amount_minor"], 25_000);

    transfer_at(
        &app,
        card,
        bank,
        5_000,
        "Statement cash advance",
        "2026-11-11T12:00:00",
    )
    .await;
    assert_eq!(account_balance(&app, card).await, -80_000);
    let advance = card_history_entry(&app, card, "Statement cash advance").await;
    assert_eq!(advance["kind"], "transfer_out");
    assert_eq!(advance["amount_minor"], 5_000);

    transfer_at(
        &app,
        bank,
        card,
        3_000,
        "Cutoff-day repayment",
        "2026-11-18T23:59:59",
    )
    .await;
    assert_eq!(account_balance(&app, card).await, -77_000);
    let cutoff_repayment = card_history_entry(&app, card, "Cutoff-day repayment").await;
    assert_eq!(cutoff_repayment["kind"], "transfer_in");
    assert_eq!(cutoff_repayment["amount_minor"], 3_000);

    transfer_at(
        &app,
        bank,
        card,
        4_000,
        "Next-day repayment",
        "2026-11-19T00:00:00",
    )
    .await;
    assert_eq!(account_balance(&app, card).await, -73_000);
    let next_day_repayment = card_history_entry(&app, card, "Next-day repayment").await;
    assert_eq!(next_day_repayment["kind"], "transfer_in");
    assert_eq!(next_day_repayment["amount_minor"], 4_000);

    transfer_at(
        &app,
        card,
        bank,
        6_000,
        "Post-cutoff cash advance",
        "2026-11-20T12:00:00",
    )
    .await;
    assert_eq!(account_balance(&app, card).await, -79_000);
    let post_advance = card_history_entry(&app, card, "Post-cutoff cash advance").await;
    assert_eq!(post_advance["kind"], "transfer_out");
    assert_eq!(post_advance["amount_minor"], 6_000);

    let (status, body) = card_report(&app, card, "2026-11-27").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let statement = &body["statement"];
    assert_eq!(statement["opening_baseline_before_period_minor"], -100_000);
    assert_eq!(statement["opening_baseline_inside_period_minor"], 0);
    assert_eq!(statement["balance_before_period_minor"], -100_000);
    assert_eq!(statement["balance_at_cutoff_minor"], -77_000);
    assert_eq!(statement["charge_minor"], 0);
    assert_eq!(statement["non_transfer_credit_minor"], 0);
    assert_eq!(statement["statement_payment_minor"], 28_000);
    assert_eq!(statement["statement_payment_count"], 2);
    assert_eq!(statement["statement_advance_minor"], 5_000);
    assert_eq!(statement["statement_advance_count"], 1);
    assert_eq!(statement["post_cutoff_payment_minor"], 4_000);
    assert_eq!(statement["post_cutoff_payment_count"], 1);
    assert_eq!(statement["post_cutoff_advance_minor"], 6_000);
    assert_eq!(statement["post_cutoff_advance_count"], 1);
    assert_eq!(statement["post_cutoff_charge_minor"], 0);
    assert_eq!(statement["post_cutoff_non_transfer_credit_minor"], 0);
    assert_eq!(body["current_balance_minor"], -79_000);
    assert_eq!(
        statement["statement_payments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|event| event["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["Statement repayment", "Cutoff-day repayment"]
    );
    assert_eq!(
        statement["post_cutoff_payments"][0]["name"],
        "Next-day repayment"
    );
    assert_eq!(
        statement["post_cutoff_payments"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        statement["post_cutoff_advances"][0]["name"],
        "Post-cutoff cash advance"
    );
    assert_eq!(
        statement["post_cutoff_advances"].as_array().unwrap().len(),
        1
    );
    assert!(!statement["post_cutoff_payments"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event["name"] == "Cutoff-day repayment"));
}

#[tokio::test]
async fn credit_card_opening_baseline_gates_before_start_inside_cutoff_and_after() {
    let (app, _pool, _directory) = setup().await;
    let before = create_card(
        &app,
        "F09 opening before period",
        100_000,
        Some("2026-10-18"),
        Some(18),
        Some(2),
        false,
        false,
    )
    .await;
    let exact_start = create_card(
        &app,
        "F09 opening exact start",
        100_000,
        Some("2026-10-19"),
        Some(18),
        Some(2),
        false,
        false,
    )
    .await;
    let inside = create_card(
        &app,
        "F09 opening inside period",
        100_000,
        Some("2026-11-10"),
        Some(18),
        Some(2),
        false,
        false,
    )
    .await;
    let cutoff = create_card(
        &app,
        "F09 opening on cutoff",
        100_000,
        Some("2026-11-18"),
        Some(18),
        Some(2),
        false,
        false,
    )
    .await;
    let after = create_card(
        &app,
        "F09 opening after cutoff",
        100_000,
        Some("2026-11-19"),
        Some(18),
        Some(2),
        false,
        false,
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        inside,
        5_000,
        "Actual pre-period charge",
        None,
        "2026-10-18T12:00:00",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        inside,
        20_000,
        "Inside-period charge",
        None,
        "2026-11-10T12:00:00",
    )
    .await;

    for (account_id, before_baseline, inside_baseline, before_balance, cutoff_balance) in [
        (before, 100_000, 0, 100_000, 100_000),
        (exact_start, 0, 100_000, 0, 100_000),
        (cutoff, 0, 100_000, 0, 100_000),
        (after, 0, 0, 0, 0),
    ] {
        let (status, body) = card_report(&app, account_id, "2026-11-27").await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let statement = &body["statement"];
        assert_eq!(
            statement["opening_baseline_before_period_minor"],
            before_baseline
        );
        assert_eq!(
            statement["opening_baseline_inside_period_minor"],
            inside_baseline
        );
        assert_eq!(statement["balance_before_period_minor"], before_balance);
        assert_eq!(statement["balance_at_cutoff_minor"], cutoff_balance);
    }

    let (status, body) = card_report(&app, inside, "2026-11-27").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let statement = &body["statement"];
    assert_eq!(statement["opening_baseline_before_period_minor"], 0);
    assert_eq!(statement["opening_baseline_inside_period_minor"], 100_000);
    assert_eq!(statement["balance_before_period_minor"], -5_000);
    assert_eq!(statement["charge_minor"], 20_000);
    assert_eq!(statement["balance_at_cutoff_minor"], 75_000);
    assert_eq!(body["current_balance_minor"], 75_000);
    assert_eq!(
        statement["balance_before_period_minor"].as_i64().unwrap()
            + statement["opening_baseline_inside_period_minor"]
                .as_i64()
                .unwrap()
            - statement["charge_minor"].as_i64().unwrap()
            + statement["non_transfer_credit_minor"].as_i64().unwrap()
            + statement["statement_payment_minor"].as_i64().unwrap()
            - statement["statement_advance_minor"].as_i64().unwrap(),
        statement["balance_at_cutoff_minor"].as_i64().unwrap()
    );

    let (status, body) = card_report(&app, after, "2026-11-27").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["current_balance_minor"], 100_000);
    assert_eq!(body["statement"]["opening_balance_minor"], 0);
}

#[tokio::test]
async fn credit_card_short_month_clamps_days_and_includes_due_day_but_not_the_next_day() {
    let (app, _pool, _directory) = setup().await;
    let card = create_card(
        &app,
        "F09 short-month card",
        0,
        Some("2026-01-01"),
        Some(18),
        Some(31),
        false,
        false,
    )
    .await;
    let bank = create_account(&app, "F09 short-month bank", 100_000).await;
    create_entry_at(
        &app,
        "expense",
        card,
        100,
        "Short-month cutoff charge",
        None,
        "2026-02-18T23:59:59",
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        card,
        200,
        "Short-month next-day charge",
        None,
        "2026-02-19T00:00:00",
    )
    .await;
    transfer_at(
        &app,
        bank,
        card,
        300,
        "Clamped due-day repayment",
        "2026-02-28T23:59:59",
    )
    .await;
    transfer_at(
        &app,
        card,
        bank,
        400,
        "After clamped due advance",
        "2026-03-01T00:00:00",
    )
    .await;

    let (status, body) = card_report(&app, card, "2026-02-10").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let statement = &body["statement"];
    assert_eq!(statement["start_date"], "2026-01-19");
    assert_eq!(statement["cutoff_date"], "2026-02-18");
    assert_eq!(statement["end_date_exclusive"], "2026-02-19");
    assert_eq!(statement["due_date"], "2026-02-28");
    assert_eq!(statement["payment_end_date_exclusive"], "2026-03-01");
    assert_eq!(statement["balance_before_period_minor"], 0);
    assert_eq!(statement["balance_at_cutoff_minor"], -100);
    assert_eq!(statement["owed_at_cutoff_minor"], 100);
    assert_eq!(statement["charge_minor"], 100);
    assert_eq!(statement["statement_payment_minor"], 0);
    assert_eq!(statement["statement_advance_minor"], 0);
    assert_eq!(statement["post_cutoff_payment_minor"], 300);
    assert_eq!(statement["post_cutoff_charge_minor"], 200);
    assert_eq!(statement["post_cutoff_advance_minor"], 0);
    assert_eq!(
        statement["post_cutoff_charges"][0]["name"],
        "Short-month next-day charge"
    );
    assert_eq!(
        statement["post_cutoff_payments"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        statement["post_cutoff_payments"][0]["name"],
        "Clamped due-day repayment"
    );
    assert_eq!(
        statement["post_cutoff_payments"][0]["entry_kind"],
        "transfer_in"
    );
    assert_eq!(body["current_balance_minor"], -400);
}

#[tokio::test]
async fn credit_card_missing_settings_visibility_validation_and_zero_state_are_explicit() {
    let (app, pool, _directory) = setup().await;
    let missing = create_card(
        &app,
        "F09 missing settings",
        -12_345,
        Some("2026-01-01"),
        None,
        None,
        false,
        false,
    )
    .await;
    let picker_hidden = create_card(
        &app,
        "F09 picker-hidden card",
        0,
        Some("2026-01-01"),
        Some(18),
        Some(2),
        true,
        false,
    )
    .await;
    let report_hidden = create_card(
        &app,
        "F09 report-hidden card",
        0,
        Some("2026-01-01"),
        Some(18),
        Some(2),
        true,
        true,
    )
    .await;
    let zero = create_card(
        &app,
        "F09 settled card",
        0,
        Some("2026-01-01"),
        Some(18),
        Some(2),
        false,
        false,
    )
    .await;
    let bank = create_account(&app, "F09 ordinary bank", 0).await;

    let (status, body) = card_report(&app, missing, "2026-11-27").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["current_balance_minor"], -12_345);
    assert_eq!(body["current_owed_minor"], 12_345);
    assert_eq!(body["available_credit_minor"], 487_655);
    assert_eq!(body["missing_settings"], json!(["cutoff_day", "due_day"]));
    assert!(body["statement"].is_null());

    let (status, body) = card_report(&app, picker_hidden, "2026-11-27").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["statement"].is_object());
    let (status, body) = card_report(&app, report_hidden, "2026-11-27").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body) = card_report(&app, bank, "2026-11-27").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, body) = card_report(&app, 999_999, "2026-11-27").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body) = card_report(&app, zero, "2026-02-30").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let (status, body) = card_report(&app, zero, "2026-11-27").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["statement"]["balance_at_cutoff_minor"], 0);
    assert_eq!(body["statement"]["owed_at_cutoff_minor"], 0);
    assert_eq!(body["statement"]["credit_at_cutoff_minor"], 0);
    assert_eq!(body["current_owed_minor"], 0);
    assert_eq!(body["current_credit_minor"], 0);
    assert_eq!(body["available_credit_minor"], 500_000);

    let mut legacy_connection = pool.acquire().await.unwrap();
    sqlx::query("PRAGMA ignore_check_constraints = ON")
        .execute(&mut *legacy_connection)
        .await
        .unwrap();
    sqlx::query("UPDATE accounts SET cutoff_day = 0 WHERE id = ?")
        .bind(zero)
        .execute(&mut *legacy_connection)
        .await
        .unwrap();
    drop(legacy_connection);
    let (status, body) = card_report(&app, zero, "2026-11-27").await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    sqlx::query("UPDATE accounts SET cutoff_day = 18, opening_date = 'not-a-date' WHERE id = ?")
        .bind(zero)
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = card_report(&app, zero, "2026-11-27").await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("invalid opening date"));
}

#[tokio::test]
async fn credit_card_invalid_legacy_dates_are_excluded_and_opening_baselines_are_explicit() {
    let (app, pool, _directory) = setup().await;
    let legacy = create_card(
        &app,
        "F09 legacy opening card",
        -1_000,
        None,
        Some(18),
        Some(2),
        false,
        false,
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        legacy,
        500,
        "Legacy baseline charge",
        None,
        "2026-01-10",
    )
    .await;
    sqlx::query(
        "INSERT INTO entries (kind, account_id, amount_minor, name, note, occurred_at, created_at)
         VALUES ('expense', ?, 999, 'Invalid legacy card charge', '', 'not-a-date', '2026-01-01T00:00:00Z')",
    )
    .bind(legacy)
    .execute(&pool)
    .await
    .unwrap();
    let (status, body) = card_report(&app, legacy, "2026-11-27").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["current_balance_minor"], -2_499);
    assert_eq!(
        body["statement"]["opening_baseline_before_period_minor"],
        -1_000
    );
    assert_eq!(body["statement"]["opening_baseline_inside_period_minor"], 0);
    assert_eq!(body["statement"]["balance_before_period_minor"], -1_500);
    assert_eq!(body["statement"]["balance_at_cutoff_minor"], -1_500);
    assert_eq!(body["statement"]["opening_balance_minor"], -1_000);
    assert_eq!(body["statement"]["invalid_date_entry_count"], 1);
    assert_eq!(body["statement"]["legacy_opening_balance"], true);

    let future_opening = create_card(
        &app,
        "F09 future opening card",
        10_000,
        Some("2026-12-01"),
        Some(18),
        Some(2),
        false,
        false,
    )
    .await;
    create_entry_at(
        &app,
        "expense",
        future_opening,
        500,
        "Posted before corrected opening metadata",
        None,
        "2026-11-10",
    )
    .await;
    let (status, body) = card_report(&app, future_opening, "2026-11-27").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["current_balance_minor"], 9_500);
    assert_eq!(body["statement"]["opening_balance_minor"], 0);
    assert_eq!(body["statement"]["opening_baseline_before_period_minor"], 0);
    assert_eq!(body["statement"]["opening_baseline_inside_period_minor"], 0);
    assert_eq!(body["statement"]["balance_before_period_minor"], 0);
    assert_eq!(body["statement"]["balance_at_cutoff_minor"], -500);
    assert_eq!(body["statement"]["legacy_opening_balance"], false);
}

#[tokio::test]
async fn credit_card_report_rejects_unrepresentable_signed_paise_totals() {
    let (app, _pool, _directory) = setup().await;
    let card = create_card(
        &app,
        "F09 overflow card",
        i64::MAX,
        Some("2026-01-01"),
        Some(18),
        Some(2),
        false,
        false,
    )
    .await;
    create_entry_at(
        &app,
        "income",
        card,
        1,
        "Overflowing card credit",
        None,
        "2026-11-10",
    )
    .await;
    let (status, body) = card_report(&app, card, "2026-11-27").await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("outside the supported integer range"));
}
