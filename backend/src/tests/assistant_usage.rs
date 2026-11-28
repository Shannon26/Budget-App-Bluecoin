use super::*;
use crate::assistant::AssistantClient;
use crate::store::Store;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

const MODEL: &str = "deepseek/deepseek-v4.1-flash-1017";
const PROVIDER: &str = "sail-research/fp4";

#[derive(Clone)]
struct MockState {
    requests: Arc<Mutex<Vec<CapturedRequest>>>,
    responses: Arc<Mutex<VecDeque<MockResponse>>>,
}

#[derive(Clone)]
struct CapturedRequest {
    headers: BTreeMap<String, String>,
    body: Value,
}

struct MockResponse {
    status: StatusCode,
    body: Value,
}

struct MockServer {
    endpoint: String,
    state: MockState,
    server: JoinHandle<()>,
}

impl MockServer {
    fn push(&self, response: MockResponse) {
        self.state.responses.lock().unwrap().push_back(response);
    }

    fn requests(&self) -> Vec<CapturedRequest> {
        self.state.requests.lock().unwrap().clone()
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn mock_handler(
    State(state): State<MockState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let captured_headers = headers
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_string(),
                value.to_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    state.requests.lock().unwrap().push(CapturedRequest {
        headers: captured_headers,
        body,
    });
    let response = state
        .responses
        .lock()
        .unwrap()
        .pop_front()
        .unwrap_or(MockResponse {
            status: StatusCode::SERVICE_UNAVAILABLE,
            body: json!({ "error": { "code": "mock_unavailable" } }),
        });
    (response.status, Json(response.body))
}

async fn mock_server() -> MockServer {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!(
        "http://{}/v1/chat/completions",
        listener.local_addr().unwrap()
    );
    let state = MockState {
        requests: Arc::new(Mutex::new(Vec::new())),
        responses: Arc::new(Mutex::new(VecDeque::new())),
    };
    let app = Router::new()
        .route("/v1/chat/completions", post(mock_handler))
        .with_state(state.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    MockServer {
        endpoint,
        state,
        server,
    }
}

async fn setup_mock(identity: IdentityConfig) -> (Router, SqlitePool, TempDir, MockServer) {
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
    let server = mock_server().await;
    let client =
        AssistantClient::openrouter_for_test(Store::new(pool.clone()), server.endpoint.clone());
    (
        build_router_with(pool.clone(), identity, client),
        pool,
        directory,
        server,
    )
}

fn proposal(name: &str, account_id: i64) -> Value {
    json!({
        "rows": [{
            "row_id": "mock-row-1",
            "ordinal": 1,
            "kind": "expense",
            "name": name,
            "amount_minor": 1234,
            "date": "2026-11-28",
            "account_id": account_id,
            "account_name": null,
            "to_account_id": null,
            "to_account_name": null,
            "category_id": null,
            "note": "",
            "status": "ready",
            "issues": [],
            "manual": false,
            "source": "assistant",
            "proposed_row_id": null,
            "account_type": null,
            "opening_balance_minor": null,
            "current_balance_minor": null,
            "transfer_request": false
        }],
        "edits": [],
        "message": "Prepared one synthetic row.",
        "clarification": null
    })
}

fn completion(id: &str, content: &str, cost: Option<&str>) -> Value {
    let mut body = json!({
        "id": id,
        "model": MODEL,
        "provider": "Sail Research",
        "choices": [{ "message": { "role": "assistant", "content": content } }],
        "usage": { "prompt_tokens": 10, "completion_tokens": 20, "total_tokens": 30 }
    });
    if let Some(cost) = cost {
        body["usage"]["cost"] = serde_json::from_str(cost).unwrap();
    }
    body
}

fn valid_completion(id: &str, name: &str, account_id: i64, cost: Option<&str>) -> MockResponse {
    MockResponse {
        status: StatusCode::OK,
        body: completion(id, &proposal(name, account_id).to_string(), cost),
    }
}

async fn prompt(app: &Router, submission_id: &str) -> (StatusCode, Value) {
    send_json(
        app,
        "POST",
        "/api/draft/prompt",
        json!({
            "text": "Synthetic usage test only",
            "client_prompt_id": submission_id,
            "local_date": "2026-11-28"
        }),
    )
    .await
}

async fn usage(app: &Router) -> Value {
    let (status, body) = send_json(app, "GET", "/api/assistant/usage", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

#[tokio::test]
async fn request_is_pinned_without_attribution_and_persists_cost_before_save() {
    let (app, _pool, _directory, server) = setup_mock(IdentityConfig::default()).await;
    let account = create_account(&app, "Synthetic account", 100000).await;
    server.push(valid_completion(
        "gen-pinned",
        "Synthetic lunch",
        account,
        Some("0.0000256"),
    ));

    let (status, response) = prompt(&app, "pinned-submission").await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["draft"]["rows"].as_array().unwrap().len(), 1);
    assert_eq!(
        state(&app).await["totals"]["expense_minor"],
        0,
        "Prepare must not post"
    );
    assert_eq!(state(&app).await["accounts"].as_array().unwrap().len(), 1);

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(
        request.headers.get("authorization").unwrap(),
        "Bearer test-openrouter-key"
    );
    assert_eq!(
        request.headers.get("content-type").unwrap(),
        "application/json"
    );
    for forbidden in [
        "http-referer",
        "referer",
        "x-title",
        "x-openrouter-title",
        "user-agent",
        "cookie",
        "x-paisa-member",
        "x-paisa-member-token",
        "x-forwarded-for",
        "x-real-ip",
    ] {
        assert!(
            !request.headers.contains_key(forbidden),
            "forbidden header {forbidden}"
        );
    }
    for (name, _) in &request.headers {
        assert!(
            [
                "accept",
                "accept-encoding",
                "authorization",
                "host",
                "content-length",
                "content-type"
            ]
            .contains(&name.as_str()),
            "unexpected outbound header {name}"
        );
    }

    let body = &request.body;
    let mut keys: Vec<_> = body.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "max_tokens",
            "messages",
            "model",
            "provider",
            "response_format"
        ]
    );
    assert_eq!(body["model"], MODEL);
    assert_eq!(body["provider"]["order"], json!([PROVIDER]));
    assert_eq!(body["provider"]["allow_fallbacks"], false);
    assert_eq!(body["provider"]["require_parameters"], true);
    assert_eq!(body["response_format"]["type"], "json_schema");
    assert_eq!(body["response_format"]["json_schema"]["strict"], true);
    let context: Value =
        serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
    let mut context_keys: Vec<_> = context.as_object().unwrap().keys().cloned().collect();
    context_keys.sort();
    assert_eq!(
        context_keys,
        [
            "account_names",
            "category_names",
            "local_date",
            "rows",
            "text"
        ]
    );
    assert!(!body.to_string().contains("client_prompt_id"));
    assert!(!body.to_string().contains("member_id"));

    let summary = usage(&app).await;
    assert_eq!(summary["total_cost_usd"], "0.0000256");
    assert_eq!(summary["completed_count"], 1);
    assert_eq!(summary["reported_count"], 1);
}

#[tokio::test]
async fn billed_invalid_proposal_is_recorded_without_prompt_or_raw_response() {
    let (app, pool, _directory, server) = setup_mock(IdentityConfig::default()).await;
    server.push(MockResponse {
        status: StatusCode::OK,
        body: completion(
            "gen-invalid",
            "not valid proposal JSON",
            Some("0.000123456789"),
        ),
    });

    let (status, response) = prompt(&app, "invalid-submission").await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert!(response["message"]
        .as_str()
        .unwrap()
        .contains("unavailable"));
    assert!(response["draft"]["rows"].as_array().unwrap().is_empty());
    assert_eq!(usage(&app).await["total_cost_usd"], "0.000123456789");

    let stored = sqlx::query_as::<_, (String, String, i64, String)>(
        "SELECT submission_key, generation_id, cost_reported, cost_usd FROM assistant_usage",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored.0.len(), 64);
    assert_ne!(stored.0, "invalid-submission");
    assert_eq!(stored.1, "gen-invalid");
    assert_eq!(stored.2, 1);
    assert_eq!(stored.3, "0.000123456789");
    let columns: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('assistant_usage')")
            .fetch_all(&pool)
            .await
            .unwrap();
    for forbidden in [
        "prompt",
        "response",
        "header",
        "secret",
        "api_key",
        "submission_id",
    ] {
        assert!(!columns.iter().any(|column| column.contains(forbidden)));
    }
}

#[tokio::test]
async fn exact_multi_day_summary_distinguishes_zero_missing_and_malformed_cost() {
    let (app, pool, _directory, server) = setup_mock(IdentityConfig::default()).await;
    let account = create_account(&app, "Synthetic account", 100000).await;
    let costs = [
        Some("0.1"),
        Some("0.2"),
        Some("0"),
        None,
        Some("\"not-a-decimal\""),
        Some("0.000000000000000001"),
    ];
    for (index, cost) in costs.into_iter().enumerate() {
        server.push(valid_completion(
            &format!("gen-summary-{index}"),
            &format!("Synthetic row {index}"),
            account,
            cost,
        ));
        let (status, _) = prompt(&app, &format!("summary-submission-{index}")).await;
        assert_eq!(status, StatusCode::OK);
    }
    let dates = [
        "2026-11-01T10:00:00Z",
        "2026-11-01T11:00:00Z",
        "2026-11-02T10:00:00Z",
        "2026-11-02T11:00:00Z",
        "2026-11-02T12:00:00Z",
        "2026-11-03T10:00:00Z",
    ];
    for (date, id) in dates.into_iter().zip(1_i64..=6) {
        sqlx::query("UPDATE assistant_usage SET created_at = ? WHERE id = ?")
            .bind(date)
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
    }

    let summary = usage(&app).await;
    assert_eq!(summary["total_cost_usd"], "0.300000000000000001");
    assert_eq!(summary["completed_count"], 6);
    assert_eq!(summary["reported_count"], 4);
    assert_eq!(summary["zero_cost_count"], 1);
    assert_eq!(summary["missing_cost_count"], 2);
    let days = summary["days"].as_array().unwrap();
    assert_eq!(days.len(), 3);
    assert_eq!(days[0]["date"], "2026-11-01");
    assert_eq!(days[0]["total_cost_usd"], "0.3");
    assert_eq!(days[0]["completed_count"], 2);
    assert_eq!(days[0]["reported_count"], 2);
    assert_eq!(days[0]["missing_cost_count"], 0);
    assert_eq!(days[1]["total_cost_usd"], "0");
    assert_eq!(days[1]["zero_cost_count"], 1);
    assert_eq!(days[1]["missing_cost_count"], 2);
    assert_eq!(days[2]["total_cost_usd"], "0.000000000000000001");
}

#[tokio::test]
async fn replay_and_reused_generation_id_do_not_double_account() {
    let (app, pool, _directory, server) = setup_mock(IdentityConfig::default()).await;
    let account = create_account(&app, "Synthetic account", 100000).await;
    server.push(valid_completion(
        "gen-reused",
        "First synthetic row",
        account,
        Some("0.1"),
    ));
    assert_eq!(prompt(&app, "replay-submission").await.0, StatusCode::OK);
    assert_eq!(prompt(&app, "replay-submission").await.0, StatusCode::OK);
    assert_eq!(
        server.requests().len(),
        1,
        "submission replay must not call upstream again"
    );

    server.push(valid_completion(
        "gen-reused",
        "Second synthetic row",
        account,
        Some("9.9"),
    ));
    assert_eq!(prompt(&app, "different-submission").await.0, StatusCode::OK);
    assert_eq!(server.requests().len(), 2);
    let summary = usage(&app).await;
    assert_eq!(summary["completed_count"], 1);
    assert_eq!(summary["total_cost_usd"], "0.1");
    let duplicates: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM assistant_usage WHERE status = 'duplicate'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(duplicates, 1);
}

#[tokio::test]
async fn unavailable_and_billed_error_responses_have_distinct_accounting() {
    let (app, pool, _directory, server) = setup_mock(IdentityConfig::default()).await;
    let account = create_account(&app, "Synthetic account", 100000).await;
    server.push(MockResponse {
        status: StatusCode::SERVICE_UNAVAILABLE,
        body: json!({ "error": { "code": "mock_unavailable" } }),
    });
    let (status, response) = prompt(&app, "unavailable-submission").await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert!(response["message"]
        .as_str()
        .unwrap()
        .contains("unavailable"));
    assert_eq!(usage(&app).await["completed_count"], 0);
    let unavailable: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM assistant_usage WHERE status = 'unavailable'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(unavailable, 1);

    let mut billed_error = completion("gen-billed-error", "", Some("0.0003"));
    billed_error["error"] = json!({ "code": "mock_billed_error" });
    server.push(MockResponse {
        status: StatusCode::SERVICE_UNAVAILABLE,
        body: billed_error,
    });
    let (status, response) = prompt(&app, "billed-error-submission").await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert!(response["message"]
        .as_str()
        .unwrap()
        .contains("unavailable"));
    let summary = usage(&app).await;
    assert_eq!(summary["completed_count"], 1);
    assert_eq!(summary["total_cost_usd"], "0.0003");

    // Manual recovery remains possible and does not create another model call.
    let (status, _) = send_json(
        &app,
        "POST",
        "/api/draft/rows",
        json!({
            "row_id": "manual-recovery",
            "kind": "expense",
            "name": "Synthetic recovery",
            "amount_minor": 1234,
            "date": "2026-11-28",
            "account_id": account,
            "manual": true
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, current) = send_json(&app, "GET", "/api/draft", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    let (status, receipt) = send_json(
        &app,
        "POST",
        "/api/draft/save",
        json!({ "revision": current["revision"] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(state(&app).await["totals"]["expense_minor"], 1234);
    assert_eq!(server.requests().len(), 2);
}

#[tokio::test]
async fn household_usage_is_member_scoped_and_read_only() {
    let identity = IdentityConfig {
        household_enabled: true,
        shared_token: Some("test-token".to_string()),
        default_member_id: 1,
    };
    let (app, _pool, _directory, server) = setup_mock(identity).await;
    let (status, created) = send_json_as(
        &app,
        1,
        "POST",
        "/api/accounts",
        json!({ "name": "Synthetic account", "opening_balance_minor": 100000 }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let account = created["id"].as_i64().unwrap();
    server.push(valid_completion(
        "gen-member-1",
        "Member one row",
        account,
        Some("0.1"),
    ));
    server.push(valid_completion(
        "gen-member-2",
        "Member two row",
        account,
        Some("0.2"),
    ));
    let (status, _) = send_json_as(
        &app,
        1,
        "POST",
        "/api/draft/prompt",
        json!({ "text": "Synthetic member one", "client_prompt_id": "member-1", "local_date": "2026-11-28" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send_json_as(
        &app,
        2,
        "POST",
        "/api/draft/prompt",
        json!({ "text": "Synthetic member two", "client_prompt_id": "member-2", "local_date": "2026-11-28" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, first) = get_as(&app, 1, "/api/assistant/usage").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["member_id"], 1);
    assert_eq!(first["total_cost_usd"], "0.1");
    let (status, second) = get_as(&app, 2, "/api/assistant/usage").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["member_id"], 2);
    assert_eq!(second["total_cost_usd"], "0.2");

    let (status, _) = send_json(&app, "GET", "/api/assistant/usage", json!({})).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = send_json_as(
        &app,
        1,
        "POST",
        "/api/assistant/usage",
        json!({ "total_cost_usd": "999" }),
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn a_different_response_model_is_rejected_but_its_cost_is_kept() {
    let (app, _pool, _directory, server) = setup_mock(IdentityConfig::default()).await;
    let account = create_account(&app, "Synthetic account", 100000).await;
    let mut response = valid_completion(
        "gen-wrong-model",
        "Wrong model row",
        account,
        Some("0.0004"),
    );
    response.body["model"] = json!("some/other-model");
    server.push(response);
    let (status, response) = prompt(&app, "wrong-model-submission").await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert!(response["message"]
        .as_str()
        .unwrap()
        .contains("unavailable"));
    assert!(response["draft"]["rows"].as_array().unwrap().is_empty());
    assert_eq!(usage(&app).await["total_cost_usd"], "0.0004");
}
