use super::Store;
use crate::date::{date_only, is_valid_calendar_date};
use crate::error::{AppError, AppResult};
use crate::models::*;
use chrono::Utc;
use sqlx::sqlite::SqliteRow;
use sqlx::{Executor, Row, Sqlite, SqliteConnection};

/// Module-level entry point used by the Command Center batch save, which
/// calls this helper by its module path inside its own transaction.
pub(super) async fn create_request_in(
    executor: &mut SqliteConnection,
    input: NewTransferRequest,
) -> AppResult<i64> {
    Store::create_request_in(executor, input).await
}

impl Store {
    /// Owner of an account, bound to the caller's executor so a batch or an
    /// approval validates the same state it writes. `None` means the account
    /// has not been assigned yet (legacy accounts remain writable here).
    async fn account_owner_in<'e, E>(executor: E, account_id: i64) -> AppResult<Option<i64>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query_scalar::<_, Option<i64>>("SELECT owner_member_id FROM accounts WHERE id = ?")
            .bind(account_id)
            .fetch_optional(executor)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("account {account_id} was not found")))
    }

    fn ensure_endpoint_owner(owner: Option<i64>, expected: i64, endpoint: &str) -> AppResult<()> {
        match owner {
            Some(owner) if owner == expected => Ok(()),
            Some(_) => Err(AppError::Conflict(format!(
                "the {endpoint} account belongs to another household member"
            ))),
            None => Ok(()),
        }
    }

    fn record_from_row(row: &SqliteRow) -> AppResult<TransferRequestRecord> {
        let status: String = row.try_get("status")?;
        let status = match status.as_str() {
            "pending" => RequestStatus::Pending,
            "approved" => RequestStatus::Approved,
            "declined" => RequestStatus::Declined,
            "canceled" => RequestStatus::Canceled,
            _ => {
                return Err(AppError::Conflict(
                    "transfer request has an unknown status".to_string(),
                ))
            }
        };
        Ok(TransferRequestRecord {
            id: row.try_get("id")?,
            source_account_id: row.try_get("source_account_id")?,
            source_account_name: row.try_get("source_account_name")?,
            destination_account_id: row.try_get("destination_account_id")?,
            destination_account_name: row.try_get("destination_account_name")?,
            amount_minor: row.try_get("amount_minor")?,
            name: row.try_get("name")?,
            note: row.try_get("note")?,
            requested_date: row.try_get("requested_date")?,
            sender_member_id: row.try_get("sender_member_id")?,
            recipient_member_id: row.try_get("recipient_member_id")?,
            status,
            source_batch_id: row.try_get("source_batch_id")?,
            original_transfer_id: row.try_get("original_transfer_id")?,
            created_at: row.try_get("created_at")?,
            decided_at: row.try_get("decided_at")?,
            decided_by_member_id: row.try_get("decided_by_member_id")?,
            posted_transfer_id: row.try_get("posted_transfer_id")?,
        })
    }

    /// Read a request (with account names) on any executor, including the
    /// approval transaction.
    pub(super) async fn request_row<'e, E>(executor: E, id: i64) -> AppResult<TransferRequestRecord>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query(
            "SELECT tr.id, tr.source_account_id, src.name AS source_account_name,
                    tr.destination_account_id, dst.name AS destination_account_name,
                    tr.amount_minor, tr.name, tr.note, tr.requested_date,
                    tr.sender_member_id, tr.recipient_member_id, tr.status,
                    tr.source_batch_id, tr.original_transfer_id, tr.created_at,
                    tr.decided_at, tr.decided_by_member_id, tr.posted_transfer_id
             FROM transfer_requests tr
             JOIN accounts src ON src.id = tr.source_account_id
             JOIN accounts dst ON dst.id = tr.destination_account_id
             WHERE tr.id = ?",
        )
        .bind(id)
        .fetch_optional(executor)
        .await?
        .map(|row| Self::record_from_row(&row))
        .transpose()?
        .ok_or_else(|| AppError::NotFound(format!("transfer request {id} was not found")))
    }

    async fn pending_request_row<'e, E>(
        executor: E,
        id: i64,
    ) -> AppResult<Option<TransferRequestRecord>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query(
            "SELECT tr.id, tr.source_account_id, src.name AS source_account_name,
                    tr.destination_account_id, dst.name AS destination_account_name,
                    tr.amount_minor, tr.name, tr.note, tr.requested_date,
                    tr.sender_member_id, tr.recipient_member_id, tr.status,
                    tr.source_batch_id, tr.original_transfer_id, tr.created_at,
                    tr.decided_at, tr.decided_by_member_id, tr.posted_transfer_id
             FROM transfer_requests tr
             JOIN accounts src ON src.id = tr.source_account_id
             JOIN accounts dst ON dst.id = tr.destination_account_id
             WHERE tr.id = ? AND tr.status = 'pending'",
        )
        .bind(id)
        .fetch_optional(executor)
        .await?
        .map(|row| Self::record_from_row(&row))
        .transpose()
    }

    /// Executor-bound insert shared with the Command Center batch save. The
    /// request is stored with no ledger effect until it is approved.
    pub(super) async fn create_request_in(
        executor: &mut SqliteConnection,
        input: NewTransferRequest,
    ) -> AppResult<i64> {
        Store::validate_amount(input.amount_minor)?;
        if input.source_account_id == input.destination_account_id {
            return Err(AppError::BadRequest(
                "source and destination accounts must be different".to_string(),
            ));
        }
        if input.sender_member_id == input.recipient_member_id {
            return Err(AppError::BadRequest(
                "sender and recipient must be different members".to_string(),
            ));
        }
        if !is_valid_calendar_date(&input.requested_date) {
            return Err(AppError::BadRequest(
                "requested_date must be a valid YYYY-MM-DD calendar date".to_string(),
            ));
        }
        let note = Store::clean_note(Some(input.note))?;
        let name = Store::clean_name(input.name, &note, "Transfer")?;

        Store::load_account(&mut *executor, input.source_account_id).await?;
        Store::load_account(&mut *executor, input.destination_account_id).await?;

        // The batch path does not know whether household enforcement is on, so
        // validate ownership whenever an account has an assigned owner.
        let source_owner = Self::account_owner_in(&mut *executor, input.source_account_id).await?;
        let destination_owner =
            Self::account_owner_in(&mut *executor, input.destination_account_id).await?;
        Self::ensure_endpoint_owner(source_owner, input.sender_member_id, "source")?;
        Self::ensure_endpoint_owner(destination_owner, input.recipient_member_id, "destination")?;

        for member_id in [input.sender_member_id, input.recipient_member_id] {
            let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM members WHERE id = ?")
                .bind(member_id)
                .fetch_one(&mut *executor)
                .await?;
            if exists == 0 {
                return Err(AppError::BadRequest(format!(
                    "member {member_id} was not found"
                )));
            }
        }

        let created_at = Utc::now().to_rfc3339();
        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO transfer_requests
                (source_account_id, destination_account_id, amount_minor, name, note,
                 requested_date, sender_member_id, recipient_member_id, status,
                 source_batch_id, original_transfer_id, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?, ?, ?)
             RETURNING id",
        )
        .bind(input.source_account_id)
        .bind(input.destination_account_id)
        .bind(input.amount_minor)
        .bind(&name)
        .bind(&note)
        .bind(&input.requested_date)
        .bind(input.sender_member_id)
        .bind(input.recipient_member_id)
        .bind(input.source_batch_id)
        .bind(input.original_transfer_id)
        .bind(&created_at)
        .fetch_one(&mut *executor)
        .await?;
        Ok(id)
    }

    async fn requests_for(
        &self,
        member_id: i64,
        incoming: bool,
    ) -> AppResult<Vec<TransferRequestRecord>> {
        let member_column = if incoming {
            "tr.recipient_member_id"
        } else {
            "tr.sender_member_id"
        };
        let sql = format!(
            "SELECT tr.id, tr.source_account_id, src.name AS source_account_name,
                    tr.destination_account_id, dst.name AS destination_account_name,
                    tr.amount_minor, tr.name, tr.note, tr.requested_date,
                    tr.sender_member_id, tr.recipient_member_id, tr.status,
                    tr.source_batch_id, tr.original_transfer_id, tr.created_at,
                    tr.decided_at, tr.decided_by_member_id, tr.posted_transfer_id
             FROM transfer_requests tr
             JOIN accounts src ON src.id = tr.source_account_id
             JOIN accounts dst ON dst.id = tr.destination_account_id
             WHERE {member_column} = ?
             ORDER BY tr.created_at DESC, tr.id DESC"
        );
        let rows = sqlx::query(&sql)
            .bind(member_id)
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(Self::record_from_row).collect()
    }

    /// Incoming and outgoing requests for a member, newest first.
    pub async fn list_requests(&self, member_id: i64) -> AppResult<TransferRequestsResponse> {
        let incoming = self.requests_for(member_id, true).await?;
        let outgoing = self.requests_for(member_id, false).await?;
        Ok(TransferRequestsResponse {
            incoming_count: incoming.len(),
            outgoing_count: outgoing.len(),
            incoming,
            outgoing,
        })
    }

    /// Only the sender or the recipient may read a request.
    pub async fn request_detail(
        &self,
        member_id: i64,
        id: i64,
    ) -> AppResult<TransferRequestRecord> {
        sqlx::query(
            "SELECT tr.id, tr.source_account_id, src.name AS source_account_name,
                    tr.destination_account_id, dst.name AS destination_account_name,
                    tr.amount_minor, tr.name, tr.note, tr.requested_date,
                    tr.sender_member_id, tr.recipient_member_id, tr.status,
                    tr.source_batch_id, tr.original_transfer_id, tr.created_at,
                    tr.decided_at, tr.decided_by_member_id, tr.posted_transfer_id
             FROM transfer_requests tr
             JOIN accounts src ON src.id = tr.source_account_id
             JOIN accounts dst ON dst.id = tr.destination_account_id
             WHERE tr.id = ? AND (tr.sender_member_id = ? OR tr.recipient_member_id = ?)",
        )
        .bind(id)
        .bind(member_id)
        .bind(member_id)
        .fetch_optional(&self.pool)
        .await?
        .map(|row| Self::record_from_row(&row))
        .transpose()?
        .ok_or_else(|| AppError::NotFound(format!("transfer request {id} was not found")))
    }

    /// Approve, decline, or cancel a pending request. The whole read/validate/
    /// post/decide sequence is serialized so only one terminal decision and at
    /// most one posted transfer can result.
    pub async fn decide(
        &self,
        member_id: i64,
        id: i64,
        decision: &str,
    ) -> AppResult<DecisionResponse> {
        let status = match decision {
            "approve" => "approved",
            "decline" => "declined",
            "cancel" => "canceled",
            _ => {
                return Err(AppError::BadRequest(
                    "decision must be approve, decline, or cancel".to_string(),
                ))
            }
        };
        let approving = decision == "approve";

        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let pending = Self::pending_request_row(&mut *transaction, id).await?;
        let Some(pending) = pending else {
            // Distinguish an unknown request from one already decided by a
            // competing action: the latter is a conflict, never a 500.
            Self::request_row(&mut *transaction, id).await?;
            return Err(AppError::Conflict(format!(
                "transfer request {id} is no longer pending"
            )));
        };

        if approving || decision == "decline" {
            if pending.recipient_member_id != member_id {
                return Err(AppError::Conflict(
                    "only the receiving member can approve or decline this request".to_string(),
                ));
            }
        } else if pending.sender_member_id != member_id {
            return Err(AppError::Conflict(
                "only the sending member can cancel this request".to_string(),
            ));
        }

        let posted_transfer_id = if approving {
            Store::load_account(&mut *transaction, pending.source_account_id).await?;
            Store::load_account(&mut *transaction, pending.destination_account_id).await?;
            let source_owner =
                Self::account_owner_in(&mut *transaction, pending.source_account_id).await?;
            let destination_owner =
                Self::account_owner_in(&mut *transaction, pending.destination_account_id).await?;
            Self::ensure_endpoint_owner(source_owner, pending.sender_member_id, "source")?;
            Self::ensure_endpoint_owner(
                destination_owner,
                pending.recipient_member_id,
                "destination",
            )?;

            if let Some(original_transfer_id) = pending.original_transfer_id {
                let approved_reversals = sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM transfer_requests
                     WHERE original_transfer_id = ? AND status = 'approved'",
                )
                .bind(original_transfer_id)
                .fetch_one(&mut *transaction)
                .await?;
                if approved_reversals > 0 {
                    return Err(AppError::Conflict(
                        "an approved reversal already exists for this transfer".to_string(),
                    ));
                }
            }

            Some(
                Store::write_transfer(
                    &mut *transaction,
                    TransferRequest {
                        from_account_id: pending.source_account_id,
                        to_account_id: pending.destination_account_id,
                        amount_minor: pending.amount_minor,
                        name: pending.name.clone(),
                        note: Some(pending.note.clone()),
                        reconciled: false,
                        occurred_at: Some(format!("{}T12:00:00", pending.requested_date)),
                        labels: None,
                    },
                )
                .await?,
            )
        } else {
            None
        };

        let decided_at = Utc::now().to_rfc3339();
        let updated = sqlx::query(
            "UPDATE transfer_requests
             SET status = ?, decided_at = ?, decided_by_member_id = ?, posted_transfer_id = ?
             WHERE id = ? AND status = 'pending'",
        )
        .bind(status)
        .bind(&decided_at)
        .bind(member_id)
        .bind(posted_transfer_id)
        .bind(id)
        .execute(&mut *transaction)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(AppError::Conflict(format!(
                "transfer request {id} is no longer pending"
            )));
        }
        transaction.commit().await?;

        Ok(DecisionResponse {
            request: Self::request_row(&self.pool, id).await?,
            posted_transfer_id,
        })
    }

    /// Submit an approved reversal of a posted transfer. The direction is
    /// swapped: the reversal's source is the original destination, so its
    /// current owner initiates it, and the original source's current owner is
    /// the recipient.
    pub async fn reverse_transfer(
        &self,
        member_id: i64,
        transfer_id: i64,
    ) -> AppResult<TransferRequestRecord> {
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let original = sqlx::query(
            "SELECT t.from_account_id, t.to_account_id, t.amount_minor, t.name, t.note,
                    t.occurred_at, src.owner_member_id AS source_owner_member_id,
                    dst.owner_member_id AS destination_owner_member_id
             FROM transfers t
             JOIN accounts src ON src.id = t.from_account_id
             JOIN accounts dst ON dst.id = t.to_account_id
             WHERE t.id = ?",
        )
        .bind(transfer_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("transfer {transfer_id} was not found")))?;

        let from_account_id: i64 = original.try_get("from_account_id")?;
        let to_account_id: i64 = original.try_get("to_account_id")?;
        let amount_minor: i64 = original.try_get("amount_minor")?;
        let name: String = original.try_get("name")?;
        let note: String = original.try_get("note")?;
        let occurred_at: String = original.try_get("occurred_at")?;
        let source_owner: Option<i64> = original.try_get("source_owner_member_id")?;
        let destination_owner: Option<i64> = original.try_get("destination_owner_member_id")?;

        let requested_date = date_only(&occurred_at).ok_or_else(|| {
            AppError::BadRequest("the original transfer has no valid calendar date".to_string())
        })?;

        // The initiator must currently control the account the reversal sends
        // from. Unowned legacy accounts stay eligible (default member when
        // household enforcement is off).
        Self::ensure_endpoint_owner(destination_owner, member_id, "reversal source")?;
        // The reversal's destination needs an assigned recipient owner.
        let recipient_member_id = source_owner.ok_or_else(|| {
            AppError::Conflict(
                "the receiving account has no assigned owner; assign ownership first".to_string(),
            )
        })?;

        let approved_reversals = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM transfer_requests
             WHERE original_transfer_id = ? AND status = 'approved'",
        )
        .bind(transfer_id)
        .fetch_one(&mut *transaction)
        .await?;
        if approved_reversals > 0 {
            return Err(AppError::Conflict(
                "an approved reversal already exists for this transfer".to_string(),
            ));
        }

        let id = Self::create_request_in(
            &mut transaction,
            NewTransferRequest {
                source_account_id: to_account_id,
                destination_account_id: from_account_id,
                amount_minor,
                name,
                note,
                requested_date,
                sender_member_id: member_id,
                recipient_member_id,
                source_batch_id: None,
                original_transfer_id: Some(transfer_id),
            },
        )
        .await?;
        transaction.commit().await?;
        self.request_detail(member_id, id).await
    }
}
