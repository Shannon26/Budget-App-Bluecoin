use super::Store;
use crate::error::{AppError, AppResult};
use crate::models::*;
use chrono::Utc;

impl Store {
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
}
