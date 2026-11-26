use super::AppResult;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedFilterRequest {
    pub name: String,
    pub definition: FilterDefinition,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct FilterDefinition {
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub date_from: Option<String>,
    #[serde(default)]
    pub date_to: Option<String>,
    #[serde(default)]
    pub account_id: Option<i64>,
    #[serde(default)]
    pub category_id: Option<i64>,
    #[serde(default)]
    pub uncategorized: Option<bool>,
    #[serde(default)]
    pub reconciled: Option<bool>,
    #[serde(default)]
    pub amount_min_minor: Option<i64>,
    #[serde(default)]
    pub amount_max_minor: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SavedFilter {
    pub id: i64,
    pub name: String,
    pub definition: FilterDefinition,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub(crate) struct SavedFilterRow {
    id: i64,
    name: String,
    definition_json: String,
    created_at: String,
    updated_at: String,
}

impl SavedFilterRow {
    pub(crate) fn into_saved_filter(self) -> AppResult<(i64, SavedFilter)> {
        let definition = serde_json::from_str(&self.definition_json)
            .map_err(|error| crate::error::AppError::BadRequest(error.to_string()))?;
        Ok((
            self.id,
            SavedFilter {
                id: self.id,
                name: self.name,
                definition,
                created_at: self.created_at,
                updated_at: self.updated_at,
            },
        ))
    }
}
