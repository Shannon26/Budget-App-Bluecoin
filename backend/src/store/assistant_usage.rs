//! Member-scoped local accounting for reported OpenRouter usage.
//!
//! Costs stay as exact base-10 decimals in SQLite and are summed without binary
//! floating point. Pending/unavailable/duplicate attempts are retained only to
//! make submission replay idempotent; they are not billed-history entries.

use super::Store;
use crate::error::{AppError, AppResult};
use crate::models::{AssistantUsageDay, AssistantUsageSummary};
use chrono::Utc;
use rust_decimal::Decimal;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn submission_key(member_id: i64, submission_id: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(b"paisa-assistant-usage-v1\0");
    hash.update(member_id.to_string().as_bytes());
    hash.update(b"\0");
    hash.update(submission_id.as_bytes());
    format!("{:x}", hash.finalize())
}

fn now_string() -> String {
    Utc::now().to_rfc3339()
}

fn exact_sum(left: Decimal, right: Decimal) -> AppResult<Decimal> {
    left.checked_add(right).ok_or_else(|| {
        AppError::Conflict("assistant cost total is outside the supported range".to_string())
    })
}

#[derive(Default)]
struct DayTotal {
    total: Decimal,
    completed: i64,
    reported: i64,
    zero: i64,
    missing: i64,
}

impl Store {
    /// Atomically claim one member submission before any upstream request.
    /// A replay returns false and therefore cannot create another generation.
    pub(crate) async fn claim_assistant_attempt(
        &self,
        member_id: i64,
        submission_id: &str,
        model: &str,
        provider: &str,
    ) -> AppResult<bool> {
        let result = sqlx::query(
            "INSERT OR IGNORE INTO assistant_usage
                (member_id, submission_key, status, model, provider, created_at)
             VALUES (?, ?, 'pending', ?, ?, ?)",
        )
        .bind(member_id)
        .bind(submission_key(member_id, submission_id))
        .bind(model)
        .bind(provider)
        .bind(now_string())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub(crate) async fn complete_assistant_attempt(
        &self,
        member_id: i64,
        submission_id: &str,
        generation_id: Option<&str>,
        cost_usd: Option<Decimal>,
    ) -> AppResult<()> {
        if cost_usd.is_some_and(|cost| cost < Decimal::ZERO) {
            return Err(AppError::BadRequest(
                "reported assistant cost cannot be negative".to_string(),
            ));
        }
        let cost = cost_usd.map(|cost| cost.normalize().to_string());
        let reported = i64::from(cost.is_some());
        let key = submission_key(member_id, submission_id);
        let completed_at = now_string();
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let current: i64 = sqlx::query_scalar(
            "SELECT id FROM assistant_usage WHERE member_id = ? AND submission_key = ?",
        )
        .bind(member_id)
        .bind(&key)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::Conflict("assistant usage attempt is missing".to_string()))?;

        let duplicate = if let Some(generation_id) = generation_id {
            sqlx::query_scalar::<_, i64>(
                "SELECT id FROM assistant_usage
                 WHERE generation_id = ? AND id <> ?",
            )
            .bind(generation_id)
            .bind(current)
            .fetch_optional(&mut *tx)
            .await?
            .is_some()
        } else {
            false
        };

        if duplicate {
            // Keep the submission claim so its replay stays offline, but never
            // count the same upstream generation under two submissions.
            sqlx::query(
                "UPDATE assistant_usage
                 SET status = 'duplicate', generation_id = NULL, cost_reported = 0,
                     cost_usd = NULL, completed_at = ?
                 WHERE id = ?",
            )
            .bind(&completed_at)
            .bind(current)
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query(
                "UPDATE assistant_usage
                 SET status = 'completed', generation_id = ?, cost_reported = ?, cost_usd = ?,
                     completed_at = ?
                 WHERE id = ? AND status = 'pending'",
            )
            .bind(generation_id)
            .bind(reported)
            .bind(&cost)
            .bind(&completed_at)
            .bind(current)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub(crate) async fn mark_assistant_unavailable(
        &self,
        member_id: i64,
        submission_id: &str,
    ) -> AppResult<()> {
        sqlx::query(
            "UPDATE assistant_usage
             SET status = 'unavailable', cost_reported = 0, cost_usd = NULL, completed_at = ?
             WHERE member_id = ? AND submission_key = ? AND status = 'pending'",
        )
        .bind(now_string())
        .bind(member_id)
        .bind(submission_key(member_id, submission_id))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(crate) async fn assistant_usage_summary(
        &self,
        member_id: i64,
    ) -> AppResult<AssistantUsageSummary> {
        let rows = sqlx::query_as::<_, (String, i64, Option<String>)>(
            "SELECT created_at, cost_reported, cost_usd
             FROM assistant_usage
             WHERE member_id = ? AND status = 'completed'
             ORDER BY created_at, id",
        )
        .bind(member_id)
        .fetch_all(&self.pool)
        .await?;

        let mut total = Decimal::ZERO;
        let mut reported_count = 0_i64;
        let mut zero_cost_count = 0_i64;
        let mut missing_cost_count = 0_i64;
        let mut days = BTreeMap::<String, DayTotal>::new();
        for (created_at, reported, stored_cost) in rows {
            let cost = match (reported, stored_cost) {
                (1, Some(value)) => Some(Decimal::from_str_exact(&value).map_err(|_| {
                    AppError::Conflict("stored assistant cost is invalid".to_string())
                })?),
                (0, None) => None,
                _ => {
                    return Err(AppError::Conflict(
                        "stored assistant cost is invalid".to_string(),
                    ))
                }
            };
            let date = created_at
                .split_once('T')
                .map(|(date, _)| date.to_string())
                .unwrap_or_else(|| created_at.clone());
            let day = days.entry(date).or_default();
            day.completed += 1;
            if let Some(cost) = cost {
                total = exact_sum(total, cost)?;
                day.total = exact_sum(day.total, cost)?;
                reported_count += 1;
                day.reported += 1;
                if cost.is_zero() {
                    zero_cost_count += 1;
                    day.zero += 1;
                }
            } else {
                missing_cost_count += 1;
                day.missing += 1;
            }
        }

        let completed_count = days.values().map(|day| day.completed).sum();
        Ok(AssistantUsageSummary {
            member_id,
            total_cost_usd: total.normalize().to_string(),
            completed_count,
            reported_count,
            zero_cost_count,
            missing_cost_count,
            days: days
                .into_iter()
                .map(|(date, day)| AssistantUsageDay {
                    date,
                    total_cost_usd: day.total.normalize().to_string(),
                    completed_count: day.completed,
                    reported_count: day.reported,
                    zero_cost_count: day.zero,
                    missing_cost_count: day.missing,
                })
                .collect(),
        })
    }
}
