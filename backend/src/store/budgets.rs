use super::Store;
use crate::date::{date_only, is_valid_month, month_bounds};
use crate::error::{AppError, AppResult};
use crate::models::*;
use chrono::Utc;
use sqlx::{Executor, Sqlite, SqliteConnection};
use std::collections::{HashMap, HashSet};

impl Store {
    fn validate_budget_month(month: &str) -> AppResult<()> {
        if !is_valid_month(month) {
            return Err(AppError::BadRequest(
                "month must be a valid calendar month in YYYY-MM format".to_string(),
            ));
        }
        Ok(())
    }

    async fn budget_category<'e, E>(
        &self,
        executor: E,
        category_id: i64,
    ) -> AppResult<(String, Option<String>, Option<i64>)>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query_as::<_, (String, Option<String>, Option<i64>)>(
            "SELECT kind, archived_at, parent_id FROM categories WHERE id = ?",
        )
        .bind(category_id)
        .fetch_optional(executor)
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
        let mut connection = self.pool.acquire().await?;
        self.monthly_category_budgets_on(&mut connection, month)
            .await
    }

    async fn monthly_category_budgets_on(
        &self,
        connection: &mut SqliteConnection,
        month: &str,
    ) -> AppResult<MonthlyCategoryBudgets> {
        Self::validate_budget_month(month)?;
        let (first_day, next_month) = month_bounds(month)
            .ok_or_else(|| AppError::BadRequest("month must be in YYYY-MM format".to_string()))?;
        let expenses = sqlx::query_as::<_, (Option<i64>, i64, String)>(
            "SELECT category_id, amount_minor, occurred_at
             FROM entries
             WHERE kind = 'expense'",
        )
        .fetch_all(&mut *connection)
        .await?;

        let mut direct_spend = HashMap::<i64, i128>::new();
        let mut total_spent_minor = 0_i128;
        let mut uncategorized_spent_minor = 0_i128;
        let mut expense_count = 0_i64;
        let mut invalid_date_expense_count = 0_i64;
        let mut invalid_date_spent_minor = 0_i128;
        for (category_id, amount_minor, occurred_at) in expenses {
            let Some(date) = date_only(&occurred_at) else {
                invalid_date_expense_count += 1;
                invalid_date_spent_minor += i128::from(amount_minor);
                continue;
            };
            if date.as_str() < first_day.as_str() || date.as_str() >= next_month.as_str() {
                continue;
            }
            total_spent_minor += i128::from(amount_minor);
            expense_count += 1;
            if let Some(category_id) = category_id {
                *direct_spend.entry(category_id).or_default() += i128::from(amount_minor);
            } else {
                uncategorized_spent_minor += i128::from(amount_minor);
            }
        }

        let all_categories = self.categories_on(connection).await?;
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
        .fetch_all(&mut *connection)
        .await?
        .into_iter()
        .collect::<HashMap<_, _>>();
        let overall_limit = sqlx::query_scalar::<_, i64>(
            "SELECT amount_minor FROM overall_budgets WHERE month = ?",
        )
        .bind(month)
        .fetch_optional(&mut *connection)
        .await?;

        let categories = ordered_ids
            .into_iter()
            .filter_map(|category_id| category_by_id.get(&category_id).cloned())
            .map(|category| {
                let direct = Store::checked_i64(
                    direct_spend.get(&category.id).copied().unwrap_or_default(),
                    "category direct expense total",
                )?;
                let spent = Store::checked_i64(
                    Self::rolled_category_spend(category.id, &direct_spend, &children),
                    "category expense total",
                )?;
                let limit = limits.get(&category.id).copied();
                Ok(CategoryBudget {
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
                })
            })
            .collect::<AppResult<Vec<_>>>()?;

        let total_spent_minor = Store::checked_i64(total_spent_minor, "monthly expense total")?;
        let overall_remaining_minor = overall_limit
            .map(|limit| {
                Store::checked_i64(
                    i128::from(limit) - i128::from(total_spent_minor),
                    "overall budget remaining",
                )
            })
            .transpose()?;
        Ok(MonthlyCategoryBudgets {
            month: month.to_string(),
            total_spent_minor,
            uncategorized_spent_minor: Store::checked_i64(
                uncategorized_spent_minor,
                "uncategorized monthly expense total",
            )?,
            expense_count,
            invalid_date_expense_count,
            invalid_date_spent_minor: Store::checked_i64(
                invalid_date_spent_minor,
                "invalid-date expense total",
            )?,
            overall_budget: OverallBudgetSummary {
                limit_minor: overall_limit,
                spent_minor: total_spent_minor,
                remaining_minor: overall_remaining_minor,
            },
            categories,
        })
    }

    fn rolled_category_spend(
        category_id: i64,
        direct_spend: &HashMap<i64, i128>,
        children: &HashMap<i64, Vec<i64>>,
    ) -> i128 {
        let mut stack = vec![category_id];
        let mut visited = HashSet::new();
        let mut total = 0_i128;
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

    pub async fn set_overall_budget(
        &self,
        month: &str,
        amount_minor: i64,
    ) -> AppResult<MonthlyCategoryBudgets> {
        Self::validate_budget_month(month)?;
        Self::validate_amount(amount_minor)?;
        // Keep the upsert and checked summary in one immediate write transaction,
        // matching category-budget mutation and rollback semantics.
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO overall_budgets (month, amount_minor, created_at, updated_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(month) DO UPDATE SET
                 amount_minor = excluded.amount_minor,
                 updated_at = excluded.updated_at",
        )
        .bind(month)
        .bind(amount_minor)
        .bind(&now)
        .bind(&now)
        .execute(&mut *transaction)
        .await?;
        let snapshot = self
            .monthly_category_budgets_on(&mut transaction, month)
            .await?;
        transaction.commit().await?;
        Ok(snapshot)
    }

    pub async fn remove_overall_budget(&self, month: &str) -> AppResult<MonthlyCategoryBudgets> {
        Self::validate_budget_month(month)?;
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::query("DELETE FROM overall_budgets WHERE month = ?")
            .bind(month)
            .execute(&mut *transaction)
            .await?;
        let snapshot = self
            .monthly_category_budgets_on(&mut transaction, month)
            .await?;
        transaction.commit().await?;
        Ok(snapshot)
    }

    pub async fn set_category_budget(
        &self,
        category_id: i64,
        month: &str,
        amount_minor: i64,
    ) -> AppResult<MonthlyCategoryBudgets> {
        Self::validate_budget_month(month)?;
        Self::validate_amount(amount_minor)?;
        // Serialize budget writers before reading so hierarchy checks, the upsert,
        // and its checked response snapshot observe the same transaction state.
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let category = self.budget_category(&mut *transaction, category_id).await?;
        let existing_limit = sqlx::query_scalar::<_, i64>(
            "SELECT amount_minor FROM category_budgets WHERE category_id = ? AND month = ?",
        )
        .bind(category_id)
        .bind(month)
        .fetch_optional(&mut *transaction)
        .await?;
        Self::validate_budget_category(category_id, &category, existing_limit)?;
        Self::validate_budget_hierarchy(
            &mut *transaction,
            category_id,
            category.2,
            month,
            amount_minor,
        )
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
        .execute(&mut *transaction)
        .await?;
        let snapshot = self
            .monthly_category_budgets_on(&mut transaction, month)
            .await?;
        transaction.commit().await?;
        Ok(snapshot)
    }

    pub async fn remove_category_budget(
        &self,
        category_id: i64,
        month: &str,
    ) -> AppResult<MonthlyCategoryBudgets> {
        Self::validate_budget_month(month)?;
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let category = self.budget_category(&mut *transaction, category_id).await?;
        Self::validate_budget_category(category_id, &category, Some(0))?;
        sqlx::query("DELETE FROM category_budgets WHERE category_id = ? AND month = ?")
            .bind(category_id)
            .bind(month)
            .execute(&mut *transaction)
            .await?;
        let snapshot = self
            .monthly_category_budgets_on(&mut transaction, month)
            .await?;
        transaction.commit().await?;
        Ok(snapshot)
    }
}
