use sqlx::{Executor, Sqlite};

use crate::error::{AppError, AppResult};

use super::Store;

fn check_owner(owner: Option<i64>, enforce: bool, member_id: i64) -> AppResult<()> {
    if !enforce {
        return Ok(());
    }
    match owner {
        Some(owner) if owner == member_id => Ok(()),
        Some(_) => Err(AppError::Conflict(
            "this account belongs to another household member".to_string(),
        )),
        None => Err(AppError::Conflict(
            "account ownership has not been assigned yet".to_string(),
        )),
    }
}

impl Store {
    /// Permission check bound to the caller's executor so the batch save sees
    /// the same transaction state it validates and writes.
    pub(super) async fn ensure_account_write_in<'e, E>(
        executor: E,
        enforce: bool,
        member_id: i64,
        account_id: i64,
    ) -> AppResult<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let owner = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT owner_member_id FROM accounts WHERE id = ?",
        )
        .bind(account_id)
        .fetch_optional(executor)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("account {account_id} was not found")))?;
        check_owner(owner, enforce, member_id)
    }

    pub async fn ensure_account_write(
        &self,
        enforce: bool,
        member_id: i64,
        account_id: i64,
    ) -> AppResult<()> {
        Self::ensure_account_write_in(&self.pool, enforce, member_id, account_id).await
    }

    pub(super) async fn entry_account_id_in<'e, E>(executor: E, entry_id: i64) -> AppResult<i64>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query_scalar::<_, i64>("SELECT account_id FROM entries WHERE id = ?")
            .bind(entry_id)
            .fetch_optional(executor)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("transaction {entry_id} was not found")))
    }

    pub async fn ensure_entry_write(
        &self,
        enforce: bool,
        member_id: i64,
        entry_id: i64,
    ) -> AppResult<()> {
        let account_id = Self::entry_account_id_in(&self.pool, entry_id).await?;
        self.ensure_account_write(enforce, member_id, account_id)
            .await
    }

    pub(super) async fn transfer_accounts_in<'e, E>(
        executor: E,
        transfer_id: i64,
    ) -> AppResult<(i64, i64)>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query_as::<_, (i64, i64)>(
            "SELECT from_account_id, to_account_id FROM transfers WHERE id = ?",
        )
        .bind(transfer_id)
        .fetch_optional(executor)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("transfer {transfer_id} was not found")))
    }

    /// Posted cross-member transfers cannot be edited, deleted, duplicated, or
    /// reconciled directly. Corrections go through an approved reversal.
    pub async fn ensure_transfer_write(
        &self,
        enforce: bool,
        member_id: i64,
        transfer_id: i64,
    ) -> AppResult<()> {
        let (from, to) = Self::transfer_accounts_in(&self.pool, transfer_id).await?;
        self.ensure_account_write(enforce, member_id, from).await?;
        self.ensure_account_write(enforce, member_id, to).await
    }

    pub async fn is_designated_member(&self, member_id: i64) -> AppResult<bool> {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM members WHERE id = ? AND is_designated = 1",
        )
        .bind(member_id)
        .fetch_one(&self.pool)
        .await
        .map(|count| count > 0)
        .map_err(AppError::from)
    }

    pub async fn member_exists(&self, member_id: i64) -> AppResult<bool> {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM members WHERE id = ?")
            .bind(member_id)
            .fetch_one(&self.pool)
            .await
            .map(|count| count > 0)
            .map_err(AppError::from)
    }
}
