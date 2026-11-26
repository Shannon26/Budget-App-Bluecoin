use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryRequest {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub parent_id: Option<i64>,
    #[serde(default = "default_category_color")]
    pub color: String,
    #[serde(default = "default_category_icon")]
    pub icon: String,
    #[serde(default)]
    pub sort_order: i64,
}

fn default_category_color() -> String {
    "#426b8a".to_string()
}

fn default_category_icon() -> String {
    "•".to_string()
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub parent_id: Option<i64>,
    pub parent_name: Option<String>,
    pub color: String,
    pub icon: String,
    pub sort_order: i64,
    pub archived_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CategoryTotal {
    pub category_id: Option<i64>,
    pub kind: String,
    pub name: String,
    pub total_minor: i64,
    pub entry_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeleteCategoryResponse {
    pub archived: bool,
}
