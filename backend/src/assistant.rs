//! Backend wrapper for the optional OpenRouter draft assistant.
//!
//! The assistant can only propose changes to an unsaved draft. Credentials,
//! prompt text, raw model responses, and response headers are never written to
//! logs or usage records; only typed failures and operational counts are logged.

use rust_decimal::Decimal;
use serde_json::{json, Value};
use std::env;
use std::fmt;
use std::time::{Duration, Instant};
use tracing::{info, warn};

use crate::error::{AppError, AppResult};
use crate::models::{
    AssistantProposal, AssistantRequest, DraftRow, DraftRowKind, DraftRowStatus, DraftSource,
};
use crate::store::Store;

const DEFAULT_MODEL: &str = "deepseek/deepseek-v4.1-flash-1017";
const DEFAULT_PROVIDER: &str = "sail-research/fp4";
const DEFAULT_ENDPOINT: &str = "https://openrouter.ai/api/v1/chat/completions";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_REQUEST_BYTES: usize = 32 * 1024;
const MAX_NEW_ROWS: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AssistantMode {
    Disabled,
    Stub,
    OpenRouter,
}

#[derive(Clone)]
pub(crate) struct AssistantClient {
    mode: AssistantMode,
    api_key: Option<String>,
    model: String,
    endpoint: String,
    usage_store: Option<Store>,
}

/// Deliberately hand-written so the API key can never be exposed by a
/// `Debug` formatting of application state.
impl fmt::Debug for AssistantClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssistantClient")
            .field("mode", &self.mode)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("model", &self.model)
            .field("endpoint", &self.endpoint)
            .field("usage_tracking", &self.usage_store.is_some())
            .finish()
    }
}

impl Default for AssistantClient {
    fn default() -> Self {
        Self {
            mode: AssistantMode::Disabled,
            api_key: None,
            model: DEFAULT_MODEL.to_string(),
            endpoint: DEFAULT_ENDPOINT.to_string(),
            usage_store: None,
        }
    }
}

impl AssistantClient {
    /// Offline deterministic client for integration tests. Never used in the
    /// production router unless `PAISA_ASSISTANT_MODE=stub` is set explicitly.
    #[cfg(test)]
    pub(crate) fn stub() -> Self {
        Self {
            mode: AssistantMode::Stub,
            api_key: None,
            model: DEFAULT_MODEL.to_string(),
            endpoint: DEFAULT_ENDPOINT.to_string(),
            usage_store: None,
        }
    }

    /// Local HTTP endpoint used by integration tests. Never constructed by the
    /// production startup path.
    #[cfg(test)]
    pub(crate) fn openrouter_for_test(store: Store, endpoint: String) -> Self {
        Self {
            mode: AssistantMode::OpenRouter,
            api_key: Some("test-openrouter-key".to_string()),
            model: DEFAULT_MODEL.to_string(),
            endpoint,
            usage_store: Some(store),
        }
    }

    pub(crate) fn from_env(store: Store) -> Self {
        let api_key = env::var("OPENROUTER_API_KEY")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let model = env::var("OPENROUTER_MODEL")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_MODEL.to_string());
        let endpoint = env::var("OPENROUTER_URL")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_ENDPOINT.to_string());
        let requested_mode = env::var("PAISA_ASSISTANT_MODE")
            .ok()
            .map(|value| value.trim().to_ascii_lowercase())
            .unwrap_or_default();

        let mode = if requested_mode == "stub" {
            AssistantMode::Stub
        } else if api_key.is_some()
            && requested_mode != "off"
            && requested_mode != "disabled"
            && requested_mode != "none"
        {
            AssistantMode::OpenRouter
        } else {
            AssistantMode::Disabled
        };

        // Only non-secret routing metadata: never the key or endpoint URL.
        info!(
            ?mode,
            %model,
            provider = DEFAULT_PROVIDER,
            endpoint_is_default = endpoint == DEFAULT_ENDPOINT,
            "assistant configuration loaded"
        );
        Self {
            mode,
            api_key,
            model,
            endpoint,
            usage_store: Some(store),
        }
    }

    pub(crate) async fn prepare(
        &self,
        request: AssistantRequest,
        member_id: i64,
        submission_id: &str,
    ) -> AppResult<AssistantProposal> {
        match self.mode {
            AssistantMode::Disabled => {
                let mut proposal = AssistantProposal::default();
                proposal.message =
                    "Assistant is not configured. Add or edit rows manually.".to_string();
                Ok(proposal)
            }
            AssistantMode::Stub => Ok(Self::stub_proposal(&request)),
            AssistantMode::OpenRouter => {
                self.prepare_openrouter(request, member_id, submission_id)
                    .await
            }
        }
    }

    /// Offline, deterministic proposal used by tests and local development.
    fn stub_proposal(request: &AssistantRequest) -> AssistantProposal {
        let Some((amount_minor, matched)) = first_amount(&request.text) else {
            return AssistantProposal {
                message: "I could not find an amount in that prompt.".to_string(),
                ..AssistantProposal::default()
            };
        };

        let cleaned = cleaned_name(&request.text, matched);
        let mut row = DraftRow {
            row_id: "stub-1".to_string(),
            ordinal: 1,
            kind: DraftRowKind::Expense,
            name: cleaned,
            amount_minor: Some(amount_minor),
            date: Some(request.local_date.clone()),
            source: DraftSource::Assistant,
            manual: false,
            status: DraftRowStatus::Ready,
            ..DraftRow::default()
        };
        // An unset account is reported as an issue, so the row needs review
        // before it can be saved.
        row.refresh_issues();

        AssistantProposal {
            rows: vec![row],
            message: "Prepared 1 row. Choose an account before saving.".to_string(),
            ..AssistantProposal::default()
        }
    }

    async fn prepare_openrouter(
        &self,
        request: AssistantRequest,
        member_id: i64,
        submission_id: &str,
    ) -> AppResult<AssistantProposal> {
        let store = self.usage_store.as_ref().ok_or_else(unavailable)?;
        let claimed = store
            .claim_assistant_attempt(member_id, submission_id, &self.model, DEFAULT_PROVIDER)
            .await
            .map_err(|_| {
                warn!(
                    failure_kind = "usage_claim",
                    "assistant usage could not be claimed"
                );
                unavailable()
            })?;
        if !claimed {
            // The same member submission already made its one upstream attempt.
            return Err(unavailable());
        }

        let result = self
            .prepare_openrouter_claimed(request, member_id, submission_id)
            .await;
        if result.is_err() {
            // Completed billed responses keep their usage row; only a still-
            // pending attempt becomes unavailable.
            let _ = store
                .mark_assistant_unavailable(member_id, submission_id)
                .await;
        }
        result
    }

    async fn prepare_openrouter_claimed(
        &self,
        request: AssistantRequest,
        member_id: i64,
        submission_id: &str,
    ) -> AppResult<AssistantProposal> {
        let api_key = self.api_key.as_deref().ok_or_else(unavailable)?;
        let serialized_request = serde_json::to_string(&request).map_err(|_| invalid_proposal())?;
        if serialized_request.len() > MAX_REQUEST_BYTES {
            return Err(AppError::BadRequest(
                "prompt is too large; shorten it and try again".to_string(),
            ));
        }

        let body = json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": SYSTEM_PROMPT,
                },
                {
                    "role": "user",
                    "content": serialized_request,
                },
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "draft_proposal",
                    "strict": true,
                    "schema": proposal_schema(),
                },
            },
            "max_tokens": 2000,
            "provider": {
                "order": [DEFAULT_PROVIDER],
                "allow_fallbacks": false,
                "require_parameters": true,
            },
        });
        let serialized_body = serde_json::to_string(&body).map_err(|_| invalid_proposal())?;

        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|_| unavailable())?;

        let started = Instant::now();
        let response = client
            .post(&self.endpoint)
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .body(serialized_body)
            .send()
            .await
            .map_err(|error| {
                let failure_kind = if error.is_timeout() {
                    "timeout"
                } else if error.is_connect() {
                    "connect"
                } else {
                    "request"
                };
                warn!(
                    duration_ms = started.elapsed().as_millis() as u64,
                    model = %self.model,
                    failure_kind,
                    "assistant request failed"
                );
                unavailable()
            })?;

        let duration_ms = started.elapsed().as_millis() as u64;
        let status = response.status();
        let payload: Value = response.json().await.map_err(|_| {
            warn!(
                duration_ms,
                status = status.as_u16(),
                model = %self.model,
                failure_kind = "decode",
                "assistant response could not be decoded"
            );
            invalid_proposal()
        })?;

        let store = self.usage_store.as_ref().ok_or_else(unavailable)?;
        let generation_id = payload.get("id").and_then(Value::as_str).filter(|value| {
            value.len() <= 200
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_:".contains(&byte))
        });
        let cost = reported_cost(payload.get("usage").and_then(|usage| usage.get("cost")));
        let has_usage = payload.get("usage").is_some_and(Value::is_object);
        if status.is_success() || has_usage {
            store
                .complete_assistant_attempt(member_id, submission_id, generation_id, cost)
                .await
                .map_err(|_| {
                    warn!(
                        duration_ms,
                        status = status.as_u16(),
                        model = %self.model,
                        failure_kind = "usage_persistence",
                        "assistant usage could not be persisted"
                    );
                    unavailable()
                })?;
        }

        if !status.is_success() {
            warn!(
                duration_ms,
                status = status.as_u16(),
                model = %self.model,
                failure_kind = "http_status",
                "assistant returned an unsuccessful status"
            );
            return Err(unavailable());
        }
        if payload
            .get("model")
            .is_some_and(|model| model.as_str() != Some(self.model.as_str()))
        {
            warn!(
                duration_ms,
                status = status.as_u16(),
                model = %self.model,
                failure_kind = "model_mismatch",
                "assistant returned a different model"
            );
            return Err(unavailable());
        }

        let content = payload
            .get("choices")
            .and_then(|choices| choices.get(0))
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(|content| content.as_str())
            .ok_or_else(|| {
                warn!(
                    duration_ms,
                    status = status.as_u16(),
                    model = %self.model,
                    failure_kind = "invalid_proposal",
                    "assistant proposal was invalid"
                );
                invalid_proposal()
            })?;

        let mut proposal: AssistantProposal = serde_json::from_str(content).map_err(|_| {
            warn!(
                duration_ms,
                status = status.as_u16(),
                model = %self.model,
                failure_kind = "invalid_proposal",
                "assistant proposal was invalid"
            );
            invalid_proposal()
        })?;
        if proposal.rows.len() > MAX_NEW_ROWS
            || proposal
                .edits
                .iter()
                .any(|edit| edit.row_id.trim().is_empty())
        {
            warn!(
                duration_ms,
                status = status.as_u16(),
                model = %self.model,
                failure_kind = "invalid_proposal",
                "assistant proposal was invalid"
            );
            return Err(invalid_proposal());
        }
        for row in proposal
            .rows
            .iter_mut()
            .chain(proposal.edits.iter_mut().map(|edit| &mut edit.row))
        {
            row.refresh_issues();
        }

        let response_provider = payload
            .get("provider")
            .and_then(Value::as_str)
            .filter(|value| {
                value.len() <= 80
                    && value
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b" _-./".contains(&byte))
            });
        info!(
            duration_ms,
            status = status.as_u16(),
            model = %self.model,
            response_provider = ?response_provider,
            cost_reported = cost.is_some(),
            new_rows = proposal.rows.len(),
            edits = proposal.edits.len(),
            "assistant proposal prepared"
        );
        Ok(proposal)
    }
}

fn unavailable() -> AppError {
    AppError::BadRequest("assistant is unavailable; retry or edit manually".to_string())
}

fn invalid_proposal() -> AppError {
    AppError::BadRequest("assistant returned an invalid proposal".to_string())
}

/// Parse the response's reported decimal without passing through `f64`.
/// Missing, malformed, negative, or unrepresentable values stay "not reported".
fn reported_cost(value: Option<&Value>) -> Option<Decimal> {
    let raw = match value? {
        Value::String(value) => value.as_str(),
        Value::Number(value) => {
            return Decimal::from_str_exact(&value.to_string())
                .or_else(|_| Decimal::from_scientific(&value.to_string()))
                .ok()
                .filter(|cost| *cost >= Decimal::ZERO)
        }
        _ => return None,
    };
    Decimal::from_str_exact(raw)
        .or_else(|_| Decimal::from_scientific(raw))
        .ok()
        .filter(|cost| *cost >= Decimal::ZERO)
}

const SYSTEM_PROMPT: &str =
    "You are the Paisa draft assistant. You may only propose changes to an \
    unsaved draft: append new draft rows or replace existing rows by their stable row_id. You must \
    never post to the ledger, approve transfers, or change saved records. The user message is an \
    allowlisted JSON object containing the submitted text, the local date, current unsaved rows, \
    and permitted saved account and category names. Resolve references using only that context; \
    ask for clarification instead of guessing when an account, date, amount, or row reference is \
    ambiguous. Return only JSON that matches the supplied strict schema. Use snake_case row kinds \
    (expense, income, transfer, new_account) and statuses (ready, needs_input), positive integer \
    paise for amounts, and YYYY-MM-DD dates. Set manual=false and source=assistant for new rows, \
    and preserve manual=true and existing row_id and ordinal fields on edits.";

fn draft_row_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "row_id": { "type": "string" },
            "ordinal": { "type": "integer" },
            "kind": { "type": "string", "enum": ["expense", "income", "transfer", "new_account"] },
            "name": { "type": "string" },
            "amount_minor": { "type": ["integer", "null"] },
            "date": { "type": ["string", "null"] },
            "account_id": { "type": ["integer", "null"] },
            "account_name": { "type": ["string", "null"] },
            "to_account_id": { "type": ["integer", "null"] },
            "to_account_name": { "type": ["string", "null"] },
            "category_id": { "type": ["integer", "null"] },
            "note": { "type": "string" },
            "status": { "type": "string", "enum": ["ready", "needs_input"] },
            "issues": { "type": "array", "items": { "type": "string" } },
            "manual": { "type": "boolean" },
            "source": { "type": "string", "enum": ["manual", "assistant"] },
            "proposed_row_id": { "type": ["string", "null"] },
            "account_type": { "type": ["string", "null"] },
            "opening_balance_minor": { "type": ["integer", "null"] },
            "current_balance_minor": { "type": ["integer", "null"] },
            "transfer_request": { "type": "boolean" }
        },
        "required": [
            "row_id", "ordinal", "kind", "name", "amount_minor", "date", "account_id",
            "account_name", "to_account_id", "to_account_name", "category_id", "note",
            "status", "issues", "manual", "source", "proposed_row_id", "account_type",
            "opening_balance_minor", "current_balance_minor", "transfer_request"
        ]
    })
}

fn proposal_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "rows": { "type": "array", "items": { "$ref": "#/$defs/draft_row" } },
            "edits": { "type": "array", "items": { "$ref": "#/$defs/row_edit" } },
            "message": { "type": "string" },
            "clarification": { "type": ["string", "null"] }
        },
        "required": ["rows", "edits", "message", "clarification"],
        "$defs": {
            "draft_row": draft_row_schema(),
            "row_edit": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "row_id": { "type": "string" },
                    "row": { "$ref": "#/$defs/draft_row" }
                },
                "required": ["row_id", "row"]
            }
        }
    })
}

/// Find the first rupee amount in the prompt and return it in integer paise
/// together with the matched source span (used to clean the row name).
fn first_amount(text: &str) -> Option<(i64, (usize, usize))> {
    for (token_start, digits_start) in amount_tokens(text) {
        if let Some((amount, end)) = parse_amount(&text[digits_start..]) {
            if amount > 0 {
                return Some((amount, (token_start, digits_start + end)));
            }
        }
    }
    None
}

/// Yield (token start, start of the numeric part) for `₹...` and `Rs ...`
/// occurrences, in reading order.
fn amount_tokens(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'R' || bytes[index] == b'r' {
            if index + 1 < bytes.len() && (bytes[index + 1] == b's' || bytes[index + 1] == b'S') {
                let boundary = index == 0
                    || !(bytes[index - 1].is_ascii_alphanumeric() || bytes[index - 1] == b'_');
                if boundary {
                    let mut cursor = index + 2;
                    if cursor < bytes.len() && bytes[cursor] == b'.' {
                        cursor += 1;
                    }
                    while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
                        cursor += 1;
                    }
                    tokens.push((index, cursor));
                    index += 2;
                    continue;
                }
            }
        }
        if text.is_char_boundary(index) && text[index..].starts_with('₹') {
            let mut cursor = index + '₹'.len_utf8();
            while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            tokens.push((index, cursor));
            index += '₹'.len_utf8();
            continue;
        }
        index += 1;
    }
    tokens
}

/// Parse digits (with optional grouping commas and up to two decimal places)
/// from the start of a string, returning integer paise and bytes consumed.
fn parse_amount(input: &str) -> Option<(i64, usize)> {
    let bytes = input.as_bytes();
    let mut index = 0;
    let mut paise: i64 = 0;
    let mut saw_digit = false;
    while index < bytes.len() {
        match bytes[index] {
            b'0'..=b'9' => {
                paise = paise
                    .checked_mul(10)?
                    .checked_add((bytes[index] - b'0') as i64)?;
                saw_digit = true;
                index += 1;
            }
            b',' => index += 1,
            _ => break,
        }
    }
    if !saw_digit {
        return None;
    }
    paise = paise.checked_mul(100)?;
    if index < bytes.len() && bytes[index] == b'.' {
        let mut fraction = 0i64;
        let mut places = 0u32;
        let mut cursor = index + 1;
        while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
            if places < 2 {
                fraction = fraction * 10 + (bytes[cursor] - b'0') as i64;
                places += 1;
            }
            cursor += 1;
        }
        if places == 1 {
            fraction *= 10;
        }
        paise = paise.checked_add(fraction)?;
        index = cursor;
    }
    Some((paise, index))
}

fn cleaned_name(text: &str, matched: (usize, usize)) -> String {
    let (start, end) = matched;
    let without_amount = format!("{} {}", &text[..start], &text[end..]);
    let collapsed = without_amount
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let name: String = collapsed.chars().take(80).collect();
    if name.trim().is_empty() {
        "Expense".to_string()
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_route_is_the_requested_version_and_provider() {
        let client = AssistantClient::default();
        assert_eq!(client.model, "deepseek/deepseek-v4.1-flash-1017");
        assert_eq!(DEFAULT_PROVIDER, "sail-research/fp4");
        assert_eq!(
            client.endpoint,
            "https://openrouter.ai/api/v1/chat/completions"
        );
    }

    #[test]
    fn reported_cost_preserves_decimal_digits_and_separates_missing_values() {
        let precise: Value = serde_json::from_str(r#"{"cost":0.300000000000000001}"#).unwrap();
        assert_eq!(
            reported_cost(precise.get("cost")).unwrap().to_string(),
            "0.300000000000000001"
        );
        let zero: Value = serde_json::from_str(r#"{"cost":0}"#).unwrap();
        assert_eq!(reported_cost(zero.get("cost")).unwrap().to_string(), "0");
        let negative: Value = serde_json::from_str(r#"{"cost":-1}"#).unwrap();
        assert!(reported_cost(negative.get("cost")).is_none());
        let missing: Value = serde_json::from_str(r#"{"usage":{}}"#).unwrap();
        assert!(reported_cost(missing.get("usage").unwrap().get("cost")).is_none());
    }
}
