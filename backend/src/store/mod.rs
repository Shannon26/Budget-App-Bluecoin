use crate::date::is_valid_timestamp;
use crate::error::{AppError, AppResult};
use crate::models::*;
use chrono::Utc;
use sqlx::SqlitePool;
use sqlx::{Executor, Sqlite};

#[derive(Clone)]
pub struct Store {
    pub(super) pool: SqlitePool,
}

impl Store {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub(super) fn validate_amount(amount_minor: i64) -> AppResult<()> {
        if amount_minor <= 0 {
            return Err(AppError::BadRequest(
                "amount must be greater than zero".to_string(),
            ));
        }
        Ok(())
    }

    pub(super) fn clean_note(note: Option<String>) -> AppResult<String> {
        let note = note.unwrap_or_default().trim().to_string();
        if note.chars().count() > 500 {
            return Err(AppError::BadRequest(
                "note must be 500 characters or fewer".to_string(),
            ));
        }
        Ok(note)
    }

    pub(super) fn clean_name(name: String, note: &str, fallback: &str) -> AppResult<String> {
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

    pub(super) fn clean_timestamp(occurred_at: Option<String>) -> AppResult<String> {
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

    pub(super) async fn load_account<'e, E>(executor: E, account_id: i64) -> AppResult<Account>
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

mod accounts;
mod budget_rules;
mod budgets;
mod categories;
mod filters;
mod ledger;
mod transactions;
mod transfers;
