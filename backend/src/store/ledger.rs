use super::Store;
use crate::date::date_only;
use crate::error::{AppError, AppResult};
use crate::models::*;
use sqlx::{QueryBuilder, Sqlite};
use std::collections::BTreeMap;

impl Store {
    fn apply_search_filter(
        builder: &mut QueryBuilder<Sqlite>,
        name_column: &str,
        note_column: &str,
        filter: &FilterDefinition,
    ) {
        if let Some(search) = filter
            .search
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            // Fold both operands with SQLite so non-ASCII text keeps exact matching
            // while ASCII case-insensitivity and literal wildcard characters remain.
            let search = search.to_string();
            builder.push(" AND (instr(lower(");
            builder.push(name_column);
            builder.push("), lower(");
            builder.push_bind(search.clone());
            builder.push(")) > 0 OR instr(lower(");
            builder.push(note_column);
            builder.push("), lower(");
            builder.push_bind(search);
            builder.push(")) > 0)");
        }
    }

    fn has_date_filter(filter: &FilterDefinition) -> bool {
        filter.date_from.is_some() || filter.date_to.is_some()
    }

    fn retain_date_matches(entries: &mut Vec<LedgerEntry>, filter: &FilterDefinition) {
        if !Self::has_date_filter(filter) {
            return;
        }
        entries.retain(|entry| {
            date_only(&entry.occurred_at).is_some_and(|date| {
                filter
                    .date_from
                    .as_ref()
                    .map_or(true, |from| date.as_str() >= from.as_str())
                    && filter
                        .date_to
                        .as_ref()
                        .map_or(true, |to| date.as_str() <= to.as_str())
            })
        });
    }

    fn apply_entry_filters(builder: &mut QueryBuilder<Sqlite>, filter: &FilterDefinition) {
        Self::apply_search_filter(builder, "e.name", "e.note", filter);
        if let Some(account_id) = filter.account_id {
            builder.push(" AND e.account_id = ");
            builder.push_bind(account_id);
        }
        if let Some(category_id) = filter.category_id {
            builder.push(" AND e.category_id = ");
            builder.push_bind(category_id);
        } else if filter.uncategorized.unwrap_or(false) {
            builder.push(" AND e.category_id IS NULL");
        }
        if let Some(reconciled) = filter.reconciled {
            builder.push(" AND e.reconciled = ");
            builder.push_bind(reconciled);
        }
        if let Some(min) = filter.amount_min_minor {
            builder.push(" AND e.amount_minor >= ");
            builder.push_bind(min);
        }
        if let Some(max) = filter.amount_max_minor {
            builder.push(" AND e.amount_minor <= ");
            builder.push_bind(max);
        }
    }

    fn apply_transfer_filters(builder: &mut QueryBuilder<Sqlite>, filter: &FilterDefinition) {
        if filter.category_id.is_some() || filter.uncategorized.unwrap_or(false) {
            builder.push(" AND 0 = 1");
        }
        Self::apply_search_filter(builder, "t.name", "t.note", filter);
        if let Some(account_id) = filter.account_id {
            builder.push(" AND (t.from_account_id = ");
            builder.push_bind(account_id);
            builder.push(" OR t.to_account_id = ");
            builder.push_bind(account_id);
            builder.push(")");
        }
        if let Some(reconciled) = filter.reconciled {
            builder.push(" AND oe.reconciled = ");
            builder.push_bind(reconciled);
        }
        if let Some(min) = filter.amount_min_minor {
            builder.push(" AND t.amount_minor >= ");
            builder.push_bind(min);
        }
        if let Some(max) = filter.amount_max_minor {
            builder.push(" AND t.amount_minor <= ");
            builder.push_bind(max);
        }
    }

    fn order_entries(entries: &mut [LedgerEntry]) {
        entries.sort_by(|left, right| {
            right
                .occurred_at
                .cmp(&left.occurred_at)
                .then(right.id.cmp(&left.id))
                .then(left.kind.cmp(&right.kind))
        });
    }

    fn build_page(
        mut entries: Vec<LedgerEntry>,
        limit: Option<usize>,
        offset: Option<usize>,
        normalize_dates: bool,
    ) -> AppResult<LedgerPage> {
        Self::order_entries(&mut entries);
        let total_count = entries.len();
        let mut totals = BTreeMap::<String, (i128, i128, i128)>::new();
        for entry in &entries {
            let legacy_date: String = entry.occurred_at.chars().take(10).collect();
            let date = if normalize_dates {
                date_only(&entry.occurred_at).unwrap_or(legacy_date)
            } else {
                legacy_date
            };
            let total = totals.entry(date).or_default();
            match entry.kind.as_str() {
                "income" => total.0 += i128::from(entry.amount_minor),
                "expense" => total.1 += i128::from(entry.amount_minor),
                "transfer" | "transfer_in" | "transfer_out" => {
                    total.2 += i128::from(entry.amount_minor)
                }
                _ => {}
            }
        }
        let daily_totals = totals
            .into_iter()
            .map(|(date, (income, expense, transfer))| {
                Ok(DailyTotal {
                    date,
                    income_minor: Store::checked_i64(income, "daily ledger income total")?,
                    expense_minor: Store::checked_i64(expense, "daily ledger expense total")?,
                    transfer_minor: Store::checked_i64(transfer, "daily ledger transfer total")?,
                })
            })
            .collect::<AppResult<Vec<_>>>()?;
        let limit = limit.unwrap_or(200).clamp(1, 200);
        let offset = offset.unwrap_or(0);
        let end = offset.saturating_add(limit).min(total_count);
        let paged = if offset < total_count {
            entries[offset..end].to_vec()
        } else {
            Vec::new()
        };
        let next_offset = (end < total_count).then_some(end);
        Ok(LedgerPage {
            entries: paged,
            daily_totals,
            total_count,
            next_offset,
        })
    }

    pub async fn ledger_entries(&self, filter: FilterDefinition) -> AppResult<Vec<LedgerEntry>> {
        Self::validate_filter(&filter)?;
        let mut entries_query = QueryBuilder::<Sqlite>::new(
            "SELECT e.id, e.kind, e.name, e.note, e.account_id, a.name AS account_name,
                    NULL AS counterpart_account_id, NULL AS counterpart_account_name,
                    e.amount_minor, e.occurred_at, e.category_id, c.name AS category_name,
                    p.name AS category_parent_name, e.reconciled, e.transfer_id
             FROM entries e
             JOIN accounts a ON a.id = e.account_id
             LEFT JOIN categories c ON c.id = e.category_id
             LEFT JOIN categories p ON p.id = c.parent_id
             WHERE e.kind IN ('income', 'expense')",
        );
        Self::apply_entry_filters(&mut entries_query, &filter);
        entries_query.push(" ORDER BY e.occurred_at DESC, e.id DESC");
        let mut entries = entries_query
            .build_query_as::<LedgerEntry>()
            .fetch_all(&self.pool)
            .await?;

        let mut transfers_query = QueryBuilder::<Sqlite>::new(
            "SELECT t.id, 'transfer' AS kind, t.name, t.note,
                    t.from_account_id AS account_id, fa.name AS account_name,
                    t.to_account_id AS counterpart_account_id, ta.name AS counterpart_account_name,
                    t.amount_minor, t.occurred_at, NULL AS category_id, NULL AS category_name,
                    NULL AS category_parent_name, oe.reconciled, t.id AS transfer_id
             FROM transfers t
             JOIN accounts fa ON fa.id = t.from_account_id
             JOIN accounts ta ON ta.id = t.to_account_id
             JOIN entries oe ON oe.transfer_id = t.id AND oe.kind = 'transfer_out'
             WHERE 1 = 1",
        );
        Self::apply_transfer_filters(&mut transfers_query, &filter);
        transfers_query.push(" ORDER BY t.occurred_at DESC, t.id DESC");
        let transfers = transfers_query
            .build_query_as::<LedgerEntry>()
            .fetch_all(&self.pool)
            .await?;
        entries.extend(transfers);
        Self::retain_date_matches(&mut entries, &filter);
        Self::order_entries(&mut entries);
        self.attach_labels(&mut entries).await?;
        Ok(entries)
    }

    pub async fn ledger(
        &self,
        filter: FilterDefinition,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> AppResult<LedgerPage> {
        let normalize_dates = Self::has_date_filter(&filter);
        Self::build_page(
            self.ledger_entries(filter).await?,
            limit,
            offset,
            normalize_dates,
        )
    }

    pub async fn invalid_date_expenses(
        &self,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> AppResult<LedgerPage> {
        let mut entries = sqlx::query_as::<_, LedgerEntry>(
            "SELECT e.id, e.kind, e.name, e.note, e.account_id, a.name AS account_name,
                    NULL AS counterpart_account_id, NULL AS counterpart_account_name,
                    e.amount_minor, e.occurred_at, e.category_id, c.name AS category_name,
                    p.name AS category_parent_name, e.reconciled, e.transfer_id
             FROM entries e
             JOIN accounts a ON a.id = e.account_id
             LEFT JOIN categories c ON c.id = e.category_id
             LEFT JOIN categories p ON p.id = c.parent_id
             WHERE e.kind = 'expense'
             ORDER BY e.occurred_at DESC, e.id DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        entries.retain(|entry| date_only(&entry.occurred_at).is_none());
        Self::order_entries(&mut entries);
        self.attach_labels(&mut entries).await?;
        let total_count = entries.len();
        let limit = limit.unwrap_or(200).clamp(1, 200);
        let offset = offset.unwrap_or(0);
        let end = offset.saturating_add(limit).min(total_count);
        let paged = if offset < total_count {
            entries[offset..end].to_vec()
        } else {
            Vec::new()
        };
        Ok(LedgerPage {
            entries: paged,
            daily_totals: Vec::new(),
            total_count,
            next_offset: (end < total_count).then_some(end),
        })
    }

    pub async fn account_history(
        &self,
        account_id: i64,
        mut filter: FilterDefinition,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> AppResult<LedgerPage> {
        Store::load_account(&self.pool, account_id).await?;
        Self::validate_filter(&filter)?;
        filter.account_id = Some(account_id);
        let mut entries_query = QueryBuilder::<Sqlite>::new(
            "SELECT e.id, e.kind, e.name, e.note, e.account_id, a.name AS account_name,
                    NULL AS counterpart_account_id, NULL AS counterpart_account_name,
                    e.amount_minor, e.occurred_at, e.category_id, c.name AS category_name,
                    p.name AS category_parent_name, e.reconciled, e.transfer_id
             FROM entries e
             JOIN accounts a ON a.id = e.account_id
             LEFT JOIN categories c ON c.id = e.category_id
             LEFT JOIN categories p ON p.id = c.parent_id
             WHERE e.account_id = ",
        );
        entries_query.push_bind(account_id);
        Self::apply_entry_filters(&mut entries_query, &filter);
        entries_query
            .push(" AND e.kind IN ('income', 'expense') ORDER BY e.occurred_at DESC, e.id DESC");
        let mut entries = entries_query
            .build_query_as::<LedgerEntry>()
            .fetch_all(&self.pool)
            .await?;

        let mut transfers_query = QueryBuilder::<Sqlite>::new(
            "SELECT t.id,
                    CASE WHEN t.from_account_id = oe.account_id THEN 'transfer_out' ELSE 'transfer_in' END AS kind,
                    t.name, t.note, oe.account_id, a.name AS account_name,
                    CASE WHEN t.from_account_id = oe.account_id THEN t.to_account_id ELSE t.from_account_id END AS counterpart_account_id,
                    CASE WHEN t.from_account_id = oe.account_id THEN ta.name ELSE fa.name END AS counterpart_account_name,
                    t.amount_minor, t.occurred_at, NULL AS category_id, NULL AS category_name,
                    NULL AS category_parent_name, oe.reconciled, t.id AS transfer_id
             FROM transfers t
             JOIN entries oe ON oe.transfer_id = t.id
             JOIN accounts a ON a.id = oe.account_id
             JOIN accounts fa ON fa.id = t.from_account_id
             JOIN accounts ta ON ta.id = t.to_account_id
             WHERE oe.account_id = ",
        );
        transfers_query.push_bind(account_id);
        Self::apply_transfer_filters(&mut transfers_query, &filter);
        transfers_query.push(
            " AND oe.kind IN ('transfer_in', 'transfer_out') ORDER BY t.occurred_at DESC, t.id DESC",
        );
        let transfers = transfers_query
            .build_query_as::<LedgerEntry>()
            .fetch_all(&self.pool)
            .await?;
        entries.extend(transfers);
        Self::retain_date_matches(&mut entries, &filter);
        self.attach_labels(&mut entries).await?;
        Self::build_page(entries, limit, offset, Self::has_date_filter(&filter))
    }

    pub async fn transaction_detail(&self, id: i64) -> AppResult<LedgerEntry> {
        let entry = sqlx::query_as::<_, LedgerEntry>(
            "SELECT e.id, e.kind, e.name, e.note, e.account_id, a.name AS account_name,
                    NULL AS counterpart_account_id, NULL AS counterpart_account_name,
                    e.amount_minor, e.occurred_at, e.category_id, c.name AS category_name,
                    p.name AS category_parent_name, e.reconciled, e.transfer_id
             FROM entries e
             JOIN accounts a ON a.id = e.account_id
             LEFT JOIN categories c ON c.id = e.category_id
             LEFT JOIN categories p ON p.id = c.parent_id
             WHERE e.id = ? AND e.transfer_id IS NULL",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("transaction {id} was not found")))?;
        let mut entries = vec![entry];
        self.attach_labels(&mut entries).await?;
        Ok(entries.remove(0))
    }

    pub async fn transfer_detail(&self, id: i64) -> AppResult<LedgerEntry> {
        let entry = sqlx::query_as::<_, LedgerEntry>(
            "SELECT t.id, 'transfer' AS kind, t.name, t.note,
                    t.from_account_id AS account_id, fa.name AS account_name,
                    t.to_account_id AS counterpart_account_id, ta.name AS counterpart_account_name,
                    t.amount_minor, t.occurred_at, NULL AS category_id, NULL AS category_name,
                    NULL AS category_parent_name, oe.reconciled, t.id AS transfer_id
             FROM transfers t
             JOIN accounts fa ON fa.id = t.from_account_id
             JOIN accounts ta ON ta.id = t.to_account_id
             JOIN entries oe ON oe.transfer_id = t.id AND oe.kind = 'transfer_out'
             WHERE t.id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("transfer {id} was not found")))?;
        let mut entries = vec![entry];
        self.attach_labels(&mut entries).await?;
        Ok(entries.remove(0))
    }
}
