use super::Store;
use crate::error::{AppError, AppResult};
use crate::models::*;
use chrono::Utc;
use sqlx::SqliteConnection;
use std::collections::BTreeMap;

impl Store {
    pub async fn categories(&self) -> AppResult<Vec<Category>> {
        let mut connection = self.pool.acquire().await?;
        self.categories_on(&mut connection).await
    }

    pub(super) async fn categories_on(
        &self,
        connection: &mut SqliteConnection,
    ) -> AppResult<Vec<Category>> {
        sqlx::query_as::<_, Category>(
            "SELECT c.id, c.name, c.kind, c.parent_id, p.name AS parent_name,
                    c.color, c.icon, c.sort_order, c.archived_at, c.created_at
             FROM categories c
             LEFT JOIN categories p ON p.id = c.parent_id
             ORDER BY c.kind, c.sort_order, c.name",
        )
        .fetch_all(&mut *connection)
        .await
        .map_err(AppError::from)
    }

    pub async fn category_totals(&self) -> AppResult<Vec<CategoryTotal>> {
        let rows = sqlx::query_as::<_, (Option<i64>, String, String, i64)>(
            "SELECT e.category_id, e.kind, COALESCE(c.name, 'Uncategorized') AS name,
                    e.amount_minor
             FROM entries e
             LEFT JOIN categories c ON c.id = e.category_id
             WHERE e.kind IN ('income', 'expense')
             ORDER BY e.kind, name, e.id",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut totals = BTreeMap::<(String, String, Option<i64>), (i128, i64)>::new();
        for (category_id, kind, name, amount_minor) in rows {
            let total = totals.entry((kind, name, category_id)).or_default();
            total.0 += i128::from(amount_minor);
            total.1 += 1;
        }
        totals
            .into_iter()
            .map(|((kind, name, category_id), (total_minor, entry_count))| {
                Ok(CategoryTotal {
                    category_id,
                    kind,
                    name,
                    total_minor: Store::checked_i64(total_minor, "category total")?,
                    entry_count,
                })
            })
            .collect()
    }

    async fn validate_category_parent(
        &self,
        executor: &mut SqliteConnection,
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
        .fetch_optional(&mut *executor)
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
                .fetch_optional(&mut *executor)
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
        let mut connection = self.pool.acquire().await?;
        self.validate_category_parent(&mut connection, &request.kind, request.parent_id, None)
            .await?;
        let duplicate = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM categories
             WHERE kind = ? AND parent_id IS ? AND lower(name) = lower(?) AND archived_at IS NULL",
        )
        .bind(&request.kind)
        .bind(request.parent_id)
        .bind(&name)
        .fetch_one(&mut *connection)
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
        .fetch_one(&mut *connection)
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
        // Category moves and budget saves share the immediate-write lock so a move
        // cannot validate against a parent limit that is concurrently being lowered.
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let current = sqlx::query_as::<_, Category>(
            "SELECT c.id, c.name, c.kind, c.parent_id, p.name AS parent_name,
                    c.color, c.icon, c.sort_order, c.archived_at, c.created_at
             FROM categories c
             LEFT JOIN categories p ON p.id = c.parent_id
             WHERE c.id = ?",
        )
        .bind(id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("category {id} was not found")))?;
        if current.archived_at.is_some() {
            return Err(AppError::BadRequest(
                "archived categories cannot be edited".to_string(),
            ));
        }
        let children =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM categories WHERE parent_id = ?")
                .bind(id)
                .fetch_one(&mut *transaction)
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
            .fetch_one(&mut *transaction)
            .await?;
            let budgets = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM category_budgets WHERE category_id = ?",
            )
            .bind(id)
            .fetch_one(&mut *transaction)
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
        self.validate_category_parent(
            &mut *transaction,
            &request.kind,
            request.parent_id,
            Some(id),
        )
        .await?;
        if current.parent_id != request.parent_id {
            Self::validate_category_move_budgets(&mut *transaction, id, request.parent_id).await?;
        }
        let duplicate = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM categories
             WHERE id <> ? AND kind = ? AND parent_id IS ? AND lower(name) = lower(?) AND archived_at IS NULL",
        )
        .bind(id)
        .bind(&request.kind)
        .bind(request.parent_id)
        .bind(&name)
        .fetch_one(&mut *transaction)
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
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
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
}
