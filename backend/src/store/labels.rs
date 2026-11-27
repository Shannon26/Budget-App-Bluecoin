use super::Store;
use crate::error::{AppError, AppResult};
use crate::models::*;
use sqlx::SqliteConnection;
use std::collections::HashMap;

impl Store {
    fn clean_label_name(name: String) -> AppResult<String> {
        let name = name.trim().to_string();
        if name.is_empty() || name.chars().count() > 40 {
            return Err(AppError::BadRequest(
                "label name must be between 1 and 40 characters".to_string(),
            ));
        }
        Ok(name)
    }

    fn normalized_label_ids(label_ids: Option<Vec<i64>>) -> Option<Vec<i64>> {
        label_ids.map(|mut ids| {
            ids.sort_unstable();
            ids.dedup();
            ids
        })
    }

    pub(super) async fn validate_label_ids(
        connection: &mut SqliteConnection,
        label_ids: Option<Vec<i64>>,
    ) -> AppResult<Option<Vec<i64>>> {
        let Some(ids) = Self::normalized_label_ids(label_ids) else {
            return Ok(None);
        };
        for id in &ids {
            let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM labels WHERE id = ?")
                .bind(id)
                .fetch_one(&mut *connection)
                .await?;
            if exists == 0 {
                return Err(AppError::BadRequest(format!("label {id} was not found")));
            }
        }
        Ok(Some(ids))
    }

    pub(super) async fn replace_entry_labels(
        connection: &mut SqliteConnection,
        entry_id: i64,
        label_ids: Option<Vec<i64>>,
    ) -> AppResult<()> {
        let Some(ids) = label_ids else {
            return Ok(());
        };
        sqlx::query("DELETE FROM entry_labels WHERE entry_id = ?")
            .bind(entry_id)
            .execute(&mut *connection)
            .await?;
        for id in ids {
            sqlx::query("INSERT INTO entry_labels (entry_id, label_id) VALUES (?, ?)")
                .bind(entry_id)
                .bind(id)
                .execute(&mut *connection)
                .await?;
        }
        Ok(())
    }

    pub(super) async fn replace_transfer_labels(
        connection: &mut SqliteConnection,
        transfer_id: i64,
        label_ids: Option<Vec<i64>>,
    ) -> AppResult<()> {
        let Some(ids) = label_ids else {
            return Ok(());
        };
        sqlx::query("DELETE FROM transfer_labels WHERE transfer_id = ?")
            .bind(transfer_id)
            .execute(&mut *connection)
            .await?;
        for id in ids {
            sqlx::query("INSERT INTO transfer_labels (transfer_id, label_id) VALUES (?, ?)")
                .bind(transfer_id)
                .bind(id)
                .execute(&mut *connection)
                .await?;
        }
        Ok(())
    }

    pub(super) async fn attach_labels(&self, entries: &mut [LedgerEntry]) -> AppResult<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let labels =
            sqlx::query_as::<_, Label>("SELECT id, name FROM labels ORDER BY name_key, id")
                .fetch_all(&self.pool)
                .await?;
        let label_by_id = labels
            .iter()
            .map(|label| (label.id, label.clone()))
            .collect::<HashMap<_, _>>();
        let entry_rows = sqlx::query_as::<_, (i64, i64)>(
            "SELECT entry_id, label_id FROM entry_labels ORDER BY entry_id, label_id",
        )
        .fetch_all(&self.pool)
        .await?;
        let transfer_rows = sqlx::query_as::<_, (i64, i64)>(
            "SELECT transfer_id, label_id FROM transfer_labels ORDER BY transfer_id, label_id",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut entry_labels = HashMap::<i64, Vec<Label>>::new();
        for (entry_id, label_id) in entry_rows {
            if let Some(label) = label_by_id.get(&label_id) {
                entry_labels
                    .entry(entry_id)
                    .or_default()
                    .push(label.clone());
            }
        }
        let mut transfer_labels = HashMap::<i64, Vec<Label>>::new();
        for (transfer_id, label_id) in transfer_rows {
            if let Some(label) = label_by_id.get(&label_id) {
                transfer_labels
                    .entry(transfer_id)
                    .or_default()
                    .push(label.clone());
            }
        }
        for entry in entries {
            let assigned = if let Some(transfer_id) = entry.transfer_id {
                transfer_labels.get(&transfer_id)
            } else {
                entry_labels.get(&entry.id)
            };
            if let Some(assigned) = assigned {
                entry.labels = assigned.clone();
                entry.labels.sort_by(|left, right| {
                    left.name
                        .to_lowercase()
                        .cmp(&right.name.to_lowercase())
                        .then_with(|| left.id.cmp(&right.id))
                });
            }
        }
        Ok(())
    }

    pub async fn labels(&self) -> AppResult<Vec<Label>> {
        Ok(
            sqlx::query_as::<_, Label>("SELECT id, name FROM labels ORDER BY name_key, id")
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn create_label(&self, request: LabelRequest) -> AppResult<Label> {
        let name = Self::clean_label_name(request.name)?;
        let name_key = name.to_lowercase();
        let mut transaction = self.pool.begin().await?;
        let inserted = sqlx::query_as::<_, Label>(
            "INSERT INTO labels (name, name_key) VALUES (?, ?)
             ON CONFLICT(name_key) DO NOTHING
             RETURNING id, name",
        )
        .bind(&name)
        .bind(&name_key)
        .fetch_optional(&mut *transaction)
        .await?;
        let label = if let Some(label) = inserted {
            label
        } else {
            sqlx::query_as::<_, Label>("SELECT id, name FROM labels WHERE name_key = ?")
                .bind(name_key)
                .fetch_one(&mut *transaction)
                .await?
        };
        transaction.commit().await?;
        Ok(label)
    }

    pub async fn update_label(&self, id: i64, request: LabelRequest) -> AppResult<Label> {
        let name = Self::clean_label_name(request.name)?;
        let name_key = name.to_lowercase();
        let mut transaction = self.pool.begin().await?;
        let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM labels WHERE id = ?")
            .bind(id)
            .fetch_one(&mut *transaction)
            .await?;
        if exists == 0 {
            return Err(AppError::NotFound(format!("label {id} was not found")));
        }
        let duplicate = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM labels WHERE name_key = ? AND id <> ?",
        )
        .bind(&name_key)
        .bind(id)
        .fetch_one(&mut *transaction)
        .await?;
        if duplicate > 0 {
            return Err(AppError::Conflict(
                "a label with this name already exists".to_string(),
            ));
        }
        let label = sqlx::query_as::<_, Label>(
            "UPDATE labels SET name = ?, name_key = ? WHERE id = ? RETURNING id, name",
        )
        .bind(name)
        .bind(name_key)
        .bind(id)
        .fetch_one(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(label)
    }
}
