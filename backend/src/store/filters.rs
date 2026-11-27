use super::Store;
use crate::date::is_valid_calendar_date;
use crate::error::{AppError, AppResult};
use crate::models::*;
use chrono::Utc;

impl Store {
    pub(super) fn validate_filter(filter: &FilterDefinition) -> AppResult<()> {
        if filter
            .date_from
            .as_ref()
            .is_some_and(|date| !is_valid_calendar_date(date))
        {
            return Err(AppError::BadRequest(
                "date_from must be a valid calendar date in YYYY-MM-DD format".to_string(),
            ));
        }
        if filter
            .date_to
            .as_ref()
            .is_some_and(|date| !is_valid_calendar_date(date))
        {
            return Err(AppError::BadRequest(
                "date_to must be a valid calendar date in YYYY-MM-DD format".to_string(),
            ));
        }
        if let (Some(from), Some(to)) = (&filter.date_from, &filter.date_to) {
            if from > to {
                return Err(AppError::BadRequest(
                    "date_from cannot be later than date_to".to_string(),
                ));
            }
        }
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
}
