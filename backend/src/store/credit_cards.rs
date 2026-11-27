use super::Store;
use crate::date::{date_only, is_valid_calendar_date, previous_month};
use crate::error::{AppError, AppResult};
use crate::models::*;
use chrono::{Datelike, NaiveDate, TimeDelta};
use sqlx::FromRow;

#[derive(Debug, FromRow)]
struct CardEntry {
    id: i64,
    transfer_id: Option<i64>,
    kind: String,
    name: String,
    amount_minor: i64,
    occurred_at: String,
    counterpart_account_name: Option<String>,
}

impl CardEntry {
    fn delta(&self) -> i128 {
        match self.kind.as_str() {
            "income" | "transfer_in" => i128::from(self.amount_minor),
            "expense" | "transfer_out" => -i128::from(self.amount_minor),
            _ => 0,
        }
    }

    fn is_non_transfer_credit(&self) -> bool {
        self.kind == "income"
    }

    fn is_card_payment(&self) -> bool {
        self.kind == "transfer_in"
    }

    fn is_card_advance(&self) -> bool {
        self.kind == "transfer_out"
    }

    fn event(&self) -> CreditCardStatementEvent {
        CreditCardStatementEvent {
            entry_id: self.id,
            transfer_id: self.transfer_id,
            entry_kind: self.kind.clone(),
            name: self.name.clone(),
            occurred_at: self.occurred_at.clone(),
            amount_minor: self.amount_minor,
            counterpart_account_name: self.counterpart_account_name.clone(),
        }
    }
}

struct CardSnapshot {
    account: Account,
    entries: Vec<CardEntry>,
}

#[derive(Debug)]
struct StatementDates {
    start: NaiveDate,
    cutoff: NaiveDate,
    end_exclusive: NaiveDate,
    due: NaiveDate,
    payment_start: NaiveDate,
    payment_end_exclusive: NaiveDate,
}

fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => unreachable!("month is validated before calendar calculations"),
    }
}

fn clamped_day(year: i32, month: u32, day: u32) -> AppResult<NaiveDate> {
    NaiveDate::from_ymd_opt(year, month, day.min(days_in_month(year, month))).ok_or_else(|| {
        AppError::BadRequest("card calendar date could not be represented".to_string())
    })
}

fn next_month(year: i32, month: u32) -> AppResult<(i32, u32)> {
    if month == 12 {
        if year == 9999 {
            return Err(AppError::BadRequest(
                "card due date must have a representable following day".to_string(),
            ));
        }
        Ok((year + 1, 1))
    } else {
        Ok((year, month + 1))
    }
}

fn statement_dates(anchor_date: &str, cutoff_day: u32, due_day: u32) -> AppResult<StatementDates> {
    let anchor = NaiveDate::parse_from_str(anchor_date, "%Y-%m-%d").map_err(|_| {
        AppError::BadRequest(
            "anchor_date must be a valid calendar date in YYYY-MM-DD format".to_string(),
        )
    })?;
    let cutoff = clamped_day(anchor.year(), anchor.month(), cutoff_day)?;
    let prior_month = previous_month(&anchor.format("%Y-%m").to_string()).ok_or_else(|| {
        AppError::BadRequest("anchor_date must allow a previous clamped cutoff".to_string())
    })?;
    let prior =
        NaiveDate::parse_from_str(&format!("{prior_month}-01"), "%Y-%m-%d").map_err(|_| {
            AppError::BadRequest("previous card month could not be represented".to_string())
        })?;
    let prior_cutoff = clamped_day(prior.year(), prior.month(), cutoff_day)?;
    let start = prior_cutoff
        .checked_add_signed(TimeDelta::days(1))
        .ok_or_else(|| {
            AppError::BadRequest("statement start could not be represented".to_string())
        })?;
    let end_exclusive = cutoff
        .checked_add_signed(TimeDelta::days(1))
        .ok_or_else(|| {
            AppError::BadRequest("statement end could not be represented".to_string())
        })?;

    // The due date is the first clamped due-day strictly after the cutoff.
    // A due-day equal to the cutoff therefore belongs to the following cycle.
    let mut due = clamped_day(anchor.year(), anchor.month(), due_day)?;
    if due <= cutoff {
        let (year, month) = next_month(anchor.year(), anchor.month())?;
        due = clamped_day(year, month, due_day)?;
    }
    let payment_end_exclusive = due.checked_add_signed(TimeDelta::days(1)).ok_or_else(|| {
        AppError::BadRequest("payment window end could not be represented".to_string())
    })?;

    Ok(StatementDates {
        start,
        cutoff,
        end_exclusive,
        due,
        payment_start: end_exclusive,
        payment_end_exclusive,
    })
}

fn validated_card_day(value: Option<i64>, field: &str) -> AppResult<Option<u32>> {
    match value {
        None => Ok(None),
        Some(value) if (1..=31).contains(&value) => Ok(Some(value as u32)),
        Some(_) => Err(AppError::Conflict(format!(
            "credit card {field} must be between 1 and 31"
        ))),
    }
}

fn sort_events(events: &mut [CreditCardStatementEvent]) {
    events.sort_by(|left, right| {
        left.occurred_at
            .cmp(&right.occurred_at)
            .then(left.entry_id.cmp(&right.entry_id))
    });
}

impl Store {
    async fn credit_card_snapshot(&self, account_id: i64) -> AppResult<CardSnapshot> {
        let mut transaction = self.pool.begin().await?;
        let account = Self::load_account(&mut *transaction, account_id).await?;
        if account.hide_from_reports {
            return Err(AppError::NotFound(format!(
                "report-visible credit card {account_id} was not found"
            )));
        }
        if account.account_type != CREDIT_CARD_ACCOUNT_TYPE {
            return Err(AppError::BadRequest(format!(
                "account {account_id} is not a credit card"
            )));
        }
        let entries = sqlx::query_as::<_, CardEntry>(
            "SELECT e.id, e.transfer_id, e.kind, e.name, e.amount_minor, e.occurred_at,
                    counterpart.name AS counterpart_account_name
             FROM entries e
             LEFT JOIN transfers t ON t.id = e.transfer_id
             LEFT JOIN accounts counterpart ON counterpart.id = CASE e.kind
                 WHEN 'transfer_out' THEN t.to_account_id
                 WHEN 'transfer_in' THEN t.from_account_id
                 ELSE NULL
             END
             WHERE e.account_id = ?",
        )
        .bind(account_id)
        .fetch_all(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(CardSnapshot { account, entries })
    }

    pub async fn credit_card_report(
        &self,
        account_id: i64,
        anchor_date: &str,
    ) -> AppResult<CreditCardReport> {
        if !is_valid_calendar_date(anchor_date) {
            return Err(AppError::BadRequest(
                "anchor_date must be a valid calendar date in YYYY-MM-DD format".to_string(),
            ));
        }
        let CardSnapshot { account, entries } = self.credit_card_snapshot(account_id).await?;
        let mut current_balance = i128::from(account.opening_balance_minor);
        for entry in &entries {
            current_balance += entry.delta();
        }
        let current_balance_minor =
            Self::checked_i64(current_balance, "credit card current balance")?;
        let current_owed_minor =
            Self::checked_i64(-current_balance, "credit card current amount owed")?;
        let current_credit_minor =
            Self::checked_i64(current_balance, "credit card current credit")?;
        let available_credit_minor = account
            .credit_limit_minor
            .map(|limit| {
                Self::checked_i64(
                    i128::from(limit) + current_balance,
                    "credit card available limit",
                )
            })
            .transpose()?;
        let cutoff_day = validated_card_day(account.cutoff_day, "cutoff day")?;
        let due_day = validated_card_day(account.due_day, "due day")?;
        let mut missing_settings = Vec::new();
        if cutoff_day.is_none() {
            missing_settings.push("cutoff_day".to_string());
        }
        if due_day.is_none() {
            missing_settings.push("due_day".to_string());
        }
        let mut report = CreditCardReport {
            account_id: account.id,
            account_name: account.name,
            anchor_date: anchor_date.to_string(),
            current_balance_minor,
            current_owed_minor: current_owed_minor.max(0),
            current_credit_minor: current_credit_minor.max(0),
            available_credit_minor,
            credit_limit_minor: account.credit_limit_minor,
            cutoff_day: account.cutoff_day,
            due_day: account.due_day,
            missing_settings,
            statement: None,
        };
        let (Some(cutoff_day), Some(due_day)) = (cutoff_day, due_day) else {
            return Ok(report);
        };
        let dates = statement_dates(anchor_date, cutoff_day, due_day)?;
        let start_date = dates.start.to_string();
        let cutoff_date = dates.cutoff.to_string();
        let end_date = dates.end_exclusive.to_string();
        let due_date = dates.due.to_string();
        let payment_start_date = dates.payment_start.to_string();
        let payment_end_date = dates.payment_end_exclusive.to_string();

        let opening_amount = i128::from(account.opening_balance_minor);
        let (opening_baseline_before_period, opening_baseline_inside_period) =
            match account.opening_date.as_deref() {
                None => (opening_amount, 0),
                Some(opening_date) if is_valid_calendar_date(opening_date) => {
                    if opening_date < start_date.as_str() {
                        (opening_amount, 0)
                    } else if opening_date < end_date.as_str() {
                        (0, opening_amount)
                    } else {
                        (0, 0)
                    }
                }
                Some(_) => {
                    return Err(AppError::Conflict(format!(
                        "account {} has an invalid opening date",
                        account.id
                    )))
                }
            };
        let opening_balance = opening_baseline_before_period + opening_baseline_inside_period;
        let legacy_opening_balance = account.opening_date.is_none();
        let mut balance_before_period = opening_baseline_before_period;
        let mut balance_at_cutoff = opening_balance;
        let mut charge_minor = 0_i128;
        let mut non_transfer_credit_minor = 0_i128;
        let mut statement_payment_minor = 0_i128;
        let mut statement_advance_minor = 0_i128;
        let mut post_cutoff_charge_minor = 0_i128;
        let mut post_cutoff_non_transfer_credit_minor = 0_i128;
        let mut post_cutoff_payment_minor = 0_i128;
        let mut post_cutoff_advance_minor = 0_i128;
        let mut charge_count = 0_i128;
        let mut non_transfer_credit_count = 0_i128;
        let mut statement_payment_count = 0_i128;
        let mut statement_advance_count = 0_i128;
        let mut post_cutoff_charge_count = 0_i128;
        let mut post_cutoff_non_transfer_credit_count = 0_i128;
        let mut post_cutoff_payment_count = 0_i128;
        let mut post_cutoff_advance_count = 0_i128;
        let mut invalid_date_entry_count = 0_i128;
        let mut statement_activities = Vec::new();
        let mut statement_payments = Vec::new();
        let mut post_cutoff_charges = Vec::new();
        let mut post_cutoff_advances = Vec::new();
        let mut post_cutoff_payments = Vec::new();
        let mut post_cutoff_non_transfer_credits = Vec::new();

        for entry in entries {
            let Some(date) = date_only(&entry.occurred_at) else {
                invalid_date_entry_count += 1;
                continue;
            };
            if date.as_str() < start_date.as_str() {
                balance_before_period += entry.delta();
            }
            if date.as_str() < end_date.as_str() {
                balance_at_cutoff += entry.delta();
            }
            if date.as_str() >= start_date.as_str() && date.as_str() < end_date.as_str() {
                match entry.kind.as_str() {
                    "expense" => {
                        charge_minor += i128::from(entry.amount_minor);
                        charge_count += 1;
                        statement_activities.push(entry.event());
                    }
                    "income" => {
                        non_transfer_credit_minor += i128::from(entry.amount_minor);
                        non_transfer_credit_count += 1;
                        statement_activities.push(entry.event());
                    }
                    "transfer_in" => {
                        statement_payment_minor += i128::from(entry.amount_minor);
                        statement_payment_count += 1;
                        statement_payments.push(entry.event());
                    }
                    "transfer_out" => {
                        statement_advance_minor += i128::from(entry.amount_minor);
                        statement_advance_count += 1;
                        statement_activities.push(entry.event());
                    }
                    _ => {}
                }
            }
            if date.as_str() >= payment_start_date.as_str()
                && date.as_str() < payment_end_date.as_str()
            {
                if entry.kind == "expense" {
                    post_cutoff_charge_minor += i128::from(entry.amount_minor);
                    post_cutoff_charge_count += 1;
                    post_cutoff_charges.push(entry.event());
                } else if entry.is_card_payment() {
                    post_cutoff_payment_minor += i128::from(entry.amount_minor);
                    post_cutoff_payment_count += 1;
                    post_cutoff_payments.push(entry.event());
                } else if entry.is_card_advance() {
                    post_cutoff_advance_minor += i128::from(entry.amount_minor);
                    post_cutoff_advance_count += 1;
                    post_cutoff_advances.push(entry.event());
                } else if entry.is_non_transfer_credit() {
                    post_cutoff_non_transfer_credit_minor += i128::from(entry.amount_minor);
                    post_cutoff_non_transfer_credit_count += 1;
                    post_cutoff_non_transfer_credits.push(entry.event());
                }
            }
        }
        sort_events(&mut statement_activities);
        sort_events(&mut statement_payments);
        sort_events(&mut post_cutoff_charges);
        sort_events(&mut post_cutoff_advances);
        sort_events(&mut post_cutoff_payments);
        sort_events(&mut post_cutoff_non_transfer_credits);

        let balance_at_cutoff_minor =
            Self::checked_i64(balance_at_cutoff, "statement balance at cutoff")?;
        let owed_at_cutoff_minor =
            Self::checked_i64(-balance_at_cutoff, "statement amount owed at cutoff")?;
        let credit_at_cutoff_minor =
            Self::checked_i64(balance_at_cutoff, "statement credit balance at cutoff")?;
        report.statement = Some(CreditCardStatement {
            start_date,
            cutoff_date,
            end_date_exclusive: end_date,
            due_date,
            payment_start_date,
            payment_end_date_exclusive: payment_end_date,
            opening_balance_minor: Self::checked_i64(opening_balance, "statement opening balance")?,
            opening_baseline_before_period_minor: Self::checked_i64(
                opening_baseline_before_period,
                "statement opening baseline before period",
            )?,
            opening_baseline_inside_period_minor: Self::checked_i64(
                opening_baseline_inside_period,
                "statement opening baseline inside period",
            )?,
            balance_before_period_minor: Self::checked_i64(
                balance_before_period,
                "statement balance before period",
            )?,
            balance_at_cutoff_minor,
            owed_at_cutoff_minor: owed_at_cutoff_minor.max(0),
            credit_at_cutoff_minor: credit_at_cutoff_minor.max(0),
            charge_minor: Self::checked_i64(charge_minor, "statement card charges")?,
            non_transfer_credit_minor: Self::checked_i64(
                non_transfer_credit_minor,
                "statement non-transfer credits",
            )?,
            statement_payment_minor: Self::checked_i64(
                statement_payment_minor,
                "statement card-directed payments",
            )?,
            statement_advance_minor: Self::checked_i64(
                statement_advance_minor,
                "statement card advances",
            )?,
            post_cutoff_charge_minor: Self::checked_i64(
                post_cutoff_charge_minor,
                "post-cutoff card charges",
            )?,
            post_cutoff_non_transfer_credit_minor: Self::checked_i64(
                post_cutoff_non_transfer_credit_minor,
                "post-cutoff non-transfer credits",
            )?,
            post_cutoff_payment_minor: Self::checked_i64(
                post_cutoff_payment_minor,
                "post-cutoff card-directed payments",
            )?,
            post_cutoff_advance_minor: Self::checked_i64(
                post_cutoff_advance_minor,
                "post-cutoff card advances",
            )?,
            charge_count: Self::checked_i64(charge_count, "statement card charge count")?,
            non_transfer_credit_count: Self::checked_i64(
                non_transfer_credit_count,
                "statement non-transfer credit count",
            )?,
            statement_payment_count: Self::checked_i64(
                statement_payment_count,
                "statement card-directed payment count",
            )?,
            statement_advance_count: Self::checked_i64(
                statement_advance_count,
                "statement card advance count",
            )?,
            post_cutoff_charge_count: Self::checked_i64(
                post_cutoff_charge_count,
                "post-cutoff card charge count",
            )?,
            post_cutoff_non_transfer_credit_count: Self::checked_i64(
                post_cutoff_non_transfer_credit_count,
                "post-cutoff non-transfer credit count",
            )?,
            post_cutoff_payment_count: Self::checked_i64(
                post_cutoff_payment_count,
                "post-cutoff card-directed payment count",
            )?,
            post_cutoff_advance_count: Self::checked_i64(
                post_cutoff_advance_count,
                "post-cutoff card advance count",
            )?,
            invalid_date_entry_count: Self::checked_i64(
                invalid_date_entry_count,
                "invalid-date card entry count",
            )?,
            legacy_opening_balance,
            statement_activities,
            statement_payments,
            post_cutoff_charges,
            post_cutoff_advances,
            post_cutoff_payments,
            post_cutoff_non_transfer_credits,
        });
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cutoff_is_inclusive_and_due_day_rolls_to_the_next_cycle() {
        let dates = statement_dates("2026-11-27", 18, 2).unwrap();
        assert_eq!(dates.start.to_string(), "2026-10-19");
        assert_eq!(dates.cutoff.to_string(), "2026-11-18");
        assert_eq!(dates.end_exclusive.to_string(), "2026-11-19");
        assert_eq!(dates.due.to_string(), "2026-12-02");
        assert_eq!(dates.payment_start.to_string(), "2026-11-19");
        assert_eq!(dates.payment_end_exclusive.to_string(), "2026-12-03");
    }

    #[test]
    fn short_months_clamp_cutoff_and_due_days_without_skipping_a_strictly_later_due_date() {
        let dates = statement_dates("2026-02-10", 18, 31).unwrap();
        assert_eq!(dates.start.to_string(), "2026-01-19");
        assert_eq!(dates.cutoff.to_string(), "2026-02-18");
        assert_eq!(dates.due.to_string(), "2026-02-28");
        assert_eq!(dates.payment_end_exclusive.to_string(), "2026-03-01");

        let equal_cutoff = statement_dates("2026-02-10", 28, 28).unwrap();
        assert_eq!(equal_cutoff.cutoff.to_string(), "2026-02-28");
        assert_eq!(equal_cutoff.due.to_string(), "2026-03-28");

        let leap = statement_dates("2024-02-10", 29, 31).unwrap();
        assert_eq!(leap.cutoff.to_string(), "2024-02-29");
        assert_eq!(leap.due.to_string(), "2024-03-31");
    }
}
