use super::Store;
use crate::date::{date_only, is_valid_calendar_date, month_bounds, previous_month};
use crate::error::{AppError, AppResult};
use crate::models::*;
use chrono::{NaiveDate, TimeDelta};
use sqlx::{FromRow, SqliteConnection};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, FromRow)]
pub(super) struct ReportEntry {
    pub(super) id: i64,
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

#[derive(Debug, FromRow)]
pub(super) struct BalanceSheetEntry {
    pub(super) account_id: i64,
    pub(super) kind: String,
    pub(super) amount_minor: i64,
    pub(super) occurred_at: String,
}

#[derive(Debug)]
pub(super) struct BalanceSheetSnapshot {
    pub(super) accounts: Vec<Account>,
    pub(super) entries: Vec<BalanceSheetEntry>,
}

pub(super) struct ReportMonths {
    pub(super) month: String,
    pub(super) prior_month: String,
    pub(super) current_start: String,
    pub(super) current_end: String,
    pub(super) prior_start: String,
    pub(super) prior_end: String,
}

const REPORT_LABEL_BATCH_SIZE: usize = 500;

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

fn inclusive_month_end(exclusive_date: &str) -> AppResult<String> {
    let date = NaiveDate::parse_from_str(exclusive_date, "%Y-%m-%d").map_err(|_| {
        AppError::BadRequest("report month-end could not be represented".to_string())
    })?;
    date.checked_sub_signed(TimeDelta::days(1))
        .map(|date| date.to_string())
        .ok_or_else(|| {
            AppError::BadRequest("report month-end could not be represented".to_string())
        })
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

#[derive(Debug, Default, Clone, Copy)]
struct LabelAmount {
    income_minor: i128,
    expense_minor: i128,
}

impl LabelAmount {
    fn add(&mut self, kind: &str, amount_minor: i128) {
        match kind {
            "income" => self.income_minor += amount_minor,
            "expense" => self.expense_minor += amount_minor,
            _ => {}
        }
    }
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

    async fn read_hidden_report_accounts(
        transaction: &mut SqliteConnection,
    ) -> AppResult<HashSet<i64>> {
        Ok(
            sqlx::query_as::<_, (i64, bool)>("SELECT id, hide_from_reports FROM accounts")
                .fetch_all(&mut *transaction)
                .await?
                .into_iter()
                .filter_map(|(id, hidden)| hidden.then_some(id))
                .collect::<HashSet<_>>(),
        )
    }

    async fn read_report_entries(
        transaction: &mut SqliteConnection,
    ) -> AppResult<Vec<ReportEntry>> {
        Ok(sqlx::query_as::<_, ReportEntry>(
            "SELECT id, account_id, category_id, name, kind, amount_minor, occurred_at
             FROM entries
             WHERE kind IN ('income', 'expense')",
        )
        .fetch_all(&mut *transaction)
        .await?)
    }

    async fn read_entry_label_ids(
        transaction: &mut SqliteConnection,
        entry_ids: &[i64],
    ) -> AppResult<HashMap<i64, Vec<i64>>> {
        let mut assignments = HashMap::<i64, Vec<i64>>::new();
        for batch in entry_ids.chunks(REPORT_LABEL_BATCH_SIZE) {
            // Stay below SQLite's legacy variable limit. Only Rust-selected,
            // eligible entry IDs are ever bound to this label-specific query.
            let placeholders = vec!["?"; batch.len()].join(",");
            let sql = format!(
                "SELECT el.entry_id, el.label_id
                 FROM entry_labels el
                 JOIN labels l ON l.id = el.label_id
                 WHERE el.entry_id IN ({placeholders})
                 ORDER BY el.entry_id, el.label_id"
            );
            let mut query = sqlx::query_as::<_, (i64, i64)>(&sql);
            for entry_id in batch {
                query = query.bind(*entry_id);
            }
            for (entry_id, label_id) in query.fetch_all(&mut *transaction).await? {
                assignments.entry(entry_id).or_default().push(label_id);
            }
        }
        Ok(assignments)
    }

    pub(super) async fn report_snapshot(&self) -> AppResult<ReportSnapshot> {
        // A deferred read transaction gives accounts, categories, and entries one
        // consistent SQLite snapshot without taking a write lock. Ordinary reports
        // deliberately never query label tables.
        let mut transaction = self.pool.begin().await?;
        let hidden_report_accounts = Self::read_hidden_report_accounts(&mut transaction).await?;
        let categories = sqlx::query_as::<_, Category>(
            "SELECT c.id, c.name, c.kind, c.parent_id, p.name AS parent_name,
                    c.color, c.icon, c.sort_order, c.archived_at, c.created_at
             FROM categories c
             LEFT JOIN categories p ON p.id = c.parent_id
             ORDER BY c.kind, c.sort_order, c.name, c.id",
        )
        .fetch_all(&mut *transaction)
        .await?;
        let entries = Self::read_report_entries(&mut transaction).await?;
        transaction.commit().await?;
        Ok(ReportSnapshot {
            hidden_report_accounts,
            categories,
            entries,
        })
    }

    pub(super) async fn balance_sheet_snapshot(&self) -> AppResult<BalanceSheetSnapshot> {
        let mut transaction = self.pool.begin().await?;
        let accounts = sqlx::query_as::<_, Account>(
            "SELECT id, name, currency, opening_balance_minor, created_at, account_type,
                    note, opening_date, hide_from_selection, hide_from_reports,
                    credit_limit_minor, cutoff_day, due_day
             FROM accounts",
        )
        .fetch_all(&mut *transaction)
        .await?;
        let entries = sqlx::query_as::<_, BalanceSheetEntry>(
            "SELECT account_id, kind, amount_minor, occurred_at FROM entries",
        )
        .fetch_all(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(BalanceSheetSnapshot { accounts, entries })
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

    pub async fn label_report(&self, month: &str) -> AppResult<LabelReport> {
        let ReportMonths {
            month,
            current_start,
            current_end,
            ..
        } = Self::report_months(month)?;
        let mut transaction = self.pool.begin().await?;
        let hidden_report_accounts = Self::read_hidden_report_accounts(&mut transaction).await?;
        let entries = Self::read_report_entries(&mut transaction).await?;
        let mut eligible_entries = Vec::new();
        let mut invalid_date_entry_count = 0_i128;
        for entry in entries {
            if hidden_report_accounts.contains(&entry.account_id) {
                continue;
            }
            let Some(date) = date_only(&entry.occurred_at) else {
                invalid_date_entry_count += 1;
                continue;
            };
            if date.as_str() >= current_start.as_str() && date.as_str() < current_end.as_str() {
                eligible_entries.push(entry);
            }
        }
        let labels =
            sqlx::query_as::<_, Label>("SELECT id, name FROM labels ORDER BY name_key, id")
                .fetch_all(&mut *transaction)
                .await?;
        let eligible_entry_ids = eligible_entries
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>();
        let entry_label_ids =
            Self::read_entry_label_ids(&mut transaction, &eligible_entry_ids).await?;
        transaction.commit().await?;
        let mut current = CategoryAmount::default();
        let mut amounts = HashMap::<Option<i64>, LabelAmount>::new();
        amounts.insert(None, LabelAmount::default());
        for label in &labels {
            amounts.insert(Some(label.id), LabelAmount::default());
        }
        for entry in eligible_entries {
            // The overall report counts each posted entry once. Allocation is a
            // separate additive partition of that same positive integer amount.
            current.add(&entry.kind, entry.amount_minor);
            let mut assigned = entry_label_ids.get(&entry.id).cloned().unwrap_or_default();
            assigned.sort_unstable();
            if assigned.is_empty() {
                amounts
                    .get_mut(&None)
                    .expect("No Labels allocation row")
                    .add(&entry.kind, i128::from(entry.amount_minor));
                continue;
            }

            let label_count = assigned.len() as i128;
            let amount_minor = i128::from(entry.amount_minor);
            let equal_share = amount_minor / label_count;
            let leftover = amount_minor % label_count;
            for (index, label_id) in assigned.into_iter().enumerate() {
                let share = equal_share + if (index as i128) < leftover { 1 } else { 0 };
                amounts
                    .get_mut(&Some(label_id))
                    .expect("current label allocation row")
                    .add(&entry.kind, share);
            }
        }

        let allocated_income = amounts
            .values()
            .map(|amount| amount.income_minor)
            .sum::<i128>();
        let allocated_expense = amounts
            .values()
            .map(|amount| amount.expense_minor)
            .sum::<i128>();
        if allocated_income != current.income_minor || allocated_expense != current.expense_minor {
            return Err(AppError::Conflict(
                "label report allocation could not reconcile to report totals".to_string(),
            ));
        }

        let current_period = ReportPeriod {
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
        };
        let mut rows = Vec::with_capacity(labels.len() + 1);
        for label in labels {
            let amount = amounts.remove(&Some(label.id)).unwrap_or_default();
            rows.push(LabelReportRow {
                label_id: Some(label.id),
                name: label.name,
                is_no_labels: false,
                income_minor: checked_i64(amount.income_minor, "report label income")?,
                expense_minor: checked_i64(amount.expense_minor, "report label expense")?,
                net_minor: checked_i64(
                    amount.income_minor - amount.expense_minor,
                    "report label net earnings",
                )?,
            });
        }
        let no_labels = amounts.remove(&None).unwrap_or_default();
        rows.push(LabelReportRow {
            label_id: None,
            name: "No Labels".to_string(),
            is_no_labels: true,
            income_minor: checked_i64(no_labels.income_minor, "report No Labels income")?,
            expense_minor: checked_i64(no_labels.expense_minor, "report No Labels expense")?,
            net_minor: checked_i64(
                no_labels.income_minor - no_labels.expense_minor,
                "report No Labels net earnings",
            )?,
        });

        Ok(LabelReport {
            month: month.to_string(),
            current: current_period,
            rows,
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

    pub async fn cash_flow_trend(
        &self,
        month: &str,
        months: usize,
    ) -> AppResult<CashFlowTrendReport> {
        if !(2..=24).contains(&months) {
            return Err(AppError::BadRequest(
                "months must be between 2 and 24".to_string(),
            ));
        }
        Self::report_months(month)?;
        let mut requested = Vec::with_capacity(months);
        let mut current_month = month.to_string();
        for index in 0..months {
            let (start, end) = month_bounds(&current_month).ok_or_else(|| {
                AppError::BadRequest("trend month is outside the supported calendar".to_string())
            })?;
            if !is_valid_calendar_date(&end) {
                return Err(AppError::BadRequest(
                    "trend month has no representable end".to_string(),
                ));
            }
            requested.push((current_month.clone(), start, end));
            if index + 1 < months {
                current_month = previous_month(&current_month).ok_or_else(|| {
                    AppError::BadRequest(
                        "trend range is outside the supported calendar".to_string(),
                    )
                })?;
            }
        }
        requested.reverse();
        let BalanceSheetSnapshot { accounts, entries } = self.balance_sheet_snapshot().await?;
        let visible: HashSet<i64> = accounts
            .iter()
            .filter(|account| !account.hide_from_reports)
            .map(|account| account.id)
            .collect();
        let mut valid_entries = Vec::new();
        let mut invalid_count = 0_i128;
        for entry in entries {
            if !visible.contains(&entry.account_id) {
                continue;
            }
            match date_only(&entry.occurred_at) {
                Some(date) => valid_entries.push((entry, date)),
                None => invalid_count += 1,
            }
        }
        let mut legacy_count = 0_i128;
        for account in accounts.iter().filter(|account| !account.hide_from_reports) {
            match account.opening_date.as_deref() {
                None => legacy_count += 1,
                Some(date) if !is_valid_calendar_date(date) => {
                    return Err(AppError::Conflict(format!(
                        "account {} has an invalid opening date",
                        account.id
                    )));
                }
                Some(_) => {}
            }
        }
        let mut points = Vec::with_capacity(months);
        for (month, start, end) in requested {
            let mut totals = CategoryAmount::default();
            let mut deltas = HashMap::<i64, i128>::new();
            for (entry, date) in &valid_entries {
                if date >= &start && date < &end {
                    totals.add(&entry.kind, entry.amount_minor);
                }
                if date < &end {
                    let delta = match entry.kind.as_str() {
                        "income" | "transfer_in" => i128::from(entry.amount_minor),
                        "expense" | "transfer_out" => -i128::from(entry.amount_minor),
                        _ => 0,
                    };
                    *deltas.entry(entry.account_id).or_default() += delta;
                }
            }
            let mut net_worth = 0_i128;
            let mut groups = HashMap::<&str, i128>::new();
            for account in accounts.iter().filter(|account| !account.hide_from_reports) {
                let opening = if account
                    .opening_date
                    .as_deref()
                    .map_or(true, |date| date < end.as_str())
                {
                    i128::from(account.opening_balance_minor)
                } else {
                    0
                };
                let balance = opening + deltas.get(&account.id).copied().unwrap_or_default();
                checked_i64(balance, "account balance")?;
                net_worth += balance;
                *groups.entry(account.account_group.as_str()).or_default() += balance;
            }
            for total in groups.values() {
                checked_i64(*total, "balance-sheet subtotal")?;
            }
            points.push(CashFlowTrendPoint {
                month,
                cutoff_date: inclusive_month_end(&end)?,
                cash_flow: ReportPeriod {
                    start_date: start,
                    end_date_exclusive: end,
                    income_minor: checked_i64(totals.income_minor, "report income")?,
                    expense_minor: checked_i64(totals.expense_minor, "report expense")?,
                    net_minor: checked_i64(
                        totals.income_minor - totals.expense_minor,
                        "report net earnings",
                    )?,
                    income_count: checked_i64(totals.income_count, "report income count")?,
                    expense_count: checked_i64(totals.expense_count, "report expense count")?,
                },
                net_worth_minor: checked_i64(net_worth, "net worth")?,
            });
        }
        Ok(CashFlowTrendReport {
            month: month.to_string(),
            points,
            invalid_date_entry_count: checked_i64(invalid_count, "invalid-date entry count")?,
            legacy_opening_account_count: checked_i64(
                legacy_count,
                "legacy opening account count",
            )?,
        })
    }

    pub async fn balance_sheet_report(&self, month: &str) -> AppResult<BalanceSheetReport> {
        let ReportMonths {
            month,
            prior_month,
            current_end,
            prior_end,
            ..
        } = Self::report_months(month)?;
        let prior_date = inclusive_month_end(&prior_end)?;
        let current_date = inclusive_month_end(&current_end)?;
        let BalanceSheetSnapshot { accounts, entries } = self.balance_sheet_snapshot().await?;
        let hidden_accounts = accounts
            .iter()
            .filter(|account| account.hide_from_reports)
            .map(|account| account.id)
            .collect::<HashSet<_>>();
        let mut prior_deltas = HashMap::<i64, i128>::new();
        let mut current_deltas = HashMap::<i64, i128>::new();
        let mut invalid_date_entry_count = 0_i128;
        for entry in entries {
            if hidden_accounts.contains(&entry.account_id) {
                continue;
            }
            let Some(date) = date_only(&entry.occurred_at) else {
                invalid_date_entry_count += 1;
                continue;
            };
            let delta = match entry.kind.as_str() {
                "income" | "transfer_in" => i128::from(entry.amount_minor),
                "expense" | "transfer_out" => -i128::from(entry.amount_minor),
                _ => 0,
            };
            if date.as_str() < prior_end.as_str() {
                *prior_deltas.entry(entry.account_id).or_default() += delta;
            }
            if date.as_str() < current_end.as_str() {
                *current_deltas.entry(entry.account_id).or_default() += delta;
            }
        }

        let mut grouped = BTreeMap::<String, Vec<(Account, i128, i128)>>::new();
        let mut prior_net_worth = 0_i128;
        let mut current_net_worth = 0_i128;
        let mut legacy_opening_account_count = 0_i128;
        for account in accounts {
            if account.hide_from_reports {
                continue;
            }
            let opening = i128::from(account.opening_balance_minor);
            let (mut prior, mut current) = match account.opening_date.as_deref() {
                None => {
                    legacy_opening_account_count += 1;
                    (opening, opening)
                }
                Some(opening_date) if is_valid_calendar_date(opening_date) => (
                    if opening_date < prior_end.as_str() {
                        opening
                    } else {
                        0
                    },
                    if opening_date < current_end.as_str() {
                        opening
                    } else {
                        0
                    },
                ),
                Some(_) => {
                    return Err(AppError::Conflict(format!(
                        "account {} has an invalid opening date",
                        account.id
                    )))
                }
            };
            prior += prior_deltas.get(&account.id).copied().unwrap_or_default();
            current += current_deltas.get(&account.id).copied().unwrap_or_default();
            prior_net_worth += prior;
            current_net_worth += current;
            let key = match account.account_group.as_str() {
                "asset" => "asset",
                "liability" => "liability",
                _ => "unclassified",
            };
            grouped
                .entry(key.to_string())
                .or_default()
                .push((account, prior, current));
        }

        let definitions = [
            ("asset", "Assets"),
            ("liability", "Liabilities"),
            ("unclassified", "Unclassified"),
        ];
        let mut groups = Vec::new();
        for (key, label) in definitions {
            let mut rows = grouped.remove(key).unwrap_or_default();
            rows.sort_by(|left, right| {
                let left_label = account_type_definition(&left.0.account_type)
                    .map(|definition| definition.label)
                    .unwrap_or("Unclassified");
                let right_label = account_type_definition(&right.0.account_type)
                    .map(|definition| definition.label)
                    .unwrap_or("Unclassified");
                left_label
                    .cmp(right_label)
                    .then_with(|| left.0.name.cmp(&right.0.name))
                    .then_with(|| left.0.id.cmp(&right.0.id))
            });
            let prior_subtotal = rows.iter().map(|row| row.1).sum::<i128>();
            let current_subtotal = rows.iter().map(|row| row.2).sum::<i128>();
            let accounts = rows
                .into_iter()
                .map(|(account, prior, current)| {
                    Ok(BalanceSheetAccount {
                        account_id: account.id,
                        name: account.name,
                        account_type_label: account_type_definition(&account.account_type)
                            .map(|definition| definition.label.to_string())
                            .unwrap_or_else(|| "Unclassified".to_string()),
                        account_type: account.account_type,
                        prior_minor: checked_i64(prior, "account balance")?,
                        current_minor: checked_i64(current, "account balance")?,
                    })
                })
                .collect::<AppResult<Vec<_>>>()?;
            groups.push(BalanceSheetGroup {
                key: key.to_string(),
                label: label.to_string(),
                accounts,
                prior_subtotal_minor: checked_i64(prior_subtotal, "balance-sheet subtotal")?,
                current_subtotal_minor: checked_i64(current_subtotal, "balance-sheet subtotal")?,
            });
        }

        Ok(BalanceSheetReport {
            month: month.to_string(),
            prior_month,
            prior_date,
            current_date,
            groups,
            prior_net_worth_minor: checked_i64(prior_net_worth, "net worth")?,
            current_net_worth_minor: checked_i64(current_net_worth, "net worth")?,
            invalid_date_entry_count: checked_i64(
                invalid_date_entry_count,
                "invalid-date entry count",
            )?,
            legacy_opening_account_count: checked_i64(
                legacy_opening_account_count,
                "legacy opening account count",
            )?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn entry_label_queries_are_scoped_batched_and_skip_empty_selection() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("CREATE TABLE labels (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE entry_labels (
                entry_id INTEGER NOT NULL,
                label_id INTEGER NOT NULL,
                PRIMARY KEY (entry_id, label_id)
             )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO labels (id, name) VALUES (1, 'One'), (2, 'Two')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "WITH RECURSIVE ids(id) AS (
                SELECT 1 UNION ALL SELECT id + 1 FROM ids WHERE id < 501
             )
             INSERT INTO entry_labels (entry_id, label_id) SELECT id, 1 FROM ids",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO entry_labels (entry_id, label_id) VALUES (500, 2), (9999, 1), (1, 999)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let eligible_ids = (1..=501).collect::<Vec<_>>();
        let mut transaction = pool.begin().await.unwrap();
        let assignments = Store::read_entry_label_ids(&mut transaction, &eligible_ids)
            .await
            .unwrap();
        assert_eq!(assignments.len(), 501);
        assert_eq!(assignments.get(&1), Some(&vec![1]));
        assert_eq!(assignments.get(&500), Some(&vec![1, 2]));
        assert!(!assignments.contains_key(&9999));
        for entry_id in 1..=501 {
            assert!(assignments.contains_key(&entry_id));
        }

        // An empty eligible selection must not touch the association table.
        sqlx::query("DROP TABLE entry_labels")
            .execute(&mut *transaction)
            .await
            .unwrap();
        let empty = Store::read_entry_label_ids(&mut transaction, &[])
            .await
            .unwrap();
        assert!(empty.is_empty());
    }
}
