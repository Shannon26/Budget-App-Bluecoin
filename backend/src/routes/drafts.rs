use crate::app::AppState;
use crate::error::AppResult;
use crate::models::*;
use axum::extract::{Path, State};
use axum::Json;

pub(crate) async fn load_draft(
    State(state): State<AppState>,
    member: MemberId,
) -> AppResult<Json<Draft>> {
    Ok(Json(state.store.load_draft(member.0).await?))
}

pub(crate) async fn append_prompt(
    State(state): State<AppState>,
    member: MemberId,
    Json(body): Json<PromptRequest>,
) -> AppResult<Json<DraftResponse>> {
    let draft = state
        .store
        .append_member_prompt(
            member.0,
            &body.client_prompt_id,
            &body.text,
            body.local_date.as_deref(),
        )
        .await?;
    let (account_names, category_names) = state
        .store
        .draft_context(member.0, &draft, state.identity.household_enabled)
        .await?;
    let request = AssistantRequest {
        text: body.text,
        local_date: draft.local_date.clone(),
        rows: draft.rows.clone(),
        account_names,
        category_names,
    };

    let Some(mut proposal) = state.assistant.prepare(request).await.ok() else {
        return Ok(Json(DraftResponse {
            draft,
            message: Some(
                "Assistant is unavailable. You can edit rows manually or retry.".to_string(),
            ),
            clarification: None,
        }));
    };

    let message = if proposal.message.trim().is_empty() {
        None
    } else {
        Some(proposal.message.clone())
    };
    if let Some(clarification) = proposal.clarification.take() {
        return Ok(Json(DraftResponse {
            draft,
            message,
            clarification: Some(clarification),
        }));
    }
    if proposal.rows.is_empty() && proposal.edits.is_empty() {
        return Ok(Json(DraftResponse {
            draft,
            message: Some(
                "Assistant is unavailable. You can edit rows manually or retry.".to_string(),
            ),
            clarification: None,
        }));
    }

    Ok(Json(
        state
            .store
            .apply_proposal(member.0, &body.client_prompt_id, proposal, message)
            .await?,
    ))
}

pub(crate) async fn add_row(
    State(state): State<AppState>,
    member: MemberId,
    Json(row): Json<DraftRow>,
) -> AppResult<Json<Draft>> {
    Ok(Json(state.store.upsert_row(member.0, row).await?))
}

pub(crate) async fn replace_row(
    State(state): State<AppState>,
    member: MemberId,
    Path(row_id): Path<String>,
    Json(mut row): Json<DraftRow>,
) -> AppResult<Json<Draft>> {
    row.row_id = row_id;
    Ok(Json(state.store.upsert_row(member.0, row).await?))
}

pub(crate) async fn remove_row(
    State(state): State<AppState>,
    member: MemberId,
    Path(row_id): Path<String>,
) -> AppResult<Json<Draft>> {
    Ok(Json(state.store.remove_row(member.0, &row_id).await?))
}

pub(crate) async fn assign_account(
    State(state): State<AppState>,
    member: MemberId,
    Json(body): Json<AssignAccountRequest>,
) -> AppResult<Json<Draft>> {
    Ok(Json(
        state
            .store
            .assign_account(member.0, &body.row_ids, body.account_id)
            .await?,
    ))
}

pub(crate) async fn remember_alias(
    State(state): State<AppState>,
    member: MemberId,
    Json(body): Json<AliasRequest>,
) -> AppResult<Json<OkResponse>> {
    state
        .store
        .remember_alias(member.0, &body.alias, body.account_id, body.proposed_row_id)
        .await?;
    Ok(Json(OkResponse { ok: true }))
}

pub(crate) async fn discard(
    State(state): State<AppState>,
    member: MemberId,
) -> AppResult<Json<OkResponse>> {
    state.store.discard(member.0).await?;
    Ok(Json(OkResponse { ok: true }))
}

pub(crate) async fn save(
    State(state): State<AppState>,
    member: MemberId,
    Json(body): Json<SaveRequest>,
) -> AppResult<Json<SaveReceipt>> {
    let receipt = state
        .store
        .save_reviewed(member.0, body.revision, state.identity.household_enabled)
        .await?;
    Ok(Json(receipt))
}
