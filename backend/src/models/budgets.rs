use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct CategoryBudgetRequest {
    pub amount_minor: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OverallBudgetRequest {
    pub amount_minor: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CategoryBudgetQuery {
    pub month: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CategoryBudget {
    pub category_id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub parent_name: Option<String>,
    pub color: String,
    pub icon: String,
    pub sort_order: i64,
    pub archived_at: Option<String>,
    pub limit_minor: Option<i64>,
    pub direct_spent_minor: i64,
    pub spent_minor: i64,
    pub remaining_minor: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverallBudgetSummary {
    pub limit_minor: Option<i64>,
    pub spent_minor: i64,
    pub remaining_minor: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MonthlyCategoryBudgets {
    pub month: String,
    pub total_spent_minor: i64,
    pub uncategorized_spent_minor: i64,
    pub expense_count: i64,
    pub invalid_date_expense_count: i64,
    pub invalid_date_spent_minor: i64,
    pub overall_budget: OverallBudgetSummary,
    pub categories: Vec<CategoryBudget>,
}
