use super::Store;
use crate::date::is_valid_calendar_date;
use crate::error::{AppError, AppResult};
use crate::models::*;
use chrono::Utc;
use sqlx::{Executor, Sqlite};

struct PreparedAccount {
    name: String,
    currency: String,
    account_type: String,
    note: String,
    opening_date: Option<String>,
    hide_from_selection: bool,
    hide_from_reports: bool,
    credit_limit_minor: Option<i64>,
    cutoff_day: Option<i64>,
    due_day: Option<i64>,
    opening_balance_minor: i64,
}

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

    fn validate_credit_limit(value: Option<i64>) -> AppResult<Option<i64>> {
        if value.is_some_and(|value| value <= 0) {
            return Err(AppError::BadRequest(
                "credit limit must be greater than zero".to_string(),
            ));
        }
        Ok(value)
    }

    fn validate_card_day(value: Option<i64>, field: &str) -> AppResult<Option<i64>> {
        if value.is_some_and(|value| !(1..=31).contains(&value)) {
            return Err(AppError::BadRequest(format!(
                "{field} must be between 1 and 31"
            )));
        }
        Ok(value)
    }

    fn validate_card_metadata(
        account_type: &str,
        credit_limit_minor: Option<i64>,
        cutoff_day: Option<i64>,
        due_day: Option<i64>,
    ) -> AppResult<(Option<i64>, Option<i64>, Option<i64>)> {
        if account_type != CREDIT_CARD_ACCOUNT_TYPE {
            return Err(AppError::BadRequest(
                "credit card fields are only valid for credit card accounts".to_string(),
            ));
        }
        Ok((
            Self::validate_credit_limit(credit_limit_minor)?,
            Self::validate_card_day(cutoff_day, "cutoff day")?,
            Self::validate_card_day(due_day, "due day")?,
        ))
    }

    fn prepare_account_insert(request: CreateAccountRequest) -> AppResult<PreparedAccount> {
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
        let has_card_metadata = request.credit_limit_minor.is_some()
            || request.cutoff_day.is_some()
            || request.due_day.is_some();
        let (credit_limit_minor, cutoff_day, due_day) = if account_type == CREDIT_CARD_ACCOUNT_TYPE
        {
            Self::validate_card_metadata(
                &account_type,
                request.credit_limit_minor,
                request.cutoff_day,
                request.due_day,
            )?
        } else {
            if has_card_metadata {
                return Err(AppError::BadRequest(
                    "credit card fields are only valid for credit card accounts".to_string(),
                ));
            }
            (None, None, None)
        };
        Ok(PreparedAccount {
            name,
            currency,
            account_type,
            note,
            opening_date,
            hide_from_selection: request.hide_from_selection,
            hide_from_reports: request.hide_from_reports,
            credit_limit_minor,
            cutoff_day,
            due_day,
            opening_balance_minor: request.opening_balance_minor,
        })
    }

    /// Executor-bound insert shared by the standalone account route and the
    /// Command Center batch save. `owner_member_id` is set for new accounts
    /// created by a household member; legacy callers pass `None`.
    pub(super) async fn insert_account<'e, E>(
        executor: E,
        request: CreateAccountRequest,
        owner_member_id: Option<i64>,
    ) -> AppResult<Account>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let prepared = Self::prepare_account_insert(request)?;
        sqlx::query_as::<_, Account>(
            "INSERT INTO accounts
                (name, currency, opening_balance_minor, created_at, account_type, note,
                 opening_date, hide_from_selection, hide_from_reports,
                 credit_limit_minor, cutoff_day, due_day, owner_member_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             RETURNING id, name, currency, opening_balance_minor, created_at, account_type,
                       note, opening_date, hide_from_selection, hide_from_reports,
                       credit_limit_minor, cutoff_day, due_day",
        )
        .bind(prepared.name)
        .bind(prepared.currency)
        .bind(prepared.opening_balance_minor)
        .bind(Utc::now().to_rfc3339())
        .bind(prepared.account_type)
        .bind(prepared.note)
        .bind(prepared.opening_date)
        .bind(prepared.hide_from_selection)
        .bind(prepared.hide_from_reports)
        .bind(prepared.credit_limit_minor)
        .bind(prepared.cutoff_day)
        .bind(prepared.due_day)
        .bind(owner_member_id)
        .fetch_one(executor)
        .await
        .map_err(AppError::from)
    }

    #[allow(dead_code)]
    pub async fn create_account(&self, request: CreateAccountRequest) -> AppResult<Account> {
        Self::insert_account(&self.pool, request, None).await
    }

    pub async fn create_account_owned(
        &self,
        request: CreateAccountRequest,
        owner_member_id: i64,
    ) -> AppResult<Account> {
        Self::insert_account(&self.pool, request, Some(owner_member_id)).await
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

        let has_card_updates = request.credit_limit_minor.is_some()
            || request.cutoff_day.is_some()
            || request.due_day.is_some();
        if account_type != CREDIT_CARD_ACCOUNT_TYPE && has_card_updates {
            return Err(AppError::BadRequest(
                "credit card fields are only valid for credit card accounts".to_string(),
            ));
        }
        let credit_limit_minor = request
            .credit_limit_minor
            .unwrap_or(current.credit_limit_minor);
        let cutoff_day = request.cutoff_day.unwrap_or(current.cutoff_day);
        let due_day = request.due_day.unwrap_or(current.due_day);
        let (credit_limit_minor, cutoff_day, due_day) = if account_type == CREDIT_CARD_ACCOUNT_TYPE
        {
            Self::validate_card_metadata(&account_type, credit_limit_minor, cutoff_day, due_day)?
        } else {
            // Retain card settings while the account is not a card so a later retype
            // can recover them. Serialization hides them on non-card accounts.
            (credit_limit_minor, cutoff_day, due_day)
        };

        let updated = sqlx::query_as::<_, Account>(
            "UPDATE accounts
             SET name = ?, note = ?, opening_date = ?, account_type = ?,
                 hide_from_selection = ?, hide_from_reports = ?,
                 credit_limit_minor = ?, cutoff_day = ?, due_day = ?
             WHERE id = ?
             RETURNING id, name, currency, opening_balance_minor, created_at, account_type,
                       note, opening_date, hide_from_selection, hide_from_reports,
                       credit_limit_minor, cutoff_day, due_day",
        )
        .bind(name)
        .bind(note)
        .bind(opening_date)
        .bind(account_type)
        .bind(hide_from_selection)
        .bind(hide_from_reports)
        .bind(credit_limit_minor)
        .bind(cutoff_day)
        .bind(due_day)
        .bind(id)
        .fetch_one(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(updated)
    }
}
