use super::Store;
use crate::error::{AppError, AppResult};
use sqlx::SqliteConnection;

impl Store {
    pub(super) async fn validate_budget_hierarchy(
        transaction: &mut SqliteConnection,
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
            .fetch_optional(&mut *transaction)
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
        .fetch_optional(&mut *transaction)
        .await?;
        if child_over_limit.is_some() {
            return Err(AppError::BadRequest(
                "parent category budget cannot be lower than a subcategory budget for the same month"
                    .to_string(),
            ));
        }
        Ok(())
    }

    pub(super) async fn validate_category_move_budgets(
        transaction: &mut SqliteConnection,
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
        .fetch_optional(&mut *transaction)
        .await?;
        if violation.is_some() {
            return Err(AppError::BadRequest(
                "subcategory budget cannot exceed its new parent category budget for the same month"
                    .to_string(),
            ));
        }
        Ok(())
    }
}
