import { useCallback, useEffect, useMemo, useState, type FormEvent } from "react";
import { api } from "./api";
import { formatMinor, parseMinorUnits } from "./money";
import type {
  BudgetState,
  DailyTotal,
  FilterDefinition,
  LedgerEntry,
  TransactionKind,
} from "./types";

interface LedgerProps {
  data: BudgetState;
  refresh: () => Promise<void>;
  onMessage: (message: string, error?: boolean) => void;
  historyAccountId: number | null;
  onClearHistory: () => void;
}

interface DraftFilter {
  search: string;
  date_from: string;
  date_to: string;
  account_id: string;
  category_id: string;
  uncategorized: boolean;
  reconciled: string;
  amount_min: string;
  amount_max: string;
}

const emptyDraft: DraftFilter = {
  search: "",
  date_from: "",
  date_to: "",
  account_id: "",
  category_id: "",
  uncategorized: false,
  reconciled: "",
  amount_min: "",
  amount_max: "",
};

function draftFromFilter(filter: FilterDefinition): DraftFilter {
  return {
    search: filter.search ?? "",
    date_from: filter.date_from ?? "",
    date_to: filter.date_to ?? "",
    account_id: filter.account_id ? String(filter.account_id) : "",
    category_id: filter.category_id ? String(filter.category_id) : "",
    uncategorized: filter.uncategorized ?? false,
    reconciled:
      filter.reconciled === null || filter.reconciled === undefined
        ? ""
        : filter.reconciled
          ? "true"
          : "false",
    amount_min:
      filter.amount_min_minor === null || filter.amount_min_minor === undefined
        ? ""
        : (filter.amount_min_minor / 100).toFixed(2),
    amount_max:
      filter.amount_max_minor === null || filter.amount_max_minor === undefined
        ? ""
        : (filter.amount_max_minor / 100).toFixed(2),
  };
}

function entryPresentation(entry: LedgerEntry) {
  if (entry.kind === "transfer") {
    return {
      label: "Transfer",
      sign: "⇄",
      tone: "transfer",
      account: `${entry.account_name} → ${entry.counterpart_account_name ?? ""}`,
      category: "Not categorized",
    };
  }
  if (entry.kind === "transfer_in" || entry.kind === "transfer_out") {
    return {
      label: entry.kind === "transfer_in" ? "Transfer in" : "Transfer out",
      sign: entry.kind === "transfer_in" ? "+" : "−",
      tone: "transfer",
      account: `${entry.account_name} · ${entry.counterpart_account_name ?? ""}`,
      category: "Not categorized",
    };
  }
  return {
    label: entry.kind === "income" ? "Income" : "Expense",
    sign: entry.kind === "income" ? "+" : "−",
    tone: entry.kind === "income" ? "positive" : "negative",
    account: entry.account_name,
    category: entry.category_name
      ? `${entry.category_parent_name ? `${entry.category_parent_name} / ` : ""}${entry.category_name}`
      : "Uncategorized",
  };
}

function EntryEditor({
  entry,
  data,
  onClose,
  onSaved,
}: {
  entry: LedgerEntry;
  data: BudgetState;
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const isTransfer =
    entry.kind === "transfer" || entry.kind === "transfer_in" || entry.kind === "transfer_out";
  const [kind, setKind] = useState<TransactionKind>(
    entry.kind === "income" ? "income" : "expense",
  );
  const [name, setName] = useState(entry.name);
  const [accountId, setAccountId] = useState(
    String(entry.kind === "transfer_in" ? entry.counterpart_account_id : entry.account_id),
  );
  const [counterpartId, setCounterpartId] = useState(
    String(entry.kind === "transfer_in" ? entry.account_id : entry.counterpart_account_id),
  );
  const [categoryId, setCategoryId] = useState(
    entry.category_id ? String(entry.category_id) : "",
  );
  const [amount, setAmount] = useState((entry.amount_minor / 100).toFixed(2));
  const [note, setNote] = useState(entry.note);
  const [date, setDate] = useState(entry.occurred_at.slice(0, 10));
  const [reconciled, setReconciled] = useState(entry.reconciled);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const activeCategories = data.categories.filter(
    (category) =>
      category.kind === kind &&
      (category.archived_at === null || category.id === entry.category_id),
  );

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSaving(true);
    setError(null);
    try {
      const amountMinor = parseMinorUnits(amount);
      if (amountMinor === null || amountMinor <= 0) {
        throw new Error("Enter a valid amount greater than zero");
      }
      if (isTransfer) {
        await api.updateTransfer(entry.id, {
          from_account_id: Number(accountId),
          to_account_id: Number(counterpartId),
          amount_minor: amountMinor,
          name,
          reconciled,
          note,
          occurred_at: date ? `${date}T12:00:00` : undefined,
        });
      } else {
        await api.updateTransaction(entry.id, {
          kind,
          account_id: Number(accountId),
          amount_minor: amountMinor,
          name,
          category_id: categoryId ? Number(categoryId) : null,
          reconciled,
          note,
          occurred_at: date ? `${date}T12:00:00` : undefined,
        });
      }
      await onSaved();
      onClose();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Could not save changes");
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="modal-backdrop" role="presentation">
      <section className="modal-card" role="dialog" aria-modal="true" aria-label="Edit entry">
        <div className="panel-heading">
          <div>
            <p className="section-kicker">{isTransfer ? "Transfer" : "Transaction"}</p>
            <h2>Edit entry</h2>
          </div>
          <button className="icon-button" type="button" onClick={onClose} aria-label="Close editor">
            ×
          </button>
        </div>
        {error && <div className="message message-error">{error}</div>}
        <form className="stack-form" onSubmit={submit}>
          <label>
            Name
            <input
              data-testid="edit-name"
              value={name}
              onChange={(event) => setName(event.target.value)}
              required
            />
          </label>
          {!isTransfer && (
            <div className="form-row">
              <label>
                Type
                <select
                  value={kind}
                  onChange={(event) => {
                    const next = event.target.value as TransactionKind;
                    setKind(next);
                    setCategoryId("");
                  }}
                >
                  <option value="expense">Expense</option>
                  <option value="income">Income</option>
                </select>
              </label>
              <label>
                Category
                <select
                  data-testid="edit-category"
                  value={categoryId}
                  onChange={(event) => setCategoryId(event.target.value)}
                >
                  <option value="">Uncategorized</option>
                  {activeCategories.map((category) => (
                    <option key={category.id} value={category.id}>
                      {category.parent_name ? `${category.parent_name} / ` : ""}
                      {category.name}
                      {category.archived_at ? " (archived)" : ""}
                    </option>
                  ))}
                </select>
              </label>
            </div>
          )}
          <div className={isTransfer ? "form-row" : ""}>
            <label>
              {isTransfer ? "From" : "Account"}
              <select
                data-testid="edit-account"
                value={accountId}
                onChange={(event) => setAccountId(event.target.value)}
                required
              >
                {data.accounts.map((account) => (
                  <option key={account.id} value={account.id}>
                    {account.name}
                  </option>
                ))}
              </select>
            </label>
            {isTransfer && (
              <label>
                To
                <select
                  data-testid="edit-counterpart"
                  value={counterpartId}
                  onChange={(event) => setCounterpartId(event.target.value)}
                  required
                >
                  {data.accounts.map((account) => (
                    <option key={account.id} value={account.id}>
                      {account.name}
                    </option>
                  ))}
                </select>
              </label>
            )}
          </div>
          <div className="form-row">
            <label>
              Amount
              <input
                data-testid="edit-amount"
                value={amount}
                onChange={(event) => setAmount(event.target.value)}
                inputMode="decimal"
                required
              />
            </label>
            <label>
              Date
              <input
                type="date"
                value={date}
                onChange={(event) => setDate(event.target.value)}
                required
              />
            </label>
          </div>
          <label>
            Note
            <textarea value={note} onChange={(event) => setNote(event.target.value)} rows={3} />
          </label>
          <label className="check-label">
            <input
              type="checkbox"
              checked={reconciled}
              onChange={(event) => setReconciled(event.target.checked)}
            />
            Reconciled
          </label>
          <button className="primary-button" data-testid="save-edit" disabled={saving}>
            {saving ? "Saving…" : "Save changes"}
          </button>
        </form>
      </section>
    </div>
  );
}

export default function Ledger({
  data,
  refresh,
  onMessage,
  historyAccountId,
  onClearHistory,
}: LedgerProps) {
  const [draft, setDraft] = useState<DraftFilter>(emptyDraft);
  const [applied, setApplied] = useState<FilterDefinition>({});
  const [entries, setEntries] = useState<LedgerEntry[]>(data.entries);
  const [dailyTotals, setDailyTotals] = useState<DailyTotal[]>([]);
  const [totalCount, setTotalCount] = useState(data.entries.length);
  const [nextOffset, setNextOffset] = useState<number | null>(null);
  const [selected, setSelected] = useState<LedgerEntry | null>(null);
  const [editing, setEditing] = useState<LedgerEntry | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<LedgerEntry | null>(null);
  const [filterName, setFilterName] = useState("");
  const [selectedFilterId, setSelectedFilterId] = useState("");
  const [loading, setLoading] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [busy, setBusy] = useState(false);

  const loadPage = useCallback(async (offset = 0, append = false) => {
    if (append) {
      setLoadingMore(true);
    } else {
      setLoading(true);
    }
    try {
      const result = historyAccountId
        ? await api.accountHistory(historyAccountId, offset)
        : await api.ledger(applied, offset);
      setEntries((current) => {
        const merged = append ? [...current, ...result.entries] : result.entries;
        return [...new Map(merged.map((entry) => [`${entry.kind}-${entry.id}`, entry])).values()];
      });
      setDailyTotals(result.daily_totals);
      setTotalCount(result.total_count);
      setNextOffset(result.next_offset);
    } catch (reason) {
      onMessage(reason instanceof Error ? reason.message : "Could not load ledger", true);
    } finally {
      setLoading(false);
      setLoadingMore(false);
    }
  }, [applied, historyAccountId, onMessage]);

  useEffect(() => {
    void loadPage();
  }, [loadPage, data.entries]);

  const groups = useMemo(() => {
    const totalsByDate = new Map(dailyTotals.map((total) => [total.date, total]));
    const map = new Map<
      string,
      { date: string; entries: LedgerEntry[]; income: number; expense: number; transfer: number }
    >();
    for (const entry of entries) {
      const date = entry.occurred_at.slice(0, 10);
      const total = totalsByDate.get(date);
      const group = map.get(date) ?? {
        date,
        entries: [],
        income: total?.income_minor ?? 0,
        expense: total?.expense_minor ?? 0,
        transfer: total?.transfer_minor ?? 0,
      };
      group.entries.push(entry);
      map.set(date, group);
    }
    return [...map.values()].sort((left, right) => right.date.localeCompare(left.date));
  }, [entries, dailyTotals]);

  function applyFilters() {
    const min = draft.amount_min ? parseMinorUnits(draft.amount_min) : null;
    const max = draft.amount_max ? parseMinorUnits(draft.amount_max) : null;
    if (draft.amount_min && min === null) {
      onMessage("Enter a valid minimum amount", true);
      return;
    }
    if (draft.amount_max && max === null) {
      onMessage("Enter a valid maximum amount", true);
      return;
    }
    if (min !== null && max !== null && min > max) {
      onMessage("Minimum amount cannot be greater than maximum amount", true);
      return;
    }
    setApplied({
      search: draft.search.trim() || null,
      date_from: draft.date_from || null,
      date_to: draft.date_to || null,
      account_id: draft.account_id ? Number(draft.account_id) : null,
      category_id: draft.category_id ? Number(draft.category_id) : null,
      uncategorized: draft.uncategorized,
      reconciled:
        draft.reconciled === "" ? null : draft.reconciled === "true",
      amount_min_minor: min,
      amount_max_minor: max,
    });
  }

  async function downloadCsv() {
    setExporting(true);
    try {
      const blob = await api.downloadLedgerCsv(applied);
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = "paisa-ledger.csv";
      document.body.appendChild(link);
      link.click();
      link.remove();
      window.setTimeout(() => URL.revokeObjectURL(url), 0);
      onMessage("CSV downloaded with all matching entries.");
    } catch (reason) {
      onMessage(reason instanceof Error ? reason.message : "Could not export ledger", true);
    } finally {
      setExporting(false);
    }
  }

  async function saveFilter() {
    if (!filterName.trim()) {
      onMessage("Give this filter a name first", true);
      return;
    }
    try {
      await api.createFilter({ name: filterName, definition: applied });
      setFilterName("");
      await refresh();
      onMessage("Named filter saved.");
    } catch (reason) {
      onMessage(reason instanceof Error ? reason.message : "Could not save filter", true);
    }
  }

  async function removeFilter(id: number) {
    try {
      await api.deleteFilter(id);
      setSelectedFilterId("");
      await refresh();
      onMessage("Saved filter removed.");
    } catch (reason) {
      onMessage(reason instanceof Error ? reason.message : "Could not remove filter", true);
    }
  }

  async function duplicate(entry: LedgerEntry) {
    setBusy(true);
    try {
      if (entry.kind === "transfer" || entry.kind === "transfer_in" || entry.kind === "transfer_out") {
        await api.duplicateTransfer(entry.id);
      } else {
        await api.duplicateTransaction(entry.id);
      }
      await refresh();
      setSelected(null);
      onMessage("Entry duplicated.");
    } catch (reason) {
      onMessage(reason instanceof Error ? reason.message : "Could not duplicate entry", true);
    } finally {
      setBusy(false);
    }
  }

  async function toggleReconciled(entry: LedgerEntry) {
    setBusy(true);
    try {
      if (entry.kind === "transfer" || entry.kind === "transfer_in" || entry.kind === "transfer_out") {
        await api.reconcileTransfer(entry.id, !entry.reconciled);
      } else {
        await api.reconcileTransaction(entry.id, !entry.reconciled);
      }
      await refresh();
      setSelected(null);
      onMessage(entry.reconciled ? "Marked unreconciled." : "Marked reconciled.");
    } catch (reason) {
      onMessage(reason instanceof Error ? reason.message : "Could not update status", true);
    } finally {
      setBusy(false);
    }
  }

  async function permanentDelete() {
    if (!deleteTarget) return;
    setBusy(true);
    try {
      if (deleteTarget.kind === "transfer" || deleteTarget.kind === "transfer_in" || deleteTarget.kind === "transfer_out") {
        await api.deleteTransfer(deleteTarget.id);
      } else {
        await api.deleteTransaction(deleteTarget.id);
      }
      await refresh();
      setDeleteTarget(null);
      setSelected(null);
      onMessage("Entry permanently deleted.");
    } catch (reason) {
      onMessage(reason instanceof Error ? reason.message : "Could not delete entry", true);
    } finally {
      setBusy(false);
    }
  }

  const historyAccount = data.accounts.find((account) => account.id === historyAccountId);

  return (
    <section className="ledger-section">
      <div className="panel-heading ledger-heading">
        <div>
          <p className="section-kicker">{historyAccount ? "Account history" : "All activity"}</p>
          <h2>{historyAccount ? historyAccount.name : "Ledger"}</h2>
        </div>
        <div className="ledger-heading-actions">
          {!historyAccount && (
            <button
              className="secondary-button"
              data-testid="download-csv"
              type="button"
              disabled={exporting}
              onClick={() => void downloadCsv()}
            >
              {exporting ? "Preparing…" : "Download CSV"}
            </button>
          )}
          {historyAccount && (
            <button className="text-button" type="button" onClick={onClearHistory}>
              Back to full ledger
            </button>
          )}
        </div>
      </div>

      {!historyAccount && (
        <div className="filter-panel">
          <div className="filter-grid">
            <label className="filter-search">
              Search
              <input
                data-testid="ledger-search"
                value={draft.search}
                onChange={(event) => setDraft({ ...draft, search: event.target.value })}
                placeholder="Name or note"
              />
            </label>
            <label>
              From date
              <input
                data-testid="filter-date-from"
                type="date"
                value={draft.date_from}
                onChange={(event) => setDraft({ ...draft, date_from: event.target.value })}
              />
            </label>
            <label>
              To date
              <input
                data-testid="filter-date-to"
                type="date"
                value={draft.date_to}
                onChange={(event) => setDraft({ ...draft, date_to: event.target.value })}
              />
            </label>
            <label>
              Account
              <select
                data-testid="filter-account"
                value={draft.account_id}
                onChange={(event) => setDraft({ ...draft, account_id: event.target.value })}
              >
                <option value="">All accounts</option>
                {data.accounts.map((account) => (
                  <option key={account.id} value={account.id}>
                    {account.name}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Category
              <select
                data-testid="filter-category"
                value={draft.category_id}
                onChange={(event) =>
                  setDraft({
                    ...draft,
                    category_id: event.target.value,
                    uncategorized: false,
                  })
                }
              >
                <option value="">All categories</option>
                {data.categories.map((category) => (
                  <option key={category.id} value={category.id}>
                    {category.parent_name ? `${category.parent_name} / ` : ""}
                    {category.name}
                    {category.archived_at ? " (archived)" : ""}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Status
              <select
                data-testid="filter-status"
                value={draft.reconciled}
                onChange={(event) => setDraft({ ...draft, reconciled: event.target.value })}
              >
                <option value="">Any status</option>
                <option value="true">Reconciled</option>
                <option value="false">Unreconciled</option>
              </select>
            </label>
            <label>
              Min amount
              <input
                data-testid="filter-amount-min"
                value={draft.amount_min}
                onChange={(event) => setDraft({ ...draft, amount_min: event.target.value })}
                inputMode="decimal"
                placeholder="0.00"
              />
            </label>
            <label>
              Max amount
              <input
                data-testid="filter-amount-max"
                value={draft.amount_max}
                onChange={(event) => setDraft({ ...draft, amount_max: event.target.value })}
                inputMode="decimal"
                placeholder="0.00"
              />
            </label>
          </div>
          <div className="filter-actions">
            <label className="check-label inline-check">
              <input
                data-testid="filter-uncategorized"
                type="checkbox"
                checked={draft.uncategorized}
                onChange={(event) =>
                  setDraft({ ...draft, uncategorized: event.target.checked, category_id: "" })
                }
              />
              Uncategorized only
            </label>
            <button className="primary-button compact-button" data-testid="apply-filters" type="button" onClick={applyFilters}>
              Apply filters
            </button>
            <button
              className="secondary-button"
              type="button"
              onClick={() => {
                setDraft(emptyDraft);
                setApplied({});
              }}
            >
              Reset
            </button>
          </div>
          <div className="saved-filter-bar">
            <label>
              Saved filters
              <select
                data-testid="saved-filter"
                value={selectedFilterId}
                onChange={(event) => {
                  const selectedId = event.target.value;
                  setSelectedFilterId(selectedId);
                  const filter = data.saved_filters.find(
                    (saved) => String(saved.id) === selectedId,
                  );
                  if (filter) {
                    setDraft(draftFromFilter(filter.definition));
                    setApplied(filter.definition);
                  }
                }}
              >
                <option value="">Choose a saved filter</option>
                {data.saved_filters.map((filter) => (
                  <option key={filter.id} value={filter.id}>
                    {filter.name}
                  </option>
                ))}
              </select>
            </label>
            <input
              data-testid="save-filter-name"
              value={filterName}
              onChange={(event) => setFilterName(event.target.value)}
              placeholder="Name this view"
            />
            <button className="secondary-button" data-testid="save-filter" type="button" onClick={() => void saveFilter()}>
              Save view
            </button>
            {data.saved_filters.length > 0 && (
              <button
                className="text-button danger-text"
                type="button"
                onClick={() => {
                  const selectedId = Number(selectedFilterId);
                  if (selectedId) void removeFilter(selectedId);
                }}
              >
                Delete selected
              </button>
            )}
          </div>
        </div>
      )}

      {loading && entries.length === 0 ? (
        <div className="empty-state">Loading ledger…</div>
      ) : groups.length === 0 ? (
        <div className="empty-state">No entries match this view.</div>
      ) : (
        <div className="ledger-groups">
          {groups.map((group) => (
            <article className="ledger-day" key={group.date}>
              <header className="ledger-day-header">
                <h3>{group.date}</h3>
                <div>
                  <span>Income {formatMinor(group.income)}</span>
                  <span>Expense {formatMinor(group.expense)}</span>
                  {group.transfer > 0 && <span>Transfers {formatMinor(group.transfer)}</span>}
                </div>
              </header>
              <div className="ledger-rows">
                {group.entries.map((entry) => {
                  const details = entryPresentation(entry);
                  return (
                    <button
                      className="ledger-row"
                      data-testid={`ledger-row-${entry.kind}-${entry.id}`}
                      key={`${entry.kind}-${entry.id}`}
                      type="button"
                      onClick={() => setSelected(entry)}
                    >
                      <span className={`entry-icon ${details.tone}`}>{details.sign}</span>
                      <span className="entry-main">
                        <strong>{entry.name || details.label}</strong>
                        <span>
                          {details.account} · {details.category}
                        </span>
                      </span>
                      {entry.reconciled && <span className="reconciled-badge">✓ Reconciled</span>}
                      <strong className={`entry-amount ${details.tone}`}>
                        {details.sign}
                        {formatMinor(entry.amount_minor)}
                      </strong>
                    </button>
                  );
                })}
              </div>
            </article>
          ))}
        </div>
      )}

      {nextOffset !== null && (
        <div className="load-more">
          <span>Showing {entries.length} of {totalCount} matching entries</span>
          <button
            className="secondary-button"
            data-testid="load-more"
            type="button"
            disabled={loadingMore}
            onClick={() => void loadPage(nextOffset, true)}
          >
            {loadingMore ? "Loading…" : "Load more"}
          </button>
        </div>
      )}

      {selected && (
        <div className="modal-backdrop" role="presentation">
          <section className="modal-card detail-card" role="dialog" aria-modal="true" aria-label="Entry details">
            <div className="panel-heading">
              <div>
                <p className="section-kicker">{entryPresentation(selected).label}</p>
                <h2>{selected.name || entryPresentation(selected).label}</h2>
              </div>
              <button className="icon-button" type="button" onClick={() => setSelected(null)} aria-label="Close details">
                ×
              </button>
            </div>
            <div className={`detail-amount ${entryPresentation(selected).tone}`}>
              {entryPresentation(selected).sign}
              {formatMinor(selected.amount_minor)}
            </div>
            <dl className="detail-list">
              <div><dt>Date</dt><dd>{selected.occurred_at.slice(0, 10)}</dd></div>
              <div><dt>Account</dt><dd>{selected.account_name}</dd></div>
              {selected.kind === "transfer" && (
                <div><dt>To</dt><dd>{selected.counterpart_account_name}</dd></div>
              )}
              {selected.kind.startsWith("transfer_") && (
                <div><dt>Counterparty</dt><dd>{selected.counterpart_account_name}</dd></div>
              )}
              <div><dt>Category</dt><dd>{entryPresentation(selected).category}</dd></div>
              <div><dt>Status</dt><dd>{selected.reconciled ? "Reconciled" : "Unreconciled"}</dd></div>
              {selected.note && <div><dt>Note</dt><dd>{selected.note}</dd></div>}
            </dl>
            <div className="detail-actions">
              <button className="primary-button" type="button" onClick={() => setEditing(selected)}>
                Edit
              </button>
              <button className="secondary-button" type="button" disabled={busy} onClick={() => void duplicate(selected)}>
                Duplicate
              </button>
              <button className="secondary-button" type="button" disabled={busy} onClick={() => void toggleReconciled(selected)}>
                {selected.reconciled ? "Mark unreconciled" : "Mark reconciled"}
              </button>
              <button className="danger-button" type="button" onClick={() => setDeleteTarget(selected)}>
                Delete
              </button>
            </div>
          </section>
        </div>
      )}

      {editing && (
        <EntryEditor
          key={`${editing.kind}-${editing.id}`}
          entry={editing}
          data={data}
          onClose={() => setEditing(null)}
          onSaved={async () => {
            await refresh();
            await loadPage();
            setSelected(null);
          }}
        />
      )}

      {deleteTarget && (
        <div className="modal-backdrop" role="presentation">
          <section className="modal-card confirm-card" role="dialog" aria-modal="true" aria-label="Confirm permanent deletion">
            <p className="section-kicker">Permanent action</p>
            <h2>Delete this entry?</h2>
            <p>
              {deleteTarget.kind === "transfer" || deleteTarget.kind === "transfer_in" || deleteTarget.kind === "transfer_out"
                ? "Both transfer legs will be permanently deleted together."
                : "This transaction will be permanently deleted."}
            </p>
            <div className="detail-actions">
              <button className="danger-button" data-testid="confirm-delete" type="button" disabled={busy} onClick={() => void permanentDelete()}>
                Permanently delete
              </button>
              <button className="secondary-button" type="button" onClick={() => setDeleteTarget(null)}>
                Cancel
              </button>
            </div>
          </section>
        </div>
      )}
    </section>
  );
}
