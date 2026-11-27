use crate::date::is_valid_timestamp;
use crate::error::{AppError, AppResult};
use crate::models::*;
use chrono::Utc;
use sqlx::SqlitePool;
use sqlx::{Executor, Sqlite};
use std::collections::HashMap;

#[derive(Clone)]
pub struct Store {
    pub(super) pool: SqlitePool,
}

impl Store {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub(super) fn checked_i64(value: i128, label: &str) -> AppResult<i64> {
        i64::try_from(value).map_err(|_| {
            AppError::Conflict(format!("{label} is outside the supported integer range"))
        })
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
            "SELECT id, name, currency, opening_balance_minor, created_at, account_type,
                    note, opening_date, hide_from_selection, hide_from_reports,
                    credit_limit_minor, cutoff_day, due_day
             FROM accounts WHERE id = ?",
        )
        .bind(account_id)
        .fetch_optional(executor)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("account {account_id} was not found")))
    }

    pub async fn state(&self) -> AppResult<StateResponse> {
        let account_rows = sqlx::query_as::<_, Account>(
            "SELECT id, name, currency, opening_balance_minor, created_at, account_type,
                    note, opening_date, hide_from_selection, hide_from_reports,
                    credit_limit_minor, cutoff_day, due_day
             FROM accounts ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;
        let entry_deltas = sqlx::query_as::<_, (i64, String, i64)>(
            "SELECT account_id, kind, amount_minor FROM entries",
        )
        .fetch_all(&self.pool)
        .await?;

        // Accumulate in i128 so intermediate additions cannot wrap at the i64 API
        // boundary. The final signed balance/total must still fit in i64.
        let mut balances: HashMap<i64, i128> = account_rows
            .iter()
            .map(|account| (account.id, i128::from(account.opening_balance_minor)))
            .collect();
        let mut income_minor = 0_i128;
        let mut expense_minor = 0_i128;
        for (account_id, kind, amount_minor) in entry_deltas {
            let delta = match kind.as_str() {
                "income" => i128::from(amount_minor),
                "expense" | "transfer_out" => -i128::from(amount_minor),
                "transfer_in" => i128::from(amount_minor),
                _ => 0,
            };
            if let Some(balance) = balances.get_mut(&account_id) {
                *balance += delta;
            }
            if kind == "income" {
                income_minor += i128::from(amount_minor);
            } else if kind == "expense" {
                expense_minor += i128::from(amount_minor);
            }
        }

        let accounts = account_rows
            .into_iter()
            .map(|account| {
                let balance = i64::try_from(balances.get(&account.id).copied().unwrap_or(0))
                    .map_err(|_| {
                        AppError::Conflict(
                            "account balance is outside the supported integer range".to_string(),
                        )
                    })?;
                Ok(AccountWithBalance {
                    id: account.id,
                    name: account.name,
                    currency: account.currency,
                    opening_balance_minor: account.opening_balance_minor,
                    created_at: account.created_at,
                    note: account.note,
                    opening_date: account.opening_date,
                    hide_from_selection: account.hide_from_selection,
                    hide_from_reports: account.hide_from_reports,
                    balance_minor: balance,
                    account_type: account.account_type,
                    account_group: account.account_group,
                    credit_limit_minor: account.credit_limit_minor,
                    cutoff_day: account.cutoff_day,
                    due_day: account.due_day,
                })
            })
            .collect::<AppResult<Vec<_>>>()?;
        let totals = Totals {
            income_minor: i64::try_from(income_minor).map_err(|_| {
                AppError::Conflict(
                    "ledger total is outside the supported integer range".to_string(),
                )
            })?,
            expense_minor: i64::try_from(expense_minor).map_err(|_| {
                AppError::Conflict(
                    "ledger total is outside the supported integer range".to_string(),
                )
            })?,
        };
        let categories = self.categories().await?;
        let category_totals = self.category_totals().await?;
        let saved_filters = self.saved_filters().await?;
        let labels = self.labels().await?;
        let entries = self
            .ledger(FilterDefinition::default(), None, None)
            .await?
            .entries;

        Ok(StateResponse {
            accounts,
            account_types: ACCOUNT_TYPES.to_vec(),
            totals,
            categories,
            category_totals,
            saved_filters,
            labels,
            entries,
        })
    }
}

mod accounts;
mod budget_rules;
mod budgets;
mod categories;
mod credit_cards;
mod drafts;
mod filters;
mod household;
mod labels;
mod ledger;
mod permissions;
mod reports;
mod transactions;
mod transfer_requests;
mod transfers;
