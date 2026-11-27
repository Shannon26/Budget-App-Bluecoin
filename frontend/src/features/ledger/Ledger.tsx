import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type FormEvent,
} from "react";
import { api } from "../../shared/api";
import { formatMinor, parseMinorUnits } from "../../shared/money";
import type {
  BudgetState,
  DailyTotal,
  FilterDefinition,
  LedgerEntry,
} from "../../shared/types";
import EntryEditor from "./EntryEditor";
import { entryPresentation } from "./presentation";

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

interface LedgerViewIdentity {
  historyAccountId: number | null;
  filterKey: string;
  dataEntries: LedgerEntry[];
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

function appliedFilterKey(filter: FilterDefinition): string {
  return JSON.stringify([
    filter.search ?? null,
    filter.date_from ?? null,
    filter.date_to ?? null,
    filter.account_id ?? null,
    filter.category_id ?? null,
    filter.uncategorized ?? false,
    filter.reconciled ?? null,
    filter.amount_min_minor ?? null,
    filter.amount_max_minor ?? null,
  ]);
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
  const [loadingMore, setLoadingMore] = useState(false);
  const [viewStatus, setViewStatus] = useState<"loading" | "ready" | "error">("loading");
  const [statusViewIdentity, setStatusViewIdentity] = useState<LedgerViewIdentity | null>(null);
  const [exporting, setExporting] = useState(false);
  const [busy, setBusy] = useState(false);

  const viewIdentity = useMemo<LedgerViewIdentity>(
    () => ({
      historyAccountId,
      filterKey: appliedFilterKey(applied),
      dataEntries: data.entries,
    }),
    [applied, data.entries, historyAccountId],
  );
  const activeViewIdentityRef = useRef<LedgerViewIdentity | null>(viewIdentity);

  useLayoutEffect(() => {
    activeViewIdentityRef.current = viewIdentity;
    return () => {
      if (activeViewIdentityRef.current === viewIdentity) {
        activeViewIdentityRef.current = null;
      }
    };
  }, [viewIdentity]);

  const mountedRef = useRef(false);
  const requestSequenceRef = useRef(0);

  const isCurrentRequest = useCallback(
    (request: number, requestViewIdentity: LedgerViewIdentity) =>
      mountedRef.current &&
      request === requestSequenceRef.current &&
      requestViewIdentity === activeViewIdentityRef.current,
    [],
  );

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      requestSequenceRef.current += 1;
    };
  }, []);

  const loadPage = useCallback(
    async (offset = 0, append = false) => {
      const request = ++requestSequenceRef.current;
      const requestViewIdentity = viewIdentity;
      if (append) {
        setLoadingMore(true);
      } else {
        setViewStatus("loading");
        setStatusViewIdentity(requestViewIdentity);
      }
      try {
        const result = historyAccountId
          ? await api.accountHistory(historyAccountId, offset)
          : await api.ledger(applied, offset);
        if (!isCurrentRequest(request, requestViewIdentity)) return;
        setEntries((current) => {
          if (!isCurrentRequest(request, requestViewIdentity)) return current;
          const merged = append ? [...current, ...result.entries] : result.entries;
          return [
            ...new Map(
              merged.map((entry) => [`${entry.kind}-${entry.id}`, entry]),
            ).values(),
          ];
        });
        setDailyTotals(result.daily_totals);
        setTotalCount(result.total_count);
        setNextOffset(result.next_offset);
        setViewStatus("ready");
        setStatusViewIdentity(requestViewIdentity);
      } catch (reason) {
        if (!isCurrentRequest(request, requestViewIdentity)) return;
        if (!append) {
          setViewStatus("error");
          setStatusViewIdentity(requestViewIdentity);
        }
        onMessage(reason instanceof Error ? reason.message : "Could not load ledger", true);
      } finally {
        if (isCurrentRequest(request, requestViewIdentity)) {
          setLoadingMore(false);
        }
      }
    },
    [applied, historyAccountId, isCurrentRequest, onMessage, viewIdentity],
  );

  useEffect(() => {
    requestSequenceRef.current += 1;
    setEntries([]);
    setDailyTotals([]);
    setTotalCount(0);
    setNextOffset(null);
    setLoadingMore(false);
    setViewStatus("loading");
    setStatusViewIdentity(viewIdentity);
    void loadPage();
  }, [data.entries, loadPage, viewIdentity]);

  const activeViewStatus =
    statusViewIdentity === viewIdentity ? viewStatus : "loading";
  const visibleEntries = statusViewIdentity === viewIdentity ? entries : [];
  const visibleDailyTotals =
    statusViewIdentity === viewIdentity ? dailyTotals : [];
  const visibleTotalCount =
    statusViewIdentity === viewIdentity ? totalCount : 0;
  const visibleNextOffset =
    statusViewIdentity === viewIdentity && activeViewStatus === "ready"
      ? nextOffset
      : null;

  const groups = useMemo(() => {
    const totalsByDate = new Map(visibleDailyTotals.map((total) => [total.date, total]));
    const map = new Map<
      string,
      { date: string; entries: LedgerEntry[]; income: number; expense: number; transfer: number }
    >();
    for (const entry of visibleEntries) {
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
  }, [visibleEntries, visibleDailyTotals]);

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

      {activeViewStatus === "loading" ? (
        <div className="empty-state">Loading ledger…</div>
      ) : activeViewStatus === "error" ? (
        <div className="empty-state">Could not load ledger.</div>
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

      {visibleNextOffset !== null && (
        <div className="load-more">
          <span>Showing {visibleEntries.length} of {visibleTotalCount} matching entries</span>
          <button
            className="secondary-button"
            data-testid="load-more"
            type="button"
            disabled={loadingMore}
            onClick={() => void loadPage(visibleNextOffset, true)}
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
