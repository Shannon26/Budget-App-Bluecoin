use crate::error::{AppError, AppResult};
use crate::models::*;
use chrono::Utc;
use sqlx::{Executor, QueryBuilder, Sqlite, SqliteConnection, SqlitePool};

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

    fn clean_name(name: String, note: &str, fallback: &str) -> AppResult<String> {
        let name = name.trim().to_string();
        let value = if name.is_empty() {
            let note = note.trim();
            if note.is_empty() {
                fallback.to_string()
            } else {
                note.to_string()
            }
        } else {
            name
        };
        if value.chars().count() > 120 {
            return Err(AppError::BadRequest(
                "transaction name must be 120 characters or fewer".to_string(),
            ));
        }
        Ok(value)
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

    async fn load_account<'e, E>(executor: E, account_id: i64) -> AppResult<Account>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query_as::<_, Account>(
            "SELECT id, name, currency, opening_balance_minor, created_at
             FROM accounts WHERE id = ?",
        )
        .bind(account_id)
        .fetch_optional(executor)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("account {account_id} was not found")))
    }

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

        Ok(transfer_id)
    }

    pub async fn create_transfer(&self, request: TransferRequest) -> AppResult<TransferResponse> {
        let mut transaction = self.pool.begin().await?;
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
        let mut transaction = self.pool.begin().await?;
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

    fn validate_filter(filter: &FilterDefinition) -> AppResult<()> {
        if filter.amount_min_minor.is_some_and(|amount| amount < 0)
            || filter.amount_max_minor.is_some_and(|amount| amount < 0)
        {
            return Err(AppError::BadRequest(
                "amount filters cannot be negative".to_string(),
            ));
        }
        if let (Some(min), Some(max)) = (filter.amount_min_minor, filter.amount_max_minor) {
            if min > max {
                return Err(AppError::BadRequest(
                    "minimum amount cannot be greater than maximum amount".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn apply_entry_filters(builder: &mut QueryBuilder<Sqlite>, filter: &FilterDefinition) {
        if let Some(search) = filter
            .search
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            let pattern = format!("%{}%", search.to_lowercase());
            builder.push(" AND (lower(e.name) LIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" OR lower(e.note) LIKE ");
            builder.push_bind(pattern);
            builder.push(")");
        }
        if let Some(from) = &filter.date_from {
            builder.push(" AND substr(e.occurred_at, 1, 10) >= ");
            builder.push_bind(from.clone());
        }
        if let Some(to) = &filter.date_to {
            builder.push(" AND substr(e.occurred_at, 1, 10) <= ");
            builder.push_bind(to.clone());
        }
        if let Some(account_id) = filter.account_id {
            builder.push(" AND e.account_id = ");
            builder.push_bind(account_id);
        }
        if let Some(category_id) = filter.category_id {
            builder.push(" AND e.category_id = ");
            builder.push_bind(category_id);
        } else if filter.uncategorized.unwrap_or(false) {
            builder.push(" AND e.category_id IS NULL");
        }
        if let Some(reconciled) = filter.reconciled {
            builder.push(" AND e.reconciled = ");
            builder.push_bind(reconciled);
        }
        if let Some(min) = filter.amount_min_minor {
            builder.push(" AND e.amount_minor >= ");
            builder.push_bind(min);
        }
        if let Some(max) = filter.amount_max_minor {
            builder.push(" AND e.amount_minor <= ");
            builder.push_bind(max);
        }
    }

    fn apply_transfer_filters(builder: &mut QueryBuilder<Sqlite>, filter: &FilterDefinition) {
        if filter.category_id.is_some() || filter.uncategorized.unwrap_or(false) {
            builder.push(" AND 0 = 1");
        }
        if let Some(search) = filter
            .search
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            let pattern = format!("%{}%", search.to_lowercase());
            builder.push(" AND (lower(t.name) LIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" OR lower(t.note) LIKE ");
            builder.push_bind(pattern);
            builder.push(")");
        }
        if let Some(from) = &filter.date_from {
            builder.push(" AND substr(t.occurred_at, 1, 10) >= ");
            builder.push_bind(from.clone());
        }
        if let Some(to) = &filter.date_to {
            builder.push(" AND substr(t.occurred_at, 1, 10) <= ");
            builder.push_bind(to.clone());
        }
        if let Some(account_id) = filter.account_id {
            builder.push(" AND (t.from_account_id = ");
            builder.push_bind(account_id);
            builder.push(" OR t.to_account_id = ");
            builder.push_bind(account_id);
            builder.push(")");
        }
        if let Some(reconciled) = filter.reconciled {
            builder.push(" AND oe.reconciled = ");
            builder.push_bind(reconciled);
        }
        if let Some(min) = filter.amount_min_minor {
            builder.push(" AND t.amount_minor >= ");
            builder.push_bind(min);
        }
        if let Some(max) = filter.amount_max_minor {
            builder.push(" AND t.amount_minor <= ");
            builder.push_bind(max);
        }
    }

    pub async fn ledger(&self, filter: FilterDefinition) -> AppResult<Vec<LedgerEntry>> {
        Self::validate_filter(&filter)?;
        let mut entries_query = QueryBuilder::<Sqlite>::new(
            "SELECT e.id, e.kind, e.name, e.note, e.account_id, a.name AS account_name,
                    NULL AS counterpart_account_id, NULL AS counterpart_account_name,
                    e.amount_minor, e.occurred_at, e.category_id, c.name AS category_name,
                    p.name AS category_parent_name, e.reconciled, e.transfer_id
             FROM entries e
             JOIN accounts a ON a.id = e.account_id
             LEFT JOIN categories c ON c.id = e.category_id
             LEFT JOIN categories p ON p.id = c.parent_id
             WHERE e.kind IN ('income', 'expense')",
        );
        Self::apply_entry_filters(&mut entries_query, &filter);
        entries_query.push(" ORDER BY e.occurred_at DESC, e.id DESC");
        let mut entries = entries_query
            .build_query_as::<LedgerEntry>()
            .fetch_all(&self.pool)
            .await?;

        let mut transfers_query = QueryBuilder::<Sqlite>::new(
            "SELECT t.id, 'transfer' AS kind, t.name, t.note,
                    t.from_account_id AS account_id, fa.name AS account_name,
                    t.to_account_id AS counterpart_account_id, ta.name AS counterpart_account_name,
                    t.amount_minor, t.occurred_at, NULL AS category_id, NULL AS category_name,
                    NULL AS category_parent_name, oe.reconciled, t.id AS transfer_id
             FROM transfers t
             JOIN accounts fa ON fa.id = t.from_account_id
             JOIN accounts ta ON ta.id = t.to_account_id
             JOIN entries oe ON oe.transfer_id = t.id AND oe.kind = 'transfer_out'
             WHERE 1 = 1",
        );
        Self::apply_transfer_filters(&mut transfers_query, &filter);
        transfers_query.push(" ORDER BY t.occurred_at DESC, t.id DESC");
        let transfers = transfers_query
            .build_query_as::<LedgerEntry>()
            .fetch_all(&self.pool)
            .await?;
        entries.extend(transfers);
        entries.sort_by(|left, right| {
            right
                .occurred_at
                .cmp(&left.occurred_at)
                .then(right.id.cmp(&left.id))
        });
        entries.truncate(200);
        Ok(entries)
    }

    pub async fn account_history(&self, account_id: i64) -> AppResult<Vec<LedgerEntry>> {
        Store::load_account(&self.pool, account_id).await?;
        let mut entries_query = QueryBuilder::<Sqlite>::new(
            "SELECT e.id, e.kind, e.name, e.note, e.account_id, a.name AS account_name,
                    NULL AS counterpart_account_id, NULL AS counterpart_account_name,
                    e.amount_minor, e.occurred_at, e.category_id, c.name AS category_name,
                    p.name AS category_parent_name, e.reconciled, e.transfer_id
             FROM entries e
             JOIN accounts a ON a.id = e.account_id
             LEFT JOIN categories c ON c.id = e.category_id
             LEFT JOIN categories p ON p.id = c.parent_id
             WHERE e.account_id = ",
        );
        entries_query.push_bind(account_id);
        entries_query
            .push(" AND e.kind IN ('income', 'expense') ORDER BY e.occurred_at DESC, e.id DESC");
        let mut entries = entries_query
            .build_query_as::<LedgerEntry>()
            .fetch_all(&self.pool)
            .await?;

        let mut transfers_query = QueryBuilder::<Sqlite>::new(
            "SELECT t.id,
                    CASE WHEN t.from_account_id = e.account_id THEN 'transfer_out' ELSE 'transfer_in' END AS kind,
                    t.name, t.note, e.account_id, a.name AS account_name,
                    CASE WHEN t.from_account_id = e.account_id THEN t.to_account_id ELSE t.from_account_id END AS counterpart_account_id,
                    CASE WHEN t.from_account_id = e.account_id THEN ta.name ELSE fa.name END AS counterpart_account_name,
                    t.amount_minor, t.occurred_at, NULL AS category_id, NULL AS category_name,
                    NULL AS category_parent_name, e.reconciled, t.id AS transfer_id
             FROM transfers t
             JOIN entries e ON e.transfer_id = t.id
             JOIN accounts a ON a.id = e.account_id
             JOIN accounts fa ON fa.id = t.from_account_id
             JOIN accounts ta ON ta.id = t.to_account_id
             WHERE e.account_id = ",
        );
        transfers_query.push_bind(account_id);
        transfers_query.push(
            " AND e.kind IN ('transfer_in', 'transfer_out') ORDER BY t.occurred_at DESC, t.id DESC",
        );
        let transfers = transfers_query
            .build_query_as::<LedgerEntry>()
            .fetch_all(&self.pool)
            .await?;
        entries.extend(transfers);
        entries.sort_by(|left, right| {
            right
                .occurred_at
                .cmp(&left.occurred_at)
                .then(right.id.cmp(&left.id))
        });
        entries.truncate(200);
        Ok(entries)
    }

    pub async fn transaction_detail(&self, id: i64) -> AppResult<LedgerEntry> {
        sqlx::query_as::<_, LedgerEntry>(
            "SELECT e.id, e.kind, e.name, e.note, e.account_id, a.name AS account_name,
                    NULL AS counterpart_account_id, NULL AS counterpart_account_name,
                    e.amount_minor, e.occurred_at, e.category_id, c.name AS category_name,
                    p.name AS category_parent_name, e.reconciled, e.transfer_id
             FROM entries e
             JOIN accounts a ON a.id = e.account_id
             LEFT JOIN categories c ON c.id = e.category_id
             LEFT JOIN categories p ON p.id = c.parent_id
             WHERE e.id = ? AND e.transfer_id IS NULL",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("transaction {id} was not found")))
    }

    pub async fn transfer_detail(&self, id: i64) -> AppResult<LedgerEntry> {
        sqlx::query_as::<_, LedgerEntry>(
            "SELECT t.id, 'transfer' AS kind, t.name, t.note,
                    t.from_account_id AS account_id, fa.name AS account_name,
                    t.to_account_id AS counterpart_account_id, ta.name AS counterpart_account_name,
                    t.amount_minor, t.occurred_at, NULL AS category_id, NULL AS category_name,
                    NULL AS category_parent_name, oe.reconciled, t.id AS transfer_id
             FROM transfers t
             JOIN accounts fa ON fa.id = t.from_account_id
             JOIN accounts ta ON ta.id = t.to_account_id
             JOIN entries oe ON oe.transfer_id = t.id AND oe.kind = 'transfer_out'
             WHERE t.id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("transfer {id} was not found")))
    }

    pub async fn categories(&self) -> AppResult<Vec<Category>> {
        sqlx::query_as::<_, Category>(
            "SELECT c.id, c.name, c.kind, c.parent_id, p.name AS parent_name,
                    c.color, c.icon, c.sort_order, c.archived_at, c.created_at
             FROM categories c
             LEFT JOIN categories p ON p.id = c.parent_id
             ORDER BY c.kind, c.sort_order, c.name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)
    }

    pub async fn category_totals(&self) -> AppResult<Vec<CategoryTotal>> {
        sqlx::query_as::<_, CategoryTotal>(
            "SELECT c.id AS category_id, e.kind,
                    COALESCE(c.name, 'Uncategorized') AS name,
                    SUM(e.amount_minor) AS total_minor, COUNT(*) AS entry_count
             FROM entries e
             LEFT JOIN categories c ON c.id = e.category_id
             WHERE e.kind IN ('income', 'expense')
             GROUP BY c.id, e.kind
             ORDER BY e.kind, name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)
    }

    async fn validate_category_parent(
        &self,
        kind: &str,
        parent_id: Option<i64>,
        current_id: Option<i64>,
    ) -> AppResult<()> {
        let Some(parent_id) = parent_id else {
            return Ok(());
        };
        if current_id == Some(parent_id) {
            return Err(AppError::BadRequest(
                "a category cannot be its own parent".to_string(),
            ));
        }
        let parent = sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT kind, archived_at FROM categories WHERE id = ?",
        )
        .bind(parent_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::BadRequest("parent category was not found".to_string()))?;
        if parent.0 != kind {
            return Err(AppError::BadRequest(
                "parent category must have the same type".to_string(),
            ));
        }
        if parent.1.is_some() {
            return Err(AppError::BadRequest(
                "archived categories cannot be parents".to_string(),
            ));
        }
        if let Some(current_id) = current_id {
            let mut cursor = parent_id;
            for _ in 0..100 {
                if cursor == current_id {
                    return Err(AppError::BadRequest(
                        "category hierarchy cannot contain a cycle".to_string(),
                    ));
                }
                let next = sqlx::query_scalar::<_, Option<i64>>(
                    "SELECT parent_id FROM categories WHERE id = ?",
                )
                .bind(cursor)
                .fetch_optional(&self.pool)
                .await?
                .flatten();
                let Some(next) = next else { break };
                cursor = next;
            }
        }
        Ok(())
    }

    pub async fn create_category(&self, request: CategoryRequest) -> AppResult<Category> {
        let name = request.name.trim().to_string();
        if name.is_empty() || name.chars().count() > 80 {
            return Err(AppError::BadRequest(
                "category name must be between 1 and 80 characters".to_string(),
            ));
        }
        if request.kind != "income" && request.kind != "expense" {
            return Err(AppError::BadRequest(
                "category type must be income or expense".to_string(),
            ));
        }
        if !Self::valid_color(&request.color) {
            return Err(AppError::BadRequest(
                "color must be a hex value like #426b8a".to_string(),
            ));
        }
        if request.icon.chars().count() > 8 {
            return Err(AppError::BadRequest(
                "icon must be 8 characters or fewer".to_string(),
            ));
        }
        self.validate_category_parent(&request.kind, request.parent_id, None)
            .await?;
        let duplicate = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM categories
             WHERE kind = ? AND parent_id IS ? AND lower(name) = lower(?) AND archived_at IS NULL",
        )
        .bind(&request.kind)
        .bind(request.parent_id)
        .bind(&name)
        .fetch_one(&self.pool)
        .await?;
        if duplicate > 0 {
            return Err(AppError::BadRequest(
                "a category with this name already exists here".to_string(),
            ));
        }
        sqlx::query_as::<_, Category>(
            "INSERT INTO categories (name, kind, parent_id, color, icon, sort_order, archived_at, created_at)
             VALUES (?, ?, ?, ?, ?, ?, NULL, ?)
             RETURNING id, name, kind, parent_id, NULL AS parent_name, color, icon, sort_order, archived_at, created_at",
        )
        .bind(name)
        .bind(request.kind)
        .bind(request.parent_id)
        .bind(request.color)
        .bind(request.icon)
        .bind(request.sort_order)
        .bind(Utc::now().to_rfc3339())
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)
    }

    fn valid_color(color: &str) -> bool {
        color.len() == 7
            && color.starts_with('#')
            && color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
    }

    pub async fn update_category(&self, id: i64, request: CategoryRequest) -> AppResult<Category> {
        let name = request.name.trim().to_string();
        if name.is_empty() || name.chars().count() > 80 {
            return Err(AppError::BadRequest(
                "category name must be between 1 and 80 characters".to_string(),
            ));
        }
        if request.kind != "income" && request.kind != "expense" {
            return Err(AppError::BadRequest(
                "category type must be income or expense".to_string(),
            ));
        }
        if !Self::valid_color(&request.color) {
            return Err(AppError::BadRequest(
                "color must be a hex value like #426b8a".to_string(),
            ));
        }
        if request.icon.chars().count() > 8 {
            return Err(AppError::BadRequest(
                "icon must be 8 characters or fewer".to_string(),
            ));
        }
        let current = self
            .categories()
            .await?
            .into_iter()
            .find(|category| category.id == id)
            .ok_or_else(|| AppError::NotFound(format!("category {id} was not found")))?;
        if current.archived_at.is_some() {
            return Err(AppError::BadRequest(
                "archived categories cannot be edited".to_string(),
            ));
        }
        if current.kind != request.kind {
            let references = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM entries WHERE category_id = ? AND kind = ?",
            )
            .bind(id)
            .bind(&current.kind)
            .fetch_one(&self.pool)
            .await?;
            let children =
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM categories WHERE parent_id = ?")
                    .bind(id)
                    .fetch_one(&self.pool)
                    .await?;
            if references > 0 || children > 0 {
                return Err(AppError::BadRequest(
                    "category type cannot change after it has entries or child categories"
                        .to_string(),
                ));
            }
        }
        self.validate_category_parent(&request.kind, request.parent_id, Some(id))
            .await?;
        let duplicate = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM categories
             WHERE id <> ? AND kind = ? AND parent_id IS ? AND lower(name) = lower(?) AND archived_at IS NULL",
        )
        .bind(id)
        .bind(&request.kind)
        .bind(request.parent_id)
        .bind(&name)
        .fetch_one(&self.pool)
        .await?;
        if duplicate > 0 {
            return Err(AppError::BadRequest(
                "a category with this name already exists here".to_string(),
            ));
        }
        sqlx::query(
            "UPDATE categories SET name = ?, kind = ?, parent_id = ?, color = ?, icon = ?, sort_order = ? WHERE id = ?",
        )
        .bind(name)
        .bind(request.kind)
        .bind(request.parent_id)
        .bind(request.color)
        .bind(request.icon)
        .bind(request.sort_order)
        .bind(id)
        .execute(&self.pool)
        .await?;
        self.categories()
            .await?
            .into_iter()
            .find(|category| category.id == id)
            .ok_or_else(|| AppError::NotFound(format!("category {id} was not found")))
    }

    pub async fn delete_category(&self, id: i64) -> AppResult<DeleteCategoryResponse> {
        let mut transaction = self.pool.begin().await?;
        let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM categories WHERE id = ?")
            .bind(id)
            .fetch_one(&mut *transaction)
            .await?;
        if exists == 0 {
            return Err(AppError::NotFound(format!("category {id} was not found")));
        }
        let references =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM entries WHERE category_id = ?")
                .bind(id)
                .fetch_one(&mut *transaction)
                .await?;
        let children =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM categories WHERE parent_id = ?")
                .bind(id)
                .fetch_one(&mut *transaction)
                .await?;
        let archived = references > 0 || children > 0;
        if archived {
            sqlx::query("UPDATE categories SET archived_at = ? WHERE id = ?")
                .bind(Utc::now().to_rfc3339())
                .bind(id)
                .execute(&mut *transaction)
                .await?;
        } else {
            sqlx::query("DELETE FROM categories WHERE id = ?")
                .bind(id)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(DeleteCategoryResponse { archived })
    }

    pub async fn saved_filters(&self) -> AppResult<Vec<SavedFilter>> {
        let rows = sqlx::query_as::<_, SavedFilterRow>(
            "SELECT id, name, definition_json, created_at, updated_at FROM saved_filters ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| row.into_saved_filter().map(|(_, filter)| filter))
            .collect()
    }

    pub async fn create_saved_filter(&self, request: SavedFilterRequest) -> AppResult<SavedFilter> {
        Self::validate_filter(&request.definition)?;
        let name = request.name.trim().to_string();
        if name.is_empty() || name.chars().count() > 80 {
            return Err(AppError::BadRequest(
                "filter name must be between 1 and 80 characters".to_string(),
            ));
        }
        let now = Utc::now().to_rfc3339();
        let json = serde_json::to_string(&request.definition)
            .map_err(|error| AppError::BadRequest(error.to_string()))?;
        let row = sqlx::query_as::<_, SavedFilterRow>(
            "INSERT INTO saved_filters (name, definition_json, created_at, updated_at)
             VALUES (?, ?, ?, ?)
             RETURNING id, name, definition_json, created_at, updated_at",
        )
        .bind(name)
        .bind(json)
        .bind(&now)
        .bind(&now)
        .fetch_one(&self.pool)
        .await?;
        row.into_saved_filter().map(|(_, filter)| filter)
    }

    pub async fn update_saved_filter(
        &self,
        id: i64,
        request: SavedFilterRequest,
    ) -> AppResult<SavedFilter> {
        Self::validate_filter(&request.definition)?;
        let name = request.name.trim().to_string();
        if name.is_empty() || name.chars().count() > 80 {
            return Err(AppError::BadRequest(
                "filter name must be between 1 and 80 characters".to_string(),
            ));
        }
        let json = serde_json::to_string(&request.definition)
            .map_err(|error| AppError::BadRequest(error.to_string()))?;
        let row = sqlx::query_as::<_, SavedFilterRow>(
            "UPDATE saved_filters SET name = ?, definition_json = ?, updated_at = ?
             WHERE id = ?
             RETURNING id, name, definition_json, created_at, updated_at",
        )
        .bind(name)
        .bind(json)
        .bind(Utc::now().to_rfc3339())
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("saved filter {id} was not found")))?;
        row.into_saved_filter().map(|(_, filter)| filter)
    }

    pub async fn delete_saved_filter(&self, id: i64) -> AppResult<()> {
        let result = sqlx::query("DELETE FROM saved_filters WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!(
                "saved filter {id} was not found"
            )));
        }
        Ok(())
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
        let categories = self.categories().await?;
        let category_totals = self.category_totals().await?;
        let saved_filters = self.saved_filters().await?;
        let entries = self.ledger(FilterDefinition::default()).await?;

        Ok(StateResponse {
            accounts,
            totals,
            categories,
            category_totals,
            saved_filters,
            entries,
        })
    }
}
