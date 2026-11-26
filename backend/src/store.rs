use crate::date::{date_only, is_valid_month, is_valid_timestamp, month_bounds};
use crate::error::{AppError, AppResult};
use crate::models::*;
use chrono::Utc;
use sqlx::{Executor, QueryBuilder, Sqlite, SqliteConnection, SqlitePool};
use std::collections::{BTreeMap, HashMap, HashSet};

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
                if !is_valid_timestamp(&value) {
                    return Err(AppError::BadRequest(
                        "occurred_at must be a valid date in YYYY-MM-DD or ISO 8601 date-time format"
                            .to_string(),
                    ));
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

    fn order_entries(entries: &mut [LedgerEntry]) {
        entries.sort_by(|left, right| {
            right
                .occurred_at
                .cmp(&left.occurred_at)
                .then(right.id.cmp(&left.id))
                .then(left.kind.cmp(&right.kind))
        });
    }

    fn build_page(
        mut entries: Vec<LedgerEntry>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> LedgerPage {
        Self::order_entries(&mut entries);
        let total_count = entries.len();
        let mut totals = BTreeMap::<String, DailyTotal>::new();
        for entry in &entries {
            let date: String = entry.occurred_at.chars().take(10).collect();
            let total = totals.entry(date).or_insert(DailyTotal {
                date: entry.occurred_at.chars().take(10).collect(),
                income_minor: 0,
                expense_minor: 0,
                transfer_minor: 0,
            });
            match entry.kind.as_str() {
                "income" => total.income_minor += entry.amount_minor,
                "expense" => total.expense_minor += entry.amount_minor,
                "transfer" | "transfer_in" | "transfer_out" => {
                    total.transfer_minor += entry.amount_minor
                }
                _ => {}
            }
        }
        let limit = limit.unwrap_or(200).clamp(1, 200);
        let offset = offset.unwrap_or(0);
        let end = offset.saturating_add(limit).min(total_count);
        let paged = if offset < total_count {
            entries[offset..end].to_vec()
        } else {
            Vec::new()
        };
        let next_offset = (end < total_count).then_some(end);
        LedgerPage {
            entries: paged,
            daily_totals: totals.into_values().collect(),
            total_count,
            next_offset,
        }
    }

    pub async fn ledger_entries(&self, filter: FilterDefinition) -> AppResult<Vec<LedgerEntry>> {
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
        Self::order_entries(&mut entries);
        Ok(entries)
    }

    pub async fn ledger(
        &self,
        filter: FilterDefinition,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> AppResult<LedgerPage> {
        Ok(Self::build_page(
            self.ledger_entries(filter).await?,
            limit,
            offset,
        ))
    }

    pub async fn account_history(
        &self,
        account_id: i64,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> AppResult<LedgerPage> {
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
        Ok(Self::build_page(entries, limit, offset))
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

    fn validate_budget_month(month: &str) -> AppResult<()> {
        if !is_valid_month(month) {
            return Err(AppError::BadRequest(
                "month must be a valid calendar month in YYYY-MM format".to_string(),
            ));
        }
        Ok(())
    }

    async fn budget_category(
        &self,
        category_id: i64,
    ) -> AppResult<(String, Option<String>, Option<i64>)> {
        sqlx::query_as::<_, (String, Option<String>, Option<i64>)>(
            "SELECT kind, archived_at, parent_id FROM categories WHERE id = ?",
        )
        .bind(category_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("category {category_id} was not found")))
    }

    fn validate_budget_category(
        category_id: i64,
        category: &(String, Option<String>, Option<i64>),
        existing_limit: Option<i64>,
    ) -> AppResult<()> {
        if category.0 != "expense" {
            return Err(AppError::BadRequest(
                "budgets can only be set for expense categories".to_string(),
            ));
        }
        if category.1.is_some() && existing_limit.is_none() {
            return Err(AppError::BadRequest(format!(
                "archived category {category_id} cannot receive a new budget"
            )));
        }
        Ok(())
    }

    pub async fn monthly_category_budgets(&self, month: &str) -> AppResult<MonthlyCategoryBudgets> {
        Self::validate_budget_month(month)?;
        let (first_day, next_month) = month_bounds(month)
            .ok_or_else(|| AppError::BadRequest("month must be in YYYY-MM format".to_string()))?;
        let expenses = sqlx::query_as::<_, (Option<i64>, i64, String)>(
            "SELECT category_id, amount_minor, occurred_at
             FROM entries
             WHERE kind = 'expense'",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut direct_spend = HashMap::<i64, i64>::new();
        let mut total_spent_minor = 0_i64;
        let mut uncategorized_spent_minor = 0_i64;
        let mut expense_count = 0_i64;
        let mut invalid_date_expense_count = 0_i64;
        let mut invalid_date_spent_minor = 0_i64;
        for (category_id, amount_minor, occurred_at) in expenses {
            let Some(date) = date_only(&occurred_at) else {
                invalid_date_expense_count += 1;
                invalid_date_spent_minor += amount_minor;
                continue;
            };
            if date.as_str() < first_day.as_str() || date.as_str() >= next_month.as_str() {
                continue;
            }
            total_spent_minor += amount_minor;
            expense_count += 1;
            if let Some(category_id) = category_id {
                *direct_spend.entry(category_id).or_default() += amount_minor;
            } else {
                uncategorized_spent_minor += amount_minor;
            }
        }

        let all_categories = self.categories().await?;
        let expense_categories: Vec<Category> = all_categories
            .into_iter()
            .filter(|category| category.kind == "expense")
            .collect();
        let category_by_id = expense_categories
            .iter()
            .map(|category| (category.id, category.clone()))
            .collect::<HashMap<_, _>>();
        let mut children = HashMap::<i64, Vec<i64>>::new();
        let mut roots = Vec::new();
        for category in &expense_categories {
            if let Some(parent_id) = category.parent_id {
                children.entry(parent_id).or_default().push(category.id);
            } else {
                roots.push(category.id);
            }
        }
        let compare_ids = |left: &i64, right: &i64| {
            let left = category_by_id.get(left).unwrap();
            let right = category_by_id.get(right).unwrap();
            left.sort_order
                .cmp(&right.sort_order)
                .then_with(|| left.name.cmp(&right.name))
                .then_with(|| left.id.cmp(&right.id))
        };
        roots.sort_by(compare_ids);
        for siblings in children.values_mut() {
            siblings.sort_by(compare_ids);
        }

        let mut ordered_ids = Vec::new();
        let mut visited = HashSet::new();
        let mut stack = roots.into_iter().rev().collect::<Vec<_>>();
        while let Some(category_id) = stack.pop() {
            if !visited.insert(category_id) {
                continue;
            }
            ordered_ids.push(category_id);
            if let Some(siblings) = children.get(&category_id) {
                stack.extend(siblings.iter().rev().copied());
            }
        }
        // Preserve legacy orphan/cycle branches instead of hiding their spending.
        for category in &expense_categories {
            if !visited.contains(&category.id) {
                ordered_ids.push(category.id);
                visited.insert(category.id);
            }
        }

        let limits = sqlx::query_as::<_, (i64, i64)>(
            "SELECT category_id, amount_minor FROM category_budgets WHERE month = ?",
        )
        .bind(month)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .collect::<HashMap<_, _>>();

        let categories = ordered_ids
            .into_iter()
            .filter_map(|category_id| category_by_id.get(&category_id).cloned())
            .map(|category| {
                let direct = direct_spend.get(&category.id).copied().unwrap_or_default();
                let spent = Self::rolled_category_spend(category.id, &direct_spend, &children);
                let limit = limits.get(&category.id).copied();
                CategoryBudget {
                    category_id: category.id,
                    name: category.name,
                    parent_id: category.parent_id,
                    parent_name: category.parent_name,
                    color: category.color,
                    icon: category.icon,
                    sort_order: category.sort_order,
                    archived_at: category.archived_at,
                    limit_minor: limit,
                    direct_spent_minor: direct,
                    spent_minor: spent,
                    remaining_minor: limit.map(|limit| limit - spent),
                }
            })
            .collect();

        Ok(MonthlyCategoryBudgets {
            month: month.to_string(),
            total_spent_minor,
            uncategorized_spent_minor,
            expense_count,
            invalid_date_expense_count,
            invalid_date_spent_minor,
            categories,
        })
    }

    fn rolled_category_spend(
        category_id: i64,
        direct_spend: &HashMap<i64, i64>,
        children: &HashMap<i64, Vec<i64>>,
    ) -> i64 {
        let mut stack = vec![category_id];
        let mut visited = HashSet::new();
        let mut total = 0_i64;
        while let Some(current) = stack.pop() {
            if !visited.insert(current) {
                continue;
            }
            total += direct_spend.get(&current).copied().unwrap_or_default();
            if let Some(descendants) = children.get(&current) {
                stack.extend(descendants.iter().copied());
            }
        }
        total
    }

    async fn validate_budget_hierarchy(
        &self,
        category_id: i64,
        parent_id: Option<i64>,
        month: &str,
        amount_minor: i64,
    ) -> AppResult<()> {
        if let Some(parent_id) = parent_id {
            let parent_limit = sqlx::query_scalar::<_, i64>(
                "SELECT amount_minor FROM category_budgets WHERE category_id = ? AND month = ?",
            )
            .bind(parent_id)
            .bind(month)
            .fetch_optional(&self.pool)
            .await?;
            if parent_limit.is_some_and(|limit| amount_minor > limit) {
                return Err(AppError::BadRequest(
                    "subcategory budget cannot exceed its parent category budget for the same month"
                        .to_string(),
                ));
            }
        }
        let child_over_limit = sqlx::query_scalar::<_, i64>(
            "SELECT b.category_id
             FROM category_budgets b
             JOIN categories c ON c.id = b.category_id
             WHERE c.parent_id = ? AND b.month = ? AND b.amount_minor > ?
             LIMIT 1",
        )
        .bind(category_id)
        .bind(month)
        .bind(amount_minor)
        .fetch_optional(&self.pool)
        .await?;
        if child_over_limit.is_some() {
            return Err(AppError::BadRequest(
                "parent category budget cannot be lower than a subcategory budget for the same month"
                    .to_string(),
            ));
        }
        Ok(())
    }

    async fn validate_category_move_budgets(
        &self,
        category_id: i64,
        new_parent_id: Option<i64>,
    ) -> AppResult<()> {
        let Some(parent_id) = new_parent_id else {
            return Ok(());
        };
        let violation = sqlx::query_scalar::<_, String>(
            "SELECT b.month
             FROM category_budgets b
             JOIN category_budgets pb ON pb.category_id = ? AND pb.month = b.month
             WHERE b.category_id = ? AND b.amount_minor > pb.amount_minor
             LIMIT 1",
        )
        .bind(parent_id)
        .bind(category_id)
        .fetch_optional(&self.pool)
        .await?;
        if violation.is_some() {
            return Err(AppError::BadRequest(
                "subcategory budget cannot exceed its new parent category budget for the same month"
                    .to_string(),
            ));
        }
        Ok(())
    }

    pub async fn set_category_budget(
        &self,
        category_id: i64,
        month: &str,
        amount_minor: i64,
    ) -> AppResult<MonthlyCategoryBudgets> {
        Self::validate_budget_month(month)?;
        Self::validate_amount(amount_minor)?;
        let category = self.budget_category(category_id).await?;
        let existing_limit = sqlx::query_scalar::<_, i64>(
            "SELECT amount_minor FROM category_budgets WHERE category_id = ? AND month = ?",
        )
        .bind(category_id)
        .bind(month)
        .fetch_optional(&self.pool)
        .await?;
        Self::validate_budget_category(category_id, &category, existing_limit)?;
        self.validate_budget_hierarchy(category_id, category.2, month, amount_minor)
            .await?;
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO category_budgets (category_id, month, amount_minor, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(category_id, month) DO UPDATE SET
                 amount_minor = excluded.amount_minor,
                 updated_at = excluded.updated_at",
        )
        .bind(category_id)
        .bind(month)
        .bind(amount_minor)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.monthly_category_budgets(month).await
    }

    pub async fn remove_category_budget(
        &self,
        category_id: i64,
        month: &str,
    ) -> AppResult<MonthlyCategoryBudgets> {
        Self::validate_budget_month(month)?;
        let category = self.budget_category(category_id).await?;
        Self::validate_budget_category(category_id, &category, Some(0))?;
        sqlx::query("DELETE FROM category_budgets WHERE category_id = ? AND month = ?")
            .bind(category_id)
            .bind(month)
            .execute(&self.pool)
            .await?;
        self.monthly_category_budgets(month).await
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
        let parent = sqlx::query_as::<_, (String, Option<String>, Option<i64>)>(
            "SELECT kind, archived_at, parent_id FROM categories WHERE id = ?",
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
        if parent.2.is_some() {
            return Err(AppError::BadRequest(
                "subcategories can only be created under top-level categories".to_string(),
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
        let children =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM categories WHERE parent_id = ?")
                .bind(id)
                .fetch_one(&self.pool)
                .await?;
        if children > 0 && current.parent_id != request.parent_id {
            return Err(AppError::BadRequest(
                "Move this category’s subcategories first.".to_string(),
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
            let budgets = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM category_budgets WHERE category_id = ?",
            )
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
            if budgets > 0 {
                return Err(AppError::BadRequest(
                    "category type cannot change while it has saved budgets".to_string(),
                ));
            }
            if references > 0 || children > 0 {
                return Err(AppError::BadRequest(
                    "category type cannot change after it has entries or child categories"
                        .to_string(),
                ));
            }
        }
        self.validate_category_parent(&request.kind, request.parent_id, Some(id))
            .await?;
        if current.parent_id != request.parent_id {
            self.validate_category_move_budgets(id, request.parent_id)
                .await?;
        }
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
        let budgets = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM category_budgets WHERE category_id = ?",
        )
        .bind(id)
        .fetch_one(&mut *transaction)
        .await?;
        let archived = references > 0 || children > 0 || budgets > 0;
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
        let entries = self
            .ledger(FilterDefinition::default(), None, None)
            .await?
            .entries;

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
