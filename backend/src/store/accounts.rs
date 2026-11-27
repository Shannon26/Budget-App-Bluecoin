use super::Store;
use crate::date::is_valid_calendar_date;
use crate::error::{AppError, AppResult};
use crate::models::*;
use chrono::Utc;

impl Store {
    fn validate_account_name(value: String) -> AppResult<String> {
        let name = value.trim().to_string();
        if name.is_empty() || name.chars().count() > 80 {
            return Err(AppError::BadRequest(
                "account name must be between 1 and 80 characters".to_string(),
            ));
        }
        Ok(name)
    }

    fn validate_account_type(value: Option<String>) -> AppResult<String> {
        let value = value.unwrap_or_else(|| UNCLASSIFIED_ACCOUNT_TYPE.to_string());
        if account_type_definition(&value).is_none() {
            return Err(AppError::BadRequest(
                "account type is not supported".to_string(),
            ));
        }
        Ok(value)
    }

    fn clean_opening_date(value: Option<String>) -> AppResult<Option<String>> {
        match value {
            Some(value) if value.is_empty() => Ok(None),
            Some(value) if is_valid_calendar_date(&value) => Ok(Some(value)),
            Some(_) => Err(AppError::BadRequest(
                "opening date must be a valid YYYY-MM-DD calendar date".to_string(),
            )),
            None => Ok(None),
        }
    }

    pub async fn create_account(&self, request: CreateAccountRequest) -> AppResult<Account> {
        let name = Self::validate_account_name(request.name)?;
        let currency = request.currency.trim().to_ascii_uppercase();
        if currency != "INR" {
            return Err(AppError::BadRequest(
                "only INR accounts are supported in this milestone".to_string(),
            ));
        }
        let account_type = Self::validate_account_type(request.account_type)?;
        let note = Self::clean_note(request.note)?;
        let opening_date = Self::clean_opening_date(request.opening_date)?;

        sqlx::query_as::<_, Account>(
            "INSERT INTO accounts
                (name, currency, opening_balance_minor, created_at, account_type, note,
                 opening_date, hide_from_selection, hide_from_reports)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             RETURNING id, name, currency, opening_balance_minor, created_at, account_type,
                       note, opening_date, hide_from_selection, hide_from_reports",
        )
        .bind(name)
        .bind(currency)
        .bind(request.opening_balance_minor)
        .bind(Utc::now().to_rfc3339())
        .bind(account_type)
        .bind(note)
        .bind(opening_date)
        .bind(request.hide_from_selection)
        .bind(request.hide_from_reports)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)
    }

    pub async fn update_account(
        &self,
        id: i64,
        request: UpdateAccountRequest,
    ) -> AppResult<Account> {
        // Serialize the read/validate/write sequence so omitted fields are preserved
        // against concurrent partial edits as well as within this request.
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let current = Store::load_account(&mut *transaction, id).await?;

        let name = match request.name {
            Some(name) => Self::validate_account_name(name)?,
            None => current.name.clone(),
        };
        let note = match request.note {
            Some(note) => Self::clean_note(note)?,
            None => current.note.clone(),
        };
        let opening_date = match request.opening_date {
            Some(opening_date) => Self::clean_opening_date(opening_date)?,
            None => current.opening_date.clone(),
        };
        let account_type = match request.account_type {
            Some(account_type) => Self::validate_account_type(Some(account_type))?,
            None => current.account_type.clone(),
        };
        let hide_from_selection = request
            .hide_from_selection
            .unwrap_or(current.hide_from_selection);
        let hide_from_reports = request
            .hide_from_reports
            .unwrap_or(current.hide_from_reports);

        let updated = sqlx::query_as::<_, Account>(
            "UPDATE accounts
             SET name = ?, note = ?, opening_date = ?, account_type = ?,
                 hide_from_selection = ?, hide_from_reports = ?
             WHERE id = ?
             RETURNING id, name, currency, opening_balance_minor, created_at, account_type,
                       note, opening_date, hide_from_selection, hide_from_reports",
        )
        .bind(name)
        .bind(note)
        .bind(opening_date)
        .bind(account_type)
        .bind(hide_from_selection)
        .bind(hide_from_reports)
        .bind(id)
        .fetch_one(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(updated)
    }
}
