use super::Store;
use crate::error::{AppError, AppResult};
use crate::models::*;
use chrono::Utc;
use sqlx::SqliteConnection;

impl Store {
    async fn write_transfer(
        transaction: &mut SqliteConnection,
        request: TransferRequest,
    ) -> AppResult<i64> {
        Store::validate_amount(request.amount_minor)?;
        if request.from_account_id == request.to_account_id {
            return Err(AppError::BadRequest(
                "from and to accounts must be different".to_string(),
            ));
        }
        let note = Store::clean_note(request.note)?;
        let name = Store::clean_name(request.name, &note, "Transfer")?;
        let occurred_at = Store::clean_timestamp(request.occurred_at)?;
        let label_ids = Store::validate_label_ids(&mut *transaction, request.labels).await?;
        let created_at = Utc::now().to_rfc3339();

        let from_account = Store::load_account(&mut *transaction, request.from_account_id).await?;
        let to_account = Store::load_account(&mut *transaction, request.to_account_id).await?;
        if from_account.currency != to_account.currency {
            return Err(AppError::Conflict(
                "cross-currency transfers are not supported in this milestone".to_string(),
            ));
        }

        let transfer_id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO transfers
                (from_account_id, to_account_id, amount_minor, name, reconciled, note, occurred_at, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             RETURNING id",
        )
        .bind(request.from_account_id)
        .bind(request.to_account_id)
        .bind(request.amount_minor)
        .bind(&name)
        .bind(request.reconciled)
        .bind(&note)
        .bind(&occurred_at)
        .bind(&created_at)
        .fetch_one(&mut *transaction)
        .await?;

        for (kind, account_id) in [
            ("transfer_out", request.from_account_id),
            ("transfer_in", request.to_account_id),
        ] {
            sqlx::query(
                "INSERT INTO entries
                    (kind, account_id, amount_minor, name, category_id, reconciled, note, occurred_at, created_at, transfer_id)
                 VALUES (?, ?, ?, ?, NULL, ?, ?, ?, ?, ?)",
            )
            .bind(kind)
            .bind(account_id)
            .bind(request.amount_minor)
            .bind(&name)
            .bind(request.reconciled)
            .bind(&note)
            .bind(&occurred_at)
            .bind(&created_at)
            .bind(transfer_id)
            .execute(&mut *transaction)
            .await?;
        }

        Store::replace_transfer_labels(&mut *transaction, transfer_id, label_ids).await?;
        Ok(transfer_id)
    }

    pub async fn create_transfer(&self, request: TransferRequest) -> AppResult<TransferResponse> {
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let id = Self::write_transfer(&mut transaction, request.clone()).await?;
        transaction.commit().await?;
        Ok(TransferResponse {
            id,
            from_account_id: request.from_account_id,
            to_account_id: request.to_account_id,
            amount_minor: request.amount_minor,
        })
    }

    pub async fn update_transfer(&self, id: i64, request: TransferRequest) -> AppResult<()> {
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM transfers WHERE id = ?")
            .bind(id)
            .fetch_one(&mut *transaction)
            .await?;
        if exists == 0 {
            return Err(AppError::NotFound(format!("transfer {id} was not found")));
        }

        Store::validate_amount(request.amount_minor)?;
        if request.from_account_id == request.to_account_id {
            return Err(AppError::BadRequest(
                "from and to accounts must be different".to_string(),
            ));
        }
        let note = Store::clean_note(request.note)?;
        let name = Store::clean_name(request.name, &note, "Transfer")?;
        let occurred_at = Store::clean_timestamp(request.occurred_at)?;
        let label_ids = Store::validate_label_ids(&mut transaction, request.labels).await?;
        let from = Store::load_account(&mut *transaction, request.from_account_id).await?;
        let to = Store::load_account(&mut *transaction, request.to_account_id).await?;
        if from.currency != to.currency {
            return Err(AppError::Conflict(
                "cross-currency transfers are not supported in this milestone".to_string(),
            ));
        }

        sqlx::query(
            "UPDATE transfers
             SET from_account_id = ?, to_account_id = ?, amount_minor = ?, name = ?,
                 reconciled = ?, note = ?, occurred_at = ?
             WHERE id = ?",
        )
        .bind(request.from_account_id)
        .bind(request.to_account_id)
        .bind(request.amount_minor)
        .bind(&name)
        .bind(request.reconciled)
        .bind(&note)
        .bind(&occurred_at)
        .bind(id)
        .execute(&mut *transaction)
        .await?;

        for (kind, account_id) in [
            ("transfer_out", request.from_account_id),
            ("transfer_in", request.to_account_id),
        ] {
            sqlx::query(
                "UPDATE entries
                 SET account_id = ?, amount_minor = ?, name = ?, reconciled = ?,
                     note = ?, occurred_at = ?
                 WHERE transfer_id = ? AND kind = ?",
            )
            .bind(account_id)
            .bind(request.amount_minor)
            .bind(&name)
            .bind(request.reconciled)
            .bind(&note)
            .bind(&occurred_at)
            .bind(id)
            .bind(kind)
            .execute(&mut *transaction)
            .await?;
        }
        Store::replace_transfer_labels(&mut transaction, id, label_ids).await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn duplicate_transfer(&self, id: i64) -> AppResult<i64> {
        let entry = self.transfer_detail(id).await?;
        self.create_transfer(TransferRequest {
            from_account_id: entry.account_id,
            to_account_id: entry.counterpart_account_id.unwrap(),
            amount_minor: entry.amount_minor,
            name: entry.name,
            reconciled: entry.reconciled,
            note: Some(entry.note),
            occurred_at: Some(entry.occurred_at),
            labels: Some(entry.labels.into_iter().map(|label| label.id).collect()),
        })
        .await
        .map(|response| response.id)
    }

    pub async fn delete_transfer(&self, id: i64) -> AppResult<()> {
        let mut transaction = self.pool.begin().await?;
        let result = sqlx::query("DELETE FROM transfers WHERE id = ?")
            .bind(id)
            .execute(&mut *transaction)
            .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("transfer {id} was not found")));
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn set_transfer_reconciled(&self, id: i64, reconciled: bool) -> AppResult<()> {
        let mut transaction = self.pool.begin().await?;
        let transfer_result = sqlx::query("UPDATE transfers SET reconciled = ? WHERE id = ?")
            .bind(reconciled)
            .bind(id)
            .execute(&mut *transaction)
            .await?;
        if transfer_result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("transfer {id} was not found")));
        }
        sqlx::query("UPDATE entries SET reconciled = ? WHERE transfer_id = ?")
            .bind(reconciled)
            .bind(id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }
}
