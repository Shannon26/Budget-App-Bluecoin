use crate::error::{AppError, AppResult};
use crate::models::{
    Account, AccountWithBalance, CreateAccountRequest, CreateTransactionRequest,
    CreateTransferRequest, EntryView, StateResponse, Totals, TransferResponse,
};
use chrono::Utc;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct Store {
    pool: SqlitePool,
}

impl Store {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn validate_amount(amount_minor: i64) -> AppResult<()> {
        if amount_minor <= 0 {
            return Err(AppError::BadRequest(
                "amount must be greater than zero".to_string(),
            ));
        }
        Ok(())
    }

    fn clean_note(note: Option<String>) -> AppResult<String> {
        let note = note.unwrap_or_default().trim().to_string();
        if note.chars().count() > 500 {
            return Err(AppError::BadRequest(
                "note must be 500 characters or fewer".to_string(),
            ));
        }
        Ok(note)
    }

    fn clean_timestamp(occurred_at: Option<String>) -> AppResult<String> {
        match occurred_at {
            Some(value) if !value.trim().is_empty() => {
                let value = value.trim().to_string();
                if value.chars().count() > 40 {
                    return Err(AppError::BadRequest("occurred_at is too long".to_string()));
                }
                Ok(value)
            }
            _ => Ok(Utc::now().to_rfc3339()),
        }
    }

    pub async fn create_account(&self, request: CreateAccountRequest) -> AppResult<Account> {
        let name = request.name.trim().to_string();
        if name.is_empty() || name.chars().count() > 80 {
            return Err(AppError::BadRequest(
                "account name must be between 1 and 80 characters".to_string(),
            ));
        }
        let currency = request.currency.trim().to_ascii_uppercase();
        if currency != "INR" {
            return Err(AppError::BadRequest(
                "only INR accounts are supported in this milestone".to_string(),
            ));
        }

        sqlx::query_as::<_, Account>(
            "INSERT INTO accounts (name, currency, opening_balance_minor, created_at)
             VALUES (?, ?, ?, ?)
             RETURNING id, name, currency, opening_balance_minor, created_at",
        )
        .bind(name)
        .bind(currency)
        .bind(request.opening_balance_minor)
        .bind(Utc::now().to_rfc3339())
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)
    }

    async fn find_account(&self, account_id: i64) -> AppResult<Account> {
        sqlx::query_as::<_, Account>(
            "SELECT id, name, currency, opening_balance_minor, created_at
             FROM accounts WHERE id = ?",
        )
        .bind(account_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("account {account_id} was not found")))
    }

    pub async fn create_transaction(&self, request: CreateTransactionRequest) -> AppResult<i64> {
        Self::validate_amount(request.amount_minor)?;
        self.find_account(request.account_id).await?;
        let note = Self::clean_note(request.note)?;
        let occurred_at = Self::clean_timestamp(request.occurred_at)?;
        let created_at = Utc::now().to_rfc3339();

        let id = sqlx::query_scalar(
            "INSERT INTO entries
                (kind, account_id, amount_minor, note, occurred_at, created_at, transfer_id)
             VALUES (?, ?, ?, ?, ?, ?, NULL)
             RETURNING id",
        )
        .bind(request.kind.as_str())
        .bind(request.account_id)
        .bind(request.amount_minor)
        .bind(note)
        .bind(occurred_at)
        .bind(created_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn create_transfer(
        &self,
        request: CreateTransferRequest,
    ) -> AppResult<TransferResponse> {
        Self::validate_amount(request.amount_minor)?;
        if request.from_account_id == request.to_account_id {
            return Err(AppError::BadRequest(
                "from and to accounts must be different".to_string(),
            ));
        }
        let note = Self::clean_note(request.note)?;
        let occurred_at = Self::clean_timestamp(request.occurred_at)?;
        let created_at = Utc::now().to_rfc3339();

        let mut transaction = self.pool.begin().await?;

        let from_account = sqlx::query_as::<_, Account>(
            "SELECT id, name, currency, opening_balance_minor, created_at
             FROM accounts WHERE id = ?",
        )
        .bind(request.from_account_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| match error {
            sqlx::Error::RowNotFound => {
                AppError::NotFound(format!("account {} was not found", request.from_account_id))
            }
            other => AppError::Database(other),
        })?;

        let to_account = sqlx::query_as::<_, Account>(
            "SELECT id, name, currency, opening_balance_minor, created_at
             FROM accounts WHERE id = ?",
        )
        .bind(request.to_account_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| match error {
            sqlx::Error::RowNotFound => {
                AppError::NotFound(format!("account {} was not found", request.to_account_id))
            }
            other => AppError::Database(other),
        })?;

        if from_account.currency != to_account.currency {
            return Err(AppError::Conflict(
                "cross-currency transfers are not supported in this milestone".to_string(),
            ));
        }

        let transfer_id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO transfers
                (from_account_id, to_account_id, amount_minor, note, occurred_at, created_at)
             VALUES (?, ?, ?, ?, ?, ?)
             RETURNING id",
        )
        .bind(request.from_account_id)
        .bind(request.to_account_id)
        .bind(request.amount_minor)
        .bind(&note)
        .bind(&occurred_at)
        .bind(&created_at)
        .fetch_one(&mut *transaction)
        .await?;

        sqlx::query(
            "INSERT INTO entries
                (kind, account_id, amount_minor, note, occurred_at, created_at, transfer_id)
             VALUES ('transfer_out', ?, ?, ?, ?, ?, ?)",
        )
        .bind(request.from_account_id)
        .bind(request.amount_minor)
        .bind(&note)
        .bind(&occurred_at)
        .bind(&created_at)
        .bind(transfer_id)
        .execute(&mut *transaction)
        .await?;

        sqlx::query(
            "INSERT INTO entries
                (kind, account_id, amount_minor, note, occurred_at, created_at, transfer_id)
             VALUES ('transfer_in', ?, ?, ?, ?, ?, ?)",
        )
        .bind(request.to_account_id)
        .bind(request.amount_minor)
        .bind(&note)
        .bind(&occurred_at)
        .bind(&created_at)
        .bind(transfer_id)
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;

        Ok(TransferResponse {
            id: transfer_id,
            from_account_id: request.from_account_id,
            to_account_id: request.to_account_id,
            amount_minor: request.amount_minor,
        })
    }

    pub async fn state(&self) -> AppResult<StateResponse> {
        let accounts = sqlx::query_as::<_, AccountWithBalance>(
            "SELECT a.id, a.name, a.currency, a.opening_balance_minor, a.created_at,
                    COALESCE(
                        a.opening_balance_minor + SUM(
                            CASE e.kind
                                WHEN 'income' THEN e.amount_minor
                                WHEN 'expense' THEN -e.amount_minor
                                WHEN 'transfer_out' THEN -e.amount_minor
                                WHEN 'transfer_in' THEN e.amount_minor
                                ELSE 0
                            END
                        ),
                        a.opening_balance_minor
                    ) AS balance_minor
             FROM accounts a
             LEFT JOIN entries e ON e.account_id = a.id
             GROUP BY a.id
             ORDER BY a.id",
        )
        .fetch_all(&self.pool)
        .await?;

        let totals = sqlx::query_as::<_, Totals>(
            "SELECT
                COALESCE(SUM(CASE WHEN kind = 'income' THEN amount_minor ELSE 0 END), 0)
                    AS income_minor,
                COALESCE(SUM(CASE WHEN kind = 'expense' THEN amount_minor ELSE 0 END), 0)
                    AS expense_minor
             FROM entries",
        )
        .fetch_one(&self.pool)
        .await?;

        let entries = sqlx::query_as::<_, EntryView>(
            "SELECT e.id, e.kind, e.account_id, a.name AS account_name,
                    e.amount_minor, e.note, e.occurred_at, e.transfer_id
             FROM entries e
             JOIN accounts a ON a.id = e.account_id
             ORDER BY e.id DESC
             LIMIT 30",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(StateResponse {
            accounts,
            totals,
            entries,
        })
    }
}
