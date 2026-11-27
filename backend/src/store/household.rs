use chrono::Utc;

use crate::error::{AppError, AppResult};
use crate::models::*;

use super::Store;

impl Store {
    pub async fn members(&self) -> AppResult<Vec<Member>> {
        let rows = sqlx::query_as::<_, (i64, String, i64)>(
            "SELECT id, name, is_designated FROM members ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(id, name, designated)| Member {
                id,
                name,
                is_designated: designated == 1,
            })
            .collect())
    }

    pub async fn member(&self, member_id: i64) -> AppResult<Member> {
        sqlx::query_as::<_, (i64, String, i64)>(
            "SELECT id, name, is_designated FROM members WHERE id = ?",
        )
        .bind(member_id)
        .fetch_optional(&self.pool)
        .await?
        .map(|(id, name, designated)| Member {
            id,
            name,
            is_designated: designated == 1,
        })
        .ok_or_else(|| AppError::NotFound(format!("member {member_id} was not found")))
    }

    /// All members see the household's saved accounts and activity. The
    /// explicit other-member view is read-only; writes stay owner-scoped.
    pub async fn accounts_view(
        &self,
        viewer: i64,
        target: i64,
        enforce: bool,
    ) -> AppResult<HouseholdAccountView> {
        if enforce && !self.member_exists(viewer).await? {
            return Err(AppError::Unauthorized(
                "viewer is not a household member".to_string(),
            ));
        }
        let owner = self.member(target).await?;
        let accounts = sqlx::query_as::<_, (i64, String, String, String, i64, Option<i64>)>(
            "SELECT id, name, currency, account_type, opening_balance_minor, owner_member_id
             FROM accounts ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;
        let deltas = sqlx::query_as::<_, (i64, String, i64)>(
            "SELECT account_id, kind, amount_minor FROM entries",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut balances = std::collections::HashMap::new();
        for (id, _, _, _, opening, _) in &accounts {
            balances.insert(*id, i128::from(*opening));
        }
        for (account_id, kind, amount_minor) in deltas {
            let delta = match kind.as_str() {
                "income" | "transfer_in" => i128::from(amount_minor),
                "expense" | "transfer_out" => -i128::from(amount_minor),
                _ => 0,
            };
            if let Some(balance) = balances.get_mut(&account_id) {
                *balance += delta;
            }
        }

        let mut view = Vec::new();
        for (id, name, currency, account_type, opening, owner_member_id) in accounts {
            if (enforce || target != viewer) && owner_member_id != Some(target) {
                continue;
            }
            let balance = Self::checked_i64(balances.get(&id).copied().unwrap_or(0), "balance")?;
            let can_write = if target != viewer {
                false
            } else if enforce {
                owner_member_id == Some(viewer)
            } else {
                owner_member_id.is_none() || owner_member_id == Some(viewer)
            };
            view.push(HouseholdAccount {
                id,
                name,
                account_type,
                currency,
                opening_balance_minor: opening,
                balance_minor: balance,
                owner_member_id,
                can_write,
            });
        }

        Ok(HouseholdAccountView {
            read_only: viewer != target,
            owner,
            accounts: view,
        })
    }

    pub async fn assignments(&self, viewer: i64, enforce: bool) -> AppResult<AssignmentView> {
        let can_assign = self.is_designated_member(viewer).await?;
        if enforce && !self.member_exists(viewer).await? {
            return Err(AppError::Unauthorized(
                "viewer is not a household member".to_string(),
            ));
        }
        if !can_assign {
            return Ok(AssignmentView {
                can_assign: false,
                accounts: Vec::new(),
            });
        }
        let rows = sqlx::query_as::<_, (i64, String, Option<i64>, Option<i64>, Option<i64>)>(
            "SELECT a.id, a.name, a.owner_member_id, o.proposed_owner_member_id,
                    o.confirmed_owner_member_id
             FROM accounts a LEFT JOIN owner_assignments o ON o.account_id = a.id
             ORDER BY a.id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(AssignmentView {
            can_assign: true,
            accounts: rows
                .into_iter()
                .map(
                    |(
                        account_id,
                        account_name,
                        current_owner_member_id,
                        proposed_owner_member_id,
                        confirmed_owner_member_id,
                    )| AssignmentRow {
                        account_id,
                        account_name,
                        current_owner_member_id,
                        proposed_owner_member_id,
                        confirmed_owner_member_id,
                    },
                )
                .collect(),
        })
    }

    pub async fn propose_assignments(
        &self,
        viewer: i64,
        proposals: Vec<AssignmentProposal>,
    ) -> AppResult<AssignmentView> {
        if !self.is_designated_member(viewer).await? {
            return Err(AppError::Conflict(
                "only the designated member can assign account ownership".to_string(),
            ));
        }
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        for proposal in &proposals {
            if !self
                .member_exists_in(&mut *transaction, proposal.owner_member_id)
                .await?
            {
                return Err(AppError::BadRequest(format!(
                    "member {} was not found",
                    proposal.owner_member_id
                )));
            }
            sqlx::query(
                "INSERT INTO owner_assignments
                    (account_id, proposed_owner_member_id, confirmed_owner_member_id, updated_at)
                 VALUES (?, ?, NULL, ?)
                 ON CONFLICT(account_id) DO UPDATE SET
                    proposed_owner_member_id = excluded.proposed_owner_member_id,
                    updated_at = excluded.updated_at",
            )
            .bind(proposal.account_id)
            .bind(proposal.owner_member_id)
            .bind(Utc::now().to_rfc3339())
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        self.assignments(viewer, false).await
    }

    pub async fn confirm_assignments(
        &self,
        viewer: i64,
        assignments: Vec<AssignmentProposal>,
    ) -> AppResult<()> {
        if !self.is_designated_member(viewer).await? {
            return Err(AppError::Conflict(
                "only the designated member can confirm account ownership".to_string(),
            ));
        }
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        for assignment in &assignments {
            let owner: Option<Option<i64>> =
                sqlx::query_scalar("SELECT owner_member_id FROM accounts WHERE id = ?")
                    .bind(assignment.account_id)
                    .fetch_optional(&mut *transaction)
                    .await?;
            let owner = owner.ok_or_else(|| {
                AppError::NotFound(format!("account {} was not found", assignment.account_id))
            })?;
            if owner.is_some() {
                return Err(AppError::Conflict(format!(
                    "account {} already has a confirmed owner",
                    assignment.account_id
                )));
            }
            if !self
                .member_exists_in(&mut *transaction, assignment.owner_member_id)
                .await?
            {
                return Err(AppError::BadRequest(format!(
                    "member {} was not found",
                    assignment.owner_member_id
                )));
            }
            sqlx::query("UPDATE accounts SET owner_member_id = ? WHERE id = ?")
                .bind(assignment.owner_member_id)
                .bind(assignment.account_id)
                .execute(&mut *transaction)
                .await?;
            sqlx::query(
                "INSERT INTO owner_assignments
                    (account_id, proposed_owner_member_id, confirmed_owner_member_id, updated_at)
                 VALUES (?, ?, ?, ?)
                 ON CONFLICT(account_id) DO UPDATE SET
                    proposed_owner_member_id = excluded.proposed_owner_member_id,
                    confirmed_owner_member_id = excluded.confirmed_owner_member_id,
                    updated_at = excluded.updated_at",
            )
            .bind(assignment.account_id)
            .bind(assignment.owner_member_id)
            .bind(assignment.owner_member_id)
            .bind(Utc::now().to_rfc3339())
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    async fn member_exists_in<'e, E>(&self, executor: E, member_id: i64) -> AppResult<bool>
    where
        E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
    {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM members WHERE id = ?")
            .bind(member_id)
            .fetch_one(executor)
            .await
            .map(|count| count > 0)
            .map_err(AppError::from)
    }
}
