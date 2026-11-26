use super::Store;
use crate::error::{AppError, AppResult};
use crate::models::*;
use chrono::Utc;
use sqlx::{Executor, Sqlite};

impl Store {
    async fn validate_category<'e, E>(
        executor: E,
        category_id: Option<i64>,
        kind: &str,
    ) -> AppResult<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let Some(category_id) = category_id else {
            return Ok(());
        };
        let row = sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT kind, archived_at FROM categories WHERE id = ?",
        )
        .bind(category_id)
        .fetch_optional(executor)
        .await?
        .ok_or_else(|| AppError::BadRequest(format!("category {category_id} was not found")))?;
        if row.0 != kind {
            return Err(AppError::BadRequest(
                "category type must match the transaction type".to_string(),
            ));
        }
        if row.1.is_some() {
            return Err(AppError::BadRequest(
                "archived categories cannot be assigned to new entries".to_string(),
            ));
        }
        Ok(())
    }

    async fn validate_category_for_update(
        &self,
        entry_id: i64,
        category_id: Option<i64>,
        kind: &str,
    ) -> AppResult<()> {
        let Some(category_id) = category_id else {
            return Ok(());
        };
        let current = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT category_id FROM entries WHERE id = ? AND transfer_id IS NULL",
        )
        .bind(entry_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("transaction {entry_id} was not found")))?;
        let category = sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT kind, archived_at FROM categories WHERE id = ?",
        )
        .bind(category_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::BadRequest(format!("category {category_id} was not found")))?;
        if category.0 != kind {
            return Err(AppError::BadRequest(
                "category type must match the transaction type".to_string(),
            ));
        }
        if category.1.is_some() && current != Some(category_id) {
            return Err(AppError::BadRequest(
                "archived categories cannot be assigned to new entries".to_string(),
            ));
        }
        Ok(())
    }

    pub async fn create_transaction(&self, request: TransactionRequest) -> AppResult<i64> {
        Self::validate_amount(request.amount_minor)?;
        Store::load_account(&self.pool, request.account_id).await?;
        Self::validate_category(&self.pool, request.category_id, request.kind.as_str()).await?;
        let note = Self::clean_note(request.note)?;
        let name = Self::clean_name(request.name, &note, request.kind.as_str())?;
        let occurred_at = Self::clean_timestamp(request.occurred_at)?;

        let id = sqlx::query_scalar(
            "INSERT INTO entries
                (kind, account_id, amount_minor, name, category_id, reconciled, note, occurred_at, created_at, transfer_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)
             RETURNING id",
        )
        .bind(request.kind.as_str())
        .bind(request.account_id)
        .bind(request.amount_minor)
        .bind(name)
        .bind(request.category_id)
        .bind(request.reconciled)
        .bind(note)
        .bind(occurred_at)
        .bind(Utc::now().to_rfc3339())
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn update_transaction(&self, id: i64, request: TransactionRequest) -> AppResult<()> {
        Self::validate_amount(request.amount_minor)?;
        Store::load_account(&self.pool, request.account_id).await?;
        self.validate_category_for_update(id, request.category_id, request.kind.as_str())
            .await?;
        let note = Self::clean_note(request.note)?;
        let name = Self::clean_name(request.name, &note, request.kind.as_str())?;
        let occurred_at = Self::clean_timestamp(request.occurred_at)?;

        let result = sqlx::query(
            "UPDATE entries
             SET kind = ?, account_id = ?, amount_minor = ?, name = ?, category_id = ?,
                 reconciled = ?, note = ?, occurred_at = ?
             WHERE id = ? AND transfer_id IS NULL",
        )
        .bind(request.kind.as_str())
        .bind(request.account_id)
        .bind(request.amount_minor)
        .bind(name)
        .bind(request.category_id)
        .bind(request.reconciled)
        .bind(note)
        .bind(occurred_at)
        .bind(id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!(
                "transaction {id} was not found"
            )));
        }
        Ok(())
    }

    pub async fn duplicate_transaction(&self, id: i64) -> AppResult<i64> {
        let entry = self.transaction_detail(id).await?;
        self.create_transaction(TransactionRequest {
            kind: if entry.kind == "income" {
                TransactionKind::Income
            } else {
                TransactionKind::Expense
            },
            account_id: entry.account_id,
            amount_minor: entry.amount_minor,
            name: entry.name,
            category_id: entry.category_id,
            reconciled: entry.reconciled,
            note: Some(entry.note),
            occurred_at: Some(entry.occurred_at),
        })
        .await
    }

    pub async fn delete_transaction(&self, id: i64) -> AppResult<()> {
        let result = sqlx::query("DELETE FROM entries WHERE id = ? AND transfer_id IS NULL")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!(
                "transaction {id} was not found"
            )));
        }
        Ok(())
    }

    pub async fn set_transaction_reconciled(&self, id: i64, reconciled: bool) -> AppResult<()> {
        let result =
            sqlx::query("UPDATE entries SET reconciled = ? WHERE id = ? AND transfer_id IS NULL")
                .bind(reconciled)
                .bind(id)
                .execute(&self.pool)
                .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!(
                "transaction {id} was not found"
            )));
        }
        Ok(())
    }
}
