use super::Store;
use crate::date::{date_only, is_valid_calendar_date, month_bounds, previous_month};
use crate::error::{AppError, AppResult};
use crate::models::*;
use chrono::{NaiveDate, TimeDelta};
use sqlx::FromRow;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, FromRow)]
pub(super) struct ReportEntry {
    pub(super) account_id: i64,
    pub(super) category_id: Option<i64>,
    pub(super) name: String,
    pub(super) kind: String,
    pub(super) amount_minor: i64,
    pub(super) occurred_at: String,
}

#[derive(Debug)]
pub(super) struct ReportSnapshot {
    pub(super) hidden_report_accounts: HashSet<i64>,
    pub(super) categories: Vec<Category>,
    pub(super) entries: Vec<ReportEntry>,
}

pub(super) struct ReportMonths {
    pub(super) month: String,
    pub(super) prior_month: String,
    pub(super) current_start: String,
    pub(super) current_end: String,
    pub(super) prior_start: String,
    pub(super) prior_end: String,
}

#[derive(Debug, Default, Clone, Copy)]
struct CategoryAmount {
    income_minor: i128,
    expense_minor: i128,
    income_count: i128,
    expense_count: i128,
}

impl CategoryAmount {
    fn add(&mut self, kind: &str, amount_minor: i64) {
        match kind {
            "income" => {
                self.income_minor += i128::from(amount_minor);
                self.income_count += 1;
            }
            "expense" => {
                self.expense_minor += i128::from(amount_minor);
                self.expense_count += 1;
            }
            _ => {}
        }
    }

    fn merge(&mut self, other: CategoryAmount) {
        self.income_minor += other.income_minor;
        self.expense_minor += other.expense_minor;
        self.income_count += other.income_count;
        self.expense_count += other.expense_count;
    }
}

#[derive(Debug, Default)]
struct PeriodAmount {
    total: CategoryAmount,
    categories: HashMap<Option<i64>, CategoryAmount>,
}

impl PeriodAmount {
    fn add(&mut self, category_id: Option<i64>, kind: &str, amount_minor: i64) {
        self.total.add(kind, amount_minor);
        self.categories
            .entry(category_id)
            .or_default()
            .add(kind, amount_minor);
    }
}

fn checked_i64(value: i128, label: &str) -> AppResult<i64> {
    i64::try_from(value)
        .map_err(|_| AppError::Conflict(format!("{label} is outside the supported integer range")))
}

fn rounded_average(total_minor: i128, days: i128, label: &str) -> AppResult<i64> {
    let rounded = total_minor.checked_add(days / 2).ok_or_else(|| {
        AppError::Conflict(format!("{label} is outside the supported integer range"))
    })? / days;
    checked_i64(rounded, label)
}

#[derive(Debug, Default)]
struct ItemAmount {
    total_minor: i128,
    entry_count: i128,
}

fn ranked_items(amounts: HashMap<String, ItemAmount>, kind: &str) -> AppResult<Vec<ReportItem>> {
    let mut rows = amounts
        .into_iter()
        .map(|(name, amount)| (name, amount.total_minor, amount.entry_count))
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let mut previous_total = None;
    let mut previous_rank = 0;
    let mut items = Vec::with_capacity(rows.len());
    for (index, (name, total_minor, entry_count)) in rows.into_iter().enumerate() {
        let rank = if previous_total == Some(total_minor) {
            previous_rank
        } else {
            index + 1
        };
        previous_total = Some(total_minor);
        previous_rank = rank;
        items.push(ReportItem {
            rank,
            name,
            total_minor: checked_i64(total_minor, &format!("report {kind} item total"))?,
            entry_count: checked_i64(entry_count, &format!("report {kind} item count"))?,
        });
    }
    Ok(items)
}

fn category_values(
    direct: CategoryAmount,
    rollup: CategoryAmount,
) -> AppResult<ReportCategoryPeriodValues> {
    Ok(ReportCategoryPeriodValues {
        income: ReportMetricValues {
            direct_minor: checked_i64(direct.income_minor, "report category income")?,
            rollup_minor: checked_i64(rollup.income_minor, "report category income")?,
            direct_count: checked_i64(direct.income_count, "report category count")?,
            rollup_count: checked_i64(rollup.income_count, "report category count")?,
        },
        expense: ReportMetricValues {
            direct_minor: checked_i64(direct.expense_minor, "report category expense")?,
            rollup_minor: checked_i64(rollup.expense_minor, "report category expense")?,
            direct_count: checked_i64(direct.expense_count, "report category count")?,
            rollup_count: checked_i64(rollup.expense_count, "report category count")?,
        },
    })
}

fn rollup_amount(
    category_id: i64,
    direct: &HashMap<Option<i64>, CategoryAmount>,
    children: &HashMap<i64, Vec<i64>>,
) -> CategoryAmount {
    let mut result = direct.get(&Some(category_id)).copied().unwrap_or_default();
    let mut stack = children.get(&category_id).cloned().unwrap_or_default();
    let mut visited = HashSet::new();
    while let Some(child_id) = stack.pop() {
        if !visited.insert(child_id) {
            continue;
        }
        result.merge(direct.get(&Some(child_id)).copied().unwrap_or_default());
        if let Some(grandchildren) = children.get(&child_id) {
            stack.extend(grandchildren.iter().copied());
        }
    }
    result
}

fn ordered_categories(categories: &[Category]) -> Vec<Category> {
    let by_id = categories
        .iter()
        .map(|category| (category.id, category))
        .collect::<HashMap<_, _>>();
    let mut children = HashMap::<i64, Vec<i64>>::new();
    let mut roots = Vec::new();
    for category in categories {
        if let Some(parent_id) = category.parent_id {
            children.entry(parent_id).or_default().push(category.id);
        } else {
            roots.push(category.id);
        }
    }
    let compare = |left: &i64, right: &i64| {
        let left = by_id.get(left).unwrap();
        let right = by_id.get(right).unwrap();
        left.kind
            .cmp(&right.kind)
            .then_with(|| left.sort_order.cmp(&right.sort_order))
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.id.cmp(&right.id))
    };
    roots.sort_by(compare);
    for siblings in children.values_mut() {
        siblings.sort_by(compare);
    }

    let mut ordered = Vec::new();
    let mut visited = HashSet::new();
    let mut stack = roots.into_iter().rev().collect::<Vec<_>>();
    while let Some(category_id) = stack.pop() {
        if !visited.insert(category_id) {
            continue;
        }
        if let Some(category) = by_id.get(&category_id) {
            ordered.push((*category).clone());
        }
        if let Some(siblings) = children.get(&category_id) {
            stack.extend(siblings.iter().rev().copied());
        }
    }
    // Preserve legacy orphan/cycle branches instead of hiding their data.
    for category in categories {
        if visited.insert(category.id) {
            ordered.push(category.clone());
        }
    }
    ordered
}

impl Store {
    pub(super) fn report_months(month: &str) -> AppResult<ReportMonths> {
        let prior_month = previous_month(month).ok_or_else(|| {
            AppError::BadRequest(
                "month must be a valid YYYY-MM calendar month with a previous month".to_string(),
            )
        })?;
        let (current_start, current_end) = month_bounds(month).ok_or_else(|| {
            AppError::BadRequest(
                "month must be a valid calendar month in YYYY-MM format".to_string(),
            )
        })?;
        let (prior_start, prior_end) = month_bounds(&prior_month).ok_or_else(|| {
            AppError::BadRequest("previous month must be a valid calendar month".to_string())
        })?;
        if !is_valid_calendar_date(&current_end) || !is_valid_calendar_date(&prior_end) {
            return Err(AppError::BadRequest(
                "report month must have representable calendar boundaries".to_string(),
            ));
        }
        Ok(ReportMonths {
            month: month.to_string(),
            prior_month,
            current_start,
            current_end,
            prior_start,
            prior_end,
        })
    }

    pub(super) async fn report_snapshot(&self) -> AppResult<ReportSnapshot> {
        // A deferred read transaction gives accounts, categories, and entries one
        // consistent SQLite snapshot without taking a write lock.
        let mut transaction = self.pool.begin().await?;
        let hidden_report_accounts =
            sqlx::query_as::<_, (i64, bool)>("SELECT id, hide_from_reports FROM accounts")
                .fetch_all(&mut *transaction)
                .await?
                .into_iter()
                .filter_map(|(id, hidden)| hidden.then_some(id))
                .collect::<HashSet<_>>();
        let categories = sqlx::query_as::<_, Category>(
            "SELECT c.id, c.name, c.kind, c.parent_id, p.name AS parent_name,
                    c.color, c.icon, c.sort_order, c.archived_at, c.created_at
             FROM categories c
             LEFT JOIN categories p ON p.id = c.parent_id
             ORDER BY c.kind, c.sort_order, c.name, c.id",
        )
        .fetch_all(&mut *transaction)
        .await?;
        let entries = sqlx::query_as::<_, ReportEntry>(
            "SELECT account_id, category_id, name, kind, amount_minor, occurred_at
             FROM entries
             WHERE kind IN ('income', 'expense')",
        )
        .fetch_all(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(ReportSnapshot {
            hidden_report_accounts,
            categories,
            entries,
        })
    }

    pub async fn net_earnings_report(&self, month: &str) -> AppResult<NetEarningsReport> {
        let ReportMonths {
            month,
            prior_month,
            current_start,
            current_end,
            prior_start,
            prior_end,
        } = Self::report_months(month)?;
        let snapshot = self.report_snapshot().await?;
        let categories = snapshot.categories;
        let hidden_report_accounts = snapshot.hidden_report_accounts;
        let entries = snapshot.entries;

        let category_ids = categories
            .iter()
            .map(|category| category.id)
            .collect::<HashSet<_>>();
        let mut current = PeriodAmount::default();
        let mut prior = PeriodAmount::default();
        let mut invalid_date_entry_count = 0_i128;
        for entry in entries {
            if hidden_report_accounts.contains(&entry.account_id) {
                continue;
            }
            let Some(date) = date_only(&entry.occurred_at) else {
                invalid_date_entry_count += 1;
                continue;
            };
            let category_id = entry
                .category_id
                .filter(|category_id| category_ids.contains(category_id));
            if date.as_str() >= current_start.as_str() && date.as_str() < current_end.as_str() {
                current.add(category_id, &entry.kind, entry.amount_minor);
            } else if date.as_str() >= prior_start.as_str() && date.as_str() < prior_end.as_str() {
                prior.add(category_id, &entry.kind, entry.amount_minor);
            }
        }

        let current_net = current.total.income_minor - current.total.expense_minor;
        let prior_net = prior.total.income_minor - prior.total.expense_minor;
        let current_period = ReportPeriod {
            start_date: current_start,
            end_date_exclusive: current_end,
            income_minor: checked_i64(current.total.income_minor, "report income")?,
            expense_minor: checked_i64(current.total.expense_minor, "report expense")?,
            net_minor: checked_i64(current_net, "report net earnings")?,
            income_count: checked_i64(current.total.income_count, "report income count")?,
            expense_count: checked_i64(current.total.expense_count, "report expense count")?,
        };
        let prior_period = ReportPeriod {
            start_date: prior_start,
            end_date_exclusive: prior_end,
            income_minor: checked_i64(prior.total.income_minor, "report income")?,
            expense_minor: checked_i64(prior.total.expense_minor, "report expense")?,
            net_minor: checked_i64(prior_net, "report net earnings")?,
            income_count: checked_i64(prior.total.income_count, "report income count")?,
            expense_count: checked_i64(prior.total.expense_count, "report expense count")?,
        };

        let mut children = HashMap::<i64, Vec<i64>>::new();
        for category in &categories {
            if let Some(parent_id) = category.parent_id {
                children.entry(parent_id).or_default().push(category.id);
            }
        }
        let mut report_categories = Vec::new();
        for category in ordered_categories(&categories) {
            let current_direct = current
                .categories
                .get(&Some(category.id))
                .copied()
                .unwrap_or_default();
            let prior_direct = prior
                .categories
                .get(&Some(category.id))
                .copied()
                .unwrap_or_default();
            let current_rollup = rollup_amount(category.id, &current.categories, &children);
            let prior_rollup = rollup_amount(category.id, &prior.categories, &children);
            report_categories.push(ReportCategory {
                category_id: Some(category.id),
                name: category.name,
                kind: category.kind,
                parent_id: category.parent_id,
                parent_name: category.parent_name,
                color: category.color,
                icon: category.icon,
                sort_order: category.sort_order,
                archived_at: category.archived_at,
                current: category_values(current_direct, current_rollup)?,
                prior: category_values(prior_direct, prior_rollup)?,
            });
        }
        let current_uncategorized = current.categories.get(&None).copied().unwrap_or_default();
        let prior_uncategorized = prior.categories.get(&None).copied().unwrap_or_default();
        report_categories.push(ReportCategory {
            category_id: None,
            name: "Uncategorized".to_string(),
            kind: "uncategorized".to_string(),
            parent_id: None,
            parent_name: None,
            color: "#8b978f".to_string(),
            icon: "?".to_string(),
            sort_order: 0,
            archived_at: None,
            current: category_values(current_uncategorized, current_uncategorized)?,
            prior: category_values(prior_uncategorized, prior_uncategorized)?,
        });

        Ok(NetEarningsReport {
            month: month.to_string(),
            prior_month,
            current: current_period,
            prior: prior_period,
            invalid_date_entry_count: checked_i64(
                invalid_date_entry_count,
                "invalid-date entry count",
            )?,
            categories: report_categories,
        })
    }

    pub async fn items_summary_report(&self, month: &str) -> AppResult<ItemsSummaryReport> {
        let ReportMonths {
            month,
            current_start,
            current_end,
            ..
        } = Self::report_months(month)?;
        let ReportSnapshot {
            hidden_report_accounts,
            entries,
            ..
        } = self.report_snapshot().await?;
        let mut income_items = HashMap::<String, ItemAmount>::new();
        let mut expense_items = HashMap::<String, ItemAmount>::new();
        let mut current = CategoryAmount::default();
        let mut invalid_date_entry_count = 0_i128;
        for entry in entries {
            if hidden_report_accounts.contains(&entry.account_id) {
                continue;
            }
            let Some(date) = date_only(&entry.occurred_at) else {
                invalid_date_entry_count += 1;
                continue;
            };
            if date.as_str() < current_start.as_str() || date.as_str() >= current_end.as_str() {
                continue;
            }
            current.add(&entry.kind, entry.amount_minor);
            let amounts = match entry.kind.as_str() {
                "income" => &mut income_items,
                "expense" => &mut expense_items,
                _ => continue,
            };
            let amount = amounts.entry(entry.name).or_default();
            amount.total_minor += i128::from(entry.amount_minor);
            amount.entry_count += 1;
        }

        let income_items = ranked_items(income_items, "income")?;
        let expense_items = ranked_items(expense_items, "expense")?;
        Ok(ItemsSummaryReport {
            month: month.to_string(),
            current: ReportPeriod {
                start_date: current_start,
                end_date_exclusive: current_end,
                income_minor: checked_i64(current.income_minor, "report income")?,
                expense_minor: checked_i64(current.expense_minor, "report expense")?,
                net_minor: checked_i64(
                    current.income_minor - current.expense_minor,
                    "report net earnings",
                )?,
                income_count: checked_i64(current.income_count, "report income count")?,
                expense_count: checked_i64(current.expense_count, "report expense count")?,
            },
            income_items,
            expense_items,
            invalid_date_entry_count: checked_i64(
                invalid_date_entry_count,
                "invalid-date entry count",
            )?,
        })
    }

    pub async fn daily_summary_report(&self, anchor_date: &str) -> AppResult<DailySummaryReport> {
        if !is_valid_calendar_date(anchor_date) {
            return Err(AppError::BadRequest(
                "anchor_date must be a valid calendar date in YYYY-MM-DD format".to_string(),
            ));
        }
        let anchor = NaiveDate::parse_from_str(anchor_date, "%Y-%m-%d").map_err(|_| {
            AppError::BadRequest("anchor_date must be a valid calendar date".to_string())
        })?;
        let start_30 = anchor
            .checked_sub_signed(TimeDelta::days(29))
            .ok_or_else(|| {
                AppError::BadRequest("anchor_date must allow a complete 30-day window".to_string())
            })?;
        let start_7_date = anchor
            .checked_sub_signed(TimeDelta::days(6))
            .ok_or_else(|| {
                AppError::BadRequest(
                    "anchor_date must allow a complete seven-day window".to_string(),
                )
            })?;
        let earliest_date = NaiveDate::from_ymd_opt(0, 1, 1)
            .ok_or_else(|| AppError::Conflict("calendar lower bound is unavailable".to_string()))?;
        if start_30 < earliest_date {
            return Err(AppError::BadRequest(
                "anchor_date must allow a complete 30-day window".to_string(),
            ));
        }
        let start_30 = start_30.to_string();
        let start_7 = start_7_date.to_string();
        let ReportSnapshot {
            hidden_report_accounts,
            entries,
            ..
        } = self.report_snapshot().await?;
        let mut daily_expenses = BTreeMap::<String, i128>::new();
        let mut total_7 = 0_i128;
        let mut total_30 = 0_i128;
        let mut invalid_date_entry_count = 0_i128;
        for entry in entries {
            if hidden_report_accounts.contains(&entry.account_id) {
                continue;
            }
            let Some(date) = date_only(&entry.occurred_at) else {
                invalid_date_entry_count += 1;
                continue;
            };
            if entry.kind != "expense"
                || date.as_str() < start_30.as_str()
                || date.as_str() > anchor_date
            {
                continue;
            }
            let amount = i128::from(entry.amount_minor);
            *daily_expenses.entry(date.clone()).or_default() += amount;
            total_30 += amount;
            if date.as_str() >= start_7.as_str() {
                total_7 += amount;
            }
        }

        let mut days = Vec::with_capacity(7);
        let mut cursor = start_7_date;
        loop {
            let date = cursor.to_string();
            days.push(DailySpending {
                expense_minor: checked_i64(
                    daily_expenses.get(&date).copied().unwrap_or_default(),
                    "daily expense total",
                )?,
                date,
            });
            if cursor == anchor {
                break;
            }
            cursor = cursor
                .checked_add_signed(TimeDelta::days(1))
                .ok_or_else(|| {
                    AppError::Conflict("daily window could not be represented".to_string())
                })?;
        }

        Ok(DailySummaryReport {
            anchor_date: anchor_date.to_string(),
            seven_day_start_date: start_7,
            thirty_day_start_date: start_30,
            average_7_minor: rounded_average(total_7, 7, "seven-day average expense")?,
            average_30_minor: rounded_average(total_30, 30, "30-day average expense")?,
            days,
            invalid_date_entry_count: checked_i64(
                invalid_date_entry_count,
                "invalid-date entry count",
            )?,
        })
    }
}
