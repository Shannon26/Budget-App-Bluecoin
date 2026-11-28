//! Private, member-scoped drafts and the atomic reviewed batch save.

use super::Store;
use crate::date::{date_only, is_valid_calendar_date};
use crate::error::{AppError, AppResult};
use crate::models::*;
use chrono::Utc;
use serde_json::Value;
use sqlx::{Executor, Sqlite};
use std::collections::{HashMap, HashSet};

fn now_string() -> String {
    Utc::now().to_rfc3339()
}

fn today_string() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}

fn status_str(status: DraftRowStatus) -> &'static str {
    match status {
        DraftRowStatus::Ready => "ready",
        DraftRowStatus::NeedsInput => "needs_input",
    }
}

fn decode_row(json: &str) -> AppResult<DraftRow> {
    serde_json::from_str(json)
        .map_err(|_| AppError::Conflict("a stored draft row is invalid".to_string()))
}

fn decode_receipt(json: &str) -> AppResult<SaveReceipt> {
    let value: Value = serde_json::from_str(json)
        .map_err(|_| AppError::Conflict("a stored save receipt is invalid".to_string()))?;
    let invalid = || AppError::Conflict("a stored save receipt is invalid".to_string());
    let text = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(invalid)
    };
    let number = |key: &str| value.get(key).and_then(Value::as_i64).ok_or_else(invalid);
    let ids = |key: &str| -> AppResult<Vec<i64>> {
        value
            .get(key)
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(Value::as_i64).collect())
            .ok_or_else(invalid)
    };
    Ok(SaveReceipt {
        batch_id: text("batch_id")?,
        member_id: number("member_id")?,
        revision: number("revision")?,
        accounts_created: ids("accounts_created")?,
        entry_ids: ids("entry_ids")?,
        transfer_ids: ids("transfer_ids")?,
        pending_request_ids: ids("pending_request_ids")?,
        created_at: text("created_at")?,
    })
}

fn encode_json(value: &impl serde::Serialize) -> AppResult<String> {
    serde_json::to_string(value)
        .map_err(|_| AppError::Conflict("draft data could not be serialized".to_string()))
}

fn names_match(left: Option<&str>, right: &str) -> bool {
    left.is_some_and(|value| value.trim().eq_ignore_ascii_case(right.trim()))
}

fn source_uses_new_account(row: &DraftRow, new_row_id: &str, new_name: &str) -> bool {
    row.proposed_row_id.as_deref() == Some(new_row_id)
        || names_match(row.account_name.as_deref(), new_name)
}

fn destination_uses_new_account(row: &DraftRow, new_name: &str) -> bool {
    names_match(row.to_account_name.as_deref(), new_name)
}

impl Store {
    async fn ensure_draft_in<'e, E>(executor: E, member_id: i64, local_date: &str) -> AppResult<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query(
            "INSERT OR IGNORE INTO drafts (member_id, revision, local_date, updated_at)
             VALUES (?, 0, ?, ?)",
        )
        .bind(member_id)
        .bind(local_date)
        .bind(now_string())
        .execute(executor)
        .await?;
        Ok(())
    }

    async fn bump_revision_in<'e, E>(executor: E, member_id: i64) -> AppResult<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query(
            "UPDATE drafts SET revision = revision + 1, updated_at = ? WHERE member_id = ?",
        )
        .bind(now_string())
        .bind(member_id)
        .execute(executor)
        .await?;
        Ok(())
    }

    async fn draft_meta_in<'e, E>(
        executor: E,
        member_id: i64,
    ) -> AppResult<Option<(i64, String, String)>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query_as::<_, (i64, String, String)>(
            "SELECT revision, local_date, updated_at FROM drafts WHERE member_id = ?",
        )
        .bind(member_id)
        .fetch_optional(executor)
        .await
        .map_err(AppError::from)
    }

    async fn draft_rows_in<'e, E>(executor: E, member_id: i64) -> AppResult<Vec<DraftRow>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let stored = sqlx::query_as::<_, (String, i64, String)>(
            "SELECT row_id, ordinal, data_json FROM draft_rows
             WHERE member_id = ? ORDER BY ordinal, row_id",
        )
        .bind(member_id)
        .fetch_all(executor)
        .await?;
        stored
            .into_iter()
            .map(|(row_id, ordinal, data_json)| {
                let mut row = decode_row(&data_json)?;
                row.row_id = row_id;
                row.ordinal = ordinal;
                Ok(row)
            })
            .collect()
    }

    async fn draft_prompts_in<'e, E>(executor: E, member_id: i64) -> AppResult<Vec<DraftPrompt>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let stored = sqlx::query_as::<_, (String, i64, String, String, String)>(
            "SELECT submission_id, seq, role, text, created_at FROM draft_prompts
             WHERE member_id = ? ORDER BY seq, created_at",
        )
        .bind(member_id)
        .fetch_all(executor)
        .await?;
        Ok(stored
            .into_iter()
            .map(|(submission_id, seq, role, text, created_at)| DraftPrompt {
                submission_id,
                seq,
                role,
                text,
                created_at,
            })
            .collect())
    }

    fn draft_from_parts(
        member_id: i64,
        meta: Option<(i64, String, String)>,
        rows: Vec<DraftRow>,
        prompts: Vec<DraftPrompt>,
    ) -> Draft {
        match meta {
            Some((revision, local_date, updated_at)) => Draft {
                member_id,
                revision,
                local_date,
                updated_at,
                rows,
                prompts,
            },
            None => Draft {
                member_id,
                revision: 0,
                local_date: today_string(),
                updated_at: now_string(),
                rows,
                prompts,
            },
        }
    }

    async fn read_draft(&self, member_id: i64) -> AppResult<Draft> {
        let meta = Self::draft_meta_in(&self.pool, member_id).await?;
        let rows = Self::draft_rows_in(&self.pool, member_id).await?;
        let prompts = Self::draft_prompts_in(&self.pool, member_id).await?;
        Ok(Self::draft_from_parts(member_id, meta, rows, prompts))
    }

    async fn write_row_in<'e, E>(
        executor: E,
        member_id: i64,
        row: &DraftRow,
        now: &str,
    ) -> AppResult<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query(
            "INSERT INTO draft_rows
                (member_id, row_id, ordinal, kind, data_json, status, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(member_id, row_id) DO UPDATE SET
                ordinal = excluded.ordinal,
                kind = excluded.kind,
                data_json = excluded.data_json,
                status = excluded.status,
                updated_at = excluded.updated_at",
        )
        .bind(member_id)
        .bind(&row.row_id)
        .bind(row.ordinal)
        .bind(row.kind.as_str())
        .bind(encode_json(row)?)
        .bind(status_str(row.status))
        .bind(now)
        .execute(executor)
        .await?;
        Ok(())
    }

    async fn insert_prompt_in<'e, E>(
        executor: E,
        member_id: i64,
        submission_id: &str,
        seq: i64,
        role: &str,
        text: &str,
    ) -> AppResult<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query(
            "INSERT OR IGNORE INTO draft_prompts
                (member_id, submission_id, seq, role, text, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(member_id)
        .bind(submission_id)
        .bind(seq)
        .bind(role)
        .bind(text)
        .bind(now_string())
        .execute(executor)
        .await?;
        Ok(())
    }

    /// Always returns a draft. A member without one gets a fresh revision-0
    /// draft dated with the current calendar day.
    pub async fn load_draft(&self, member_id: i64) -> AppResult<Draft> {
        let local_date = today_string();
        Self::ensure_draft_in(&self.pool, member_id, &local_date).await?;
        self.read_draft(member_id).await
    }

    /// Insert or replace one row by `(member_id, row_id)` and bump the revision.
    pub async fn upsert_row(&self, member_id: i64, mut row: DraftRow) -> AppResult<Draft> {
        let local_date = today_string();
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        Self::ensure_draft_in(&mut *tx, member_id, &local_date).await?;

        if row.ordinal == 0 {
            let existing: Option<i64> = sqlx::query_scalar(
                "SELECT ordinal FROM draft_rows WHERE member_id = ? AND row_id = ?",
            )
            .bind(member_id)
            .bind(&row.row_id)
            .fetch_optional(&mut *tx)
            .await?;
            row.ordinal = match existing {
                Some(ordinal) => ordinal,
                None => {
                    sqlx::query_scalar::<_, Option<i64>>(
                        "SELECT MAX(ordinal) FROM draft_rows WHERE member_id = ?",
                    )
                    .bind(member_id)
                    .fetch_one(&mut *tx)
                    .await?
                    .unwrap_or(0)
                        + 1
                }
            };
        }
        row.refresh_issues();
        Self::write_row_in(&mut *tx, member_id, &row, &now_string()).await?;
        Self::bump_revision_in(&mut *tx, member_id).await?;

        let meta = Self::draft_meta_in(&mut *tx, member_id).await?;
        let rows = Self::draft_rows_in(&mut *tx, member_id).await?;
        let prompts = Self::draft_prompts_in(&mut *tx, member_id).await?;
        let draft = Self::draft_from_parts(member_id, meta, rows, prompts);
        tx.commit().await?;
        Ok(draft)
    }

    pub async fn remove_row(&self, member_id: i64, row_id: &str) -> AppResult<Draft> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let result = sqlx::query("DELETE FROM draft_rows WHERE member_id = ? AND row_id = ?")
            .bind(member_id)
            .bind(row_id)
            .execute(&mut *tx)
            .await?;
        if result.rows_affected() > 0 {
            Self::bump_revision_in(&mut *tx, member_id).await?;
        }
        let meta = Self::draft_meta_in(&mut *tx, member_id).await?;
        let rows = Self::draft_rows_in(&mut *tx, member_id).await?;
        let prompts = Self::draft_prompts_in(&mut *tx, member_id).await?;
        let draft = Self::draft_from_parts(member_id, meta, rows, prompts);
        tx.commit().await?;
        Ok(draft)
    }

    /// Assign one existing account to every named row. Other row fields and
    /// other rows are left untouched.
    pub async fn assign_account(
        &self,
        member_id: i64,
        row_ids: &[String],
        account_id: i64,
    ) -> AppResult<Draft> {
        Store::load_account(&self.pool, account_id).await?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let wanted: HashSet<&str> = row_ids.iter().map(String::as_str).collect();
        let now = now_string();
        let mut rows = Self::draft_rows_in(&mut *tx, member_id).await?;
        for row in &mut rows {
            if wanted.contains(row.row_id.as_str()) {
                row.account_id = Some(account_id);
                row.account_name = None;
                row.proposed_row_id = None;
                row.refresh_issues();
                Self::write_row_in(&mut *tx, member_id, row, &now).await?;
            }
        }
        Self::bump_revision_in(&mut *tx, member_id).await?;
        let meta = Self::draft_meta_in(&mut *tx, member_id).await?;
        let prompts = Self::draft_prompts_in(&mut *tx, member_id).await?;
        let draft = Self::draft_from_parts(member_id, meta, rows, prompts);
        tx.commit().await?;
        Ok(draft)
    }

    /// Persist a personal nickname only after the member made an explicit
    /// choice. Exactly one target must be supplied.
    pub async fn remember_alias(
        &self,
        member_id: i64,
        alias: &str,
        account_id: Option<i64>,
        proposed_row_id: Option<String>,
    ) -> AppResult<()> {
        if account_id.is_some() == proposed_row_id.is_some() {
            return Err(AppError::BadRequest(
                "choose exactly one account or proposed account".to_string(),
            ));
        }
        let alias = alias.trim();
        if alias.is_empty() {
            return Err(AppError::BadRequest("alias is required".to_string()));
        }
        if let Some(account_id) = account_id {
            Store::load_account(&self.pool, account_id).await?;
        }
        sqlx::query(
            "INSERT INTO member_aliases (member_id, alias, account_id, proposed_row_id, created_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(member_id, alias) DO UPDATE SET
                account_id = excluded.account_id,
                proposed_row_id = excluded.proposed_row_id",
        )
        .bind(member_id)
        .bind(alias)
        .bind(account_id)
        .bind(proposed_row_id)
        .bind(now_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Record a member submission once per `(member_id, submission_id)`.
    pub async fn append_member_prompt(
        &self,
        member_id: i64,
        submission_id: &str,
        text: &str,
        local_date: Option<&str>,
    ) -> AppResult<Draft> {
        let local_date = local_date
            .and_then(date_only)
            .filter(|value| is_valid_calendar_date(value))
            .unwrap_or_else(today_string);
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        Self::ensure_draft_in(&mut *tx, member_id, &local_date).await?;
        let existing = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM draft_prompts WHERE member_id = ? AND submission_id = ?",
        )
        .bind(member_id)
        .bind(submission_id)
        .fetch_one(&mut *tx)
        .await?;
        if existing == 0 {
            Self::insert_prompt_in(&mut *tx, member_id, submission_id, 1, "member", text).await?;
            Self::bump_revision_in(&mut *tx, member_id).await?;
        }
        let meta = Self::draft_meta_in(&mut *tx, member_id).await?;
        let rows = Self::draft_rows_in(&mut *tx, member_id).await?;
        let prompts = Self::draft_prompts_in(&mut *tx, member_id).await?;
        let draft = Self::draft_from_parts(member_id, meta, rows, prompts);
        tx.commit().await?;
        Ok(draft)
    }

    /// Apply a proposal without disturbing rows that were not explicitly named
    /// in `edits`. Follow-ups are idempotent per submission ID.
    pub async fn apply_proposal(
        &self,
        member_id: i64,
        submission_id: &str,
        proposal: AssistantProposal,
        message: Option<String>,
    ) -> AppResult<DraftResponse> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        Self::ensure_draft_in(&mut *tx, member_id, &today_string()).await?;

        let recorded = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM draft_prompts
             WHERE member_id = ? AND submission_id = ? AND role = 'assistant'",
        )
        .bind(member_id)
        .bind(submission_id)
        .fetch_one(&mut *tx)
        .await?;
        if recorded > 0 {
            let meta = Self::draft_meta_in(&mut *tx, member_id).await?;
            let rows = Self::draft_rows_in(&mut *tx, member_id).await?;
            let prompts = Self::draft_prompts_in(&mut *tx, member_id).await?;
            tx.commit().await?;
            return Ok(DraftResponse {
                draft: Self::draft_from_parts(member_id, meta, rows, prompts),
                message: None,
                clarification: None,
            });
        }

        let mut rows = Self::draft_rows_in(&mut *tx, member_id).await?;
        let edited: HashSet<&str> = proposal
            .edits
            .iter()
            .map(|edit| edit.row_id.as_str())
            .collect();
        let mut by_id: HashMap<String, usize> = rows
            .iter()
            .enumerate()
            .map(|(index, row)| (row.row_id.clone(), index))
            .collect();

        for edit in &proposal.edits {
            let Some(index) = by_id.get(&edit.row_id).copied() else {
                continue;
            };
            let mut replacement = edit.row.clone();
            replacement.row_id = rows[index].row_id.clone();
            replacement.ordinal = rows[index].ordinal;
            replacement.source = DraftSource::Assistant;
            replacement.manual = false;
            replacement.refresh_issues();
            rows[index] = replacement;
        }

        // Stale issues from an earlier proposal must not linger on rows this
        // follow-up did not mention. The save path recomputes issues anyway.
        for row in &mut rows {
            if !edited.contains(row.row_id.as_str()) {
                row.issues.clear();
                row.status = DraftRowStatus::Ready;
            }
        }

        let mut next_ordinal = rows.iter().map(|row| row.ordinal).max().unwrap_or(0);
        let mut taken: HashSet<String> = by_id.keys().cloned().collect();
        for (index, mut row) in proposal.rows.into_iter().enumerate() {
            let mut row_id = if row.row_id.trim().is_empty() {
                format!("asst:{submission_id}:{index}")
            } else {
                format!("asst:{submission_id}:{}:{}", index, row.row_id)
            };
            while taken.contains(&row_id) {
                row_id.push('x');
            }
            next_ordinal += 1;
            row.row_id = row_id.clone();
            row.ordinal = next_ordinal;
            row.source = DraftSource::Assistant;
            row.manual = false;
            row.refresh_issues();
            by_id.insert(row_id.clone(), rows.len());
            taken.insert(row_id);
            rows.push(row);
        }

        let now = now_string();
        for row in &rows {
            Self::write_row_in(&mut *tx, member_id, row, &now).await?;
        }
        let assistant_text = message.as_deref().unwrap_or_default();
        Self::insert_prompt_in(
            &mut *tx,
            member_id,
            submission_id,
            2,
            "assistant",
            assistant_text,
        )
        .await?;
        Self::bump_revision_in(&mut *tx, member_id).await?;

        let meta = Self::draft_meta_in(&mut *tx, member_id).await?;
        let prompts = Self::draft_prompts_in(&mut *tx, member_id).await?;
        let draft = Self::draft_from_parts(member_id, meta, rows, prompts);
        tx.commit().await?;
        Ok(DraftResponse {
            draft,
            message: message.filter(|value| !value.trim().is_empty()),
            clarification: None,
        })
    }

    pub async fn discard(&self, member_id: i64) -> AppResult<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::query("DELETE FROM draft_rows WHERE member_id = ?")
            .bind(member_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM draft_prompts WHERE member_id = ?")
            .bind(member_id)
            .execute(&mut *tx)
            .await?;
        // Keep the revision monotonic. Save receipts are keyed by member and
        // revision and intentionally survive draft clearance, so resetting the
        // draft to revision zero would let a later draft replay an old receipt.
        sqlx::query(
            "UPDATE drafts SET revision = revision + 1, local_date = ?, updated_at = ? WHERE member_id = ?",
        )
        .bind(today_string())
        .bind(now_string())
        .bind(member_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Commit a reviewed revision atomically. A repeat of the same
    /// `(member_id, revision)` returns the original receipt without posting.
    pub async fn save_reviewed(
        &self,
        member_id: i64,
        revision: i64,
        enforce: bool,
    ) -> AppResult<SaveReceipt> {
        let save_key = format!("{member_id}:{revision}");
        if let Some(json) = sqlx::query_scalar::<_, String>(
            "SELECT result_json FROM save_receipts WHERE save_key = ?",
        )
        .bind(&save_key)
        .fetch_optional(&self.pool)
        .await?
        {
            return decode_receipt(&json);
        }

        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        if let Some(json) = sqlx::query_scalar::<_, String>(
            "SELECT result_json FROM save_receipts WHERE save_key = ?",
        )
        .bind(&save_key)
        .fetch_optional(&mut *tx)
        .await?
        {
            return decode_receipt(&json);
        }

        let meta = Self::draft_meta_in(&mut *tx, member_id).await?;
        if meta.is_none() {
            return Err(AppError::BadRequest(
                "draft changed; review again".to_string(),
            ));
        }
        let mut draft = Self::draft_from_parts(
            member_id,
            meta,
            Self::draft_rows_in(&mut *tx, member_id).await?,
            Self::draft_prompts_in(&mut *tx, member_id).await?,
        );
        if draft.revision != revision {
            return Err(AppError::BadRequest(
                "draft changed; review again".to_string(),
            ));
        }

        for row in &mut draft.rows {
            row.refresh_issues();
        }
        let structural: Vec<String> = draft
            .rows
            .iter()
            .filter(|row| row.status == DraftRowStatus::NeedsInput)
            .flat_map(|row| {
                row.issues
                    .iter()
                    .map(|issue| format!("{}: {}", row.row_id, issue))
                    .collect::<Vec<_>>()
            })
            .collect();
        if !structural.is_empty() {
            return Err(AppError::BadRequest(format!(
                "draft has unresolved issues: {}",
                structural.join("; ")
            )));
        }

        let new_account_rows: Vec<DraftRow> = draft
            .rows
            .iter()
            .filter(|row| row.kind == DraftRowKind::NewAccount)
            .cloned()
            .collect();
        let mut proposed_ids = HashMap::<String, i64>::new();
        let mut accounts_created = Vec::new();

        for new_row in &new_account_rows {
            let opening = match (new_row.opening_balance_minor, new_row.current_balance_minor) {
                (Some(opening), _) => opening,
                (None, Some(current)) => {
                    let mut balance = i128::from(current);
                    for activity in &draft.rows {
                        if activity.row_id == new_row.row_id || activity.transfer_request {
                            continue;
                        }
                        let amount = i128::from(activity.amount_minor.unwrap_or(0));
                        match activity.kind {
                            // Opening = stated current - signed effects. An
                            // expense/transfer_out reduces the balance, so it
                            // must be added back when walking backwards.
                            DraftRowKind::Expense => {
                                if source_uses_new_account(activity, &new_row.row_id, &new_row.name)
                                {
                                    balance += amount;
                                }
                            }
                            DraftRowKind::Income => {
                                if source_uses_new_account(activity, &new_row.row_id, &new_row.name)
                                {
                                    balance -= amount;
                                }
                            }
                            DraftRowKind::Transfer => {
                                if source_uses_new_account(activity, &new_row.row_id, &new_row.name)
                                {
                                    balance += amount;
                                }
                                if destination_uses_new_account(activity, &new_row.name) {
                                    balance -= amount;
                                }
                            }
                            DraftRowKind::NewAccount => {}
                        }
                    }
                    Self::checked_i64(balance, "opening balance")?
                }
                (None, None) => {
                    return Err(AppError::BadRequest(format!(
                        "{}: add an opening or current balance",
                        new_row.row_id
                    )))
                }
            };

            let account = Self::insert_account(
                &mut *tx,
                CreateAccountRequest {
                    name: new_row.name.clone(),
                    currency: "INR".to_string(),
                    opening_balance_minor: opening,
                    account_type: new_row.account_type.clone(),
                    note: Some(new_row.note.clone()),
                    opening_date: new_row.date.clone(),
                    hide_from_selection: false,
                    hide_from_reports: false,
                    credit_limit_minor: None,
                    cutoff_day: None,
                    due_day: None,
                },
                Some(member_id),
            )
            .await?;
            proposed_ids.insert(new_row.row_id.clone(), account.id);
            accounts_created.push(account.id);
        }

        let account_rows = sqlx::query_as::<_, (i64, String)>("SELECT id, name FROM accounts")
            .fetch_all(&mut *tx)
            .await?;
        let mut by_name = HashMap::<String, Vec<i64>>::new();
        for (id, name) in account_rows {
            by_name
                .entry(name.trim().to_lowercase())
                .or_default()
                .push(id);
        }

        let mut unresolved = Vec::new();
        let mut resolved = HashMap::<String, i64>::new();
        for row in &draft.rows {
            if row.kind == DraftRowKind::NewAccount {
                continue;
            }
            let source = Self::resolve_draft_account(
                row.account_id,
                row.proposed_row_id.as_deref(),
                row.account_name.as_deref(),
                &proposed_ids,
                &by_name,
                &format!("{}: sending account", row.row_id),
                &mut unresolved,
            );
            let destination = if row.kind == DraftRowKind::Transfer {
                Some(Self::resolve_draft_account(
                    row.to_account_id,
                    None,
                    row.to_account_name.as_deref(),
                    &proposed_ids,
                    &by_name,
                    &format!("{}: receiving account", row.row_id),
                    &mut unresolved,
                ))
            } else {
                None
            };
            if let Some(source) = source {
                resolved.insert(row.row_id.clone(), source);
            }
            if let Some(Some(id)) = destination {
                resolved.insert(format!("{}:to", row.row_id), id);
            }
        }
        if !unresolved.is_empty() {
            return Err(AppError::BadRequest(format!(
                "draft has unresolved issues: {}",
                unresolved.join("; ")
            )));
        }

        let mut entry_ids = Vec::new();
        let mut transfer_ids = Vec::new();
        let mut pending_request_ids = Vec::new();

        for row in &draft.rows {
            let date = row
                .date
                .clone()
                .filter(|value| is_valid_calendar_date(value))
                .unwrap_or_else(|| draft.local_date.clone());
            let occurred_at = format!("{date}T12:00:00");
            let amount = row.amount_minor.unwrap_or(0);
            let account_id = match resolved.get(&row.row_id).copied() {
                Some(id) => id,
                None => continue,
            };

            match row.kind {
                DraftRowKind::Expense | DraftRowKind::Income => {
                    Self::ensure_account_write_in(&mut *tx, enforce, member_id, account_id).await?;
                    let id = Self::write_entry(
                        &mut *tx,
                        TransactionRequest {
                            kind: if row.kind == DraftRowKind::Income {
                                TransactionKind::Income
                            } else {
                                TransactionKind::Expense
                            },
                            account_id,
                            amount_minor: amount,
                            name: row.name.clone(),
                            category_id: row.category_id,
                            reconciled: false,
                            note: Some(row.note.clone()),
                            occurred_at: Some(occurred_at),
                            labels: None,
                        },
                    )
                    .await?;
                    entry_ids.push(id);
                }
                DraftRowKind::Transfer => {
                    let destination_id = resolved
                        .get(&format!("{}:to", row.row_id))
                        .copied()
                        .ok_or_else(|| {
                            AppError::BadRequest(format!(
                                "{}: choose a receiving account",
                                row.row_id
                            ))
                        })?;
                    Self::ensure_account_write_in(&mut *tx, enforce, member_id, account_id).await?;
                    let source_owner = Self::account_owner(&mut *tx, account_id).await?;
                    let destination_owner = Self::account_owner(&mut *tx, destination_id).await?;
                    if source_owner == destination_owner {
                        if enforce {
                            Self::ensure_account_write_in(
                                &mut *tx,
                                enforce,
                                member_id,
                                destination_id,
                            )
                            .await?;
                        }
                        let id = Self::write_transfer(
                            &mut *tx,
                            TransferRequest {
                                from_account_id: account_id,
                                to_account_id: destination_id,
                                amount_minor: amount,
                                name: row.name.clone(),
                                reconciled: false,
                                note: Some(row.note.clone()),
                                occurred_at: Some(occurred_at),
                                labels: None,
                            },
                        )
                        .await?;
                        transfer_ids.push(id);
                    } else {
                        let recipient = destination_owner.ok_or_else(|| {
                            AppError::BadRequest(format!(
                                "{}: the receiving account needs an owner",
                                row.row_id
                            ))
                        })?;
                        let sender = source_owner.unwrap_or(member_id);
                        if sender == recipient {
                            return Err(AppError::BadRequest(format!(
                                "{}: a transfer must have two different members",
                                row.row_id
                            )));
                        }
                        let id = crate::store::transfer_requests::create_request_in(
                            &mut *tx,
                            NewTransferRequest {
                                source_account_id: account_id,
                                destination_account_id: destination_id,
                                amount_minor: amount,
                                name: row.name.clone(),
                                note: row.note.clone(),
                                requested_date: date,
                                sender_member_id: sender,
                                recipient_member_id: recipient,
                                source_batch_id: Some(save_key.clone()),
                                original_transfer_id: None,
                            },
                        )
                        .await?;
                        pending_request_ids.push(id);
                    }
                }
                DraftRowKind::NewAccount => {}
            }
        }

        let created_at = now_string();
        let receipt = SaveReceipt {
            batch_id: save_key.clone(),
            member_id,
            revision,
            accounts_created,
            entry_ids,
            transfer_ids,
            pending_request_ids,
            created_at: created_at.clone(),
        };
        sqlx::query(
            "INSERT INTO save_receipts (save_key, member_id, revision, result_json, created_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(save_key) DO NOTHING",
        )
        .bind(&save_key)
        .bind(member_id)
        .bind(revision)
        .bind(encode_json(&receipt)?)
        .bind(&created_at)
        .execute(&mut *tx)
        .await?;

        sqlx::query("DELETE FROM draft_rows WHERE member_id = ?")
            .bind(member_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM draft_prompts WHERE member_id = ?")
            .bind(member_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "UPDATE drafts SET revision = revision + 1, updated_at = ? WHERE member_id = ?",
        )
        .bind(&created_at)
        .bind(member_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(receipt)
    }

    async fn account_owner<'e, E>(executor: E, account_id: i64) -> AppResult<Option<i64>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query_scalar::<_, Option<i64>>("SELECT owner_member_id FROM accounts WHERE id = ?")
            .bind(account_id)
            .fetch_one(executor)
            .await
            .map_err(AppError::from)
    }

    fn resolve_draft_account(
        account_id: Option<i64>,
        proposed_row_id: Option<&str>,
        account_name: Option<&str>,
        proposed_ids: &HashMap<String, i64>,
        by_name: &HashMap<String, Vec<i64>>,
        label: &str,
        issues: &mut Vec<String>,
    ) -> Option<i64> {
        if let Some(id) = account_id {
            return Some(id);
        }
        if let Some(row_id) = proposed_row_id {
            return match proposed_ids.get(row_id) {
                Some(id) => Some(*id),
                None => {
                    issues.push(format!("{label}: proposed account was not created"));
                    None
                }
            };
        }
        if let Some(name) = account_name {
            let key = name.trim().to_lowercase();
            return match by_name.get(&key) {
                Some(ids) if ids.len() == 1 => Some(ids[0]),
                Some(_) => {
                    issues.push(format!("{label}: account name is ambiguous"));
                    None
                }
                None => {
                    issues.push(format!("{label}: account name was not found"));
                    None
                }
            };
        }
        issues.push(format!("{label}: choose an account"));
        None
    }

    /// Explicit allowlist of account and category names for an assistant
    /// request. Never includes ledger history, balances, or other prompts.
    pub async fn draft_context(
        &self,
        member_id: i64,
        draft: &Draft,
        enforce: bool,
    ) -> AppResult<(Vec<AssistantAccountName>, Vec<AssistantCategoryName>)> {
        let mut referenced_ids = HashSet::new();
        let mut referenced_names = HashSet::new();
        for row in &draft.rows {
            if let Some(id) = row.account_id {
                referenced_ids.insert(id);
            }
            if let Some(id) = row.to_account_id {
                referenced_ids.insert(id);
            }
            if let Some(name) = &row.account_name {
                referenced_names.insert(name.trim().to_lowercase());
            }
            if let Some(name) = &row.to_account_name {
                referenced_names.insert(name.trim().to_lowercase());
            }
        }

        let accounts = sqlx::query_as::<_, (i64, String, Option<i64>)>(
            "SELECT id, name, owner_member_id FROM accounts ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;
        // Only send account names needed for this follow-up: accounts already
        // referenced by the draft, or permitted accounts when a row has an
        // unresolved name that needs matching. No unrelated names are included.
        let needs_matching_names = draft.rows.iter().any(|row| {
            row.account_name.is_some()
                || row.to_account_name.is_some()
                || (row.kind != DraftRowKind::NewAccount && row.account_id.is_none())
        });
        let account_names = accounts
            .into_iter()
            .filter(|(id, name, owner)| {
                referenced_ids.contains(id)
                    || referenced_names.contains(&name.trim().to_lowercase())
                    || (needs_matching_names && (!enforce || owner == &Some(member_id)))
            })
            .take(50)
            .map(|(id, name, _)| AssistantAccountName { id, name })
            .collect();

        let categories = sqlx::query_as::<_, (i64, String, String)>(
            "SELECT id, name, kind FROM categories
             WHERE archived_at IS NULL ORDER BY kind, sort_order, name LIMIT 50",
        )
        .fetch_all(&self.pool)
        .await?;
        let category_names = categories
            .into_iter()
            .map(|(id, name, kind)| AssistantCategoryName { id, name, kind })
            .collect();
        Ok((account_names, category_names))
    }
}
