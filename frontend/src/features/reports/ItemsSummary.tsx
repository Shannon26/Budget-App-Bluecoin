import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../../shared/api";
import { localDateValue } from "../../shared/date";
import { formatMinor } from "../../shared/money";
import type { ItemsSummaryReport, ReportItem } from "../../shared/types";

const MONTH_NAMES = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];

const MIN_MONTH = "0001-02";
const MAX_MONTH = "9999-11";

type LoadState =
  | { month: string; status: "loading" }
  | { month: string; status: "ready" }
  | { month: string; status: "error"; message: string };

function monthLabel(month: string): string {
  const year = month.slice(0, 4);
  const monthIndex = Number(month.slice(5, 7)) - 1;
  return `${MONTH_NAMES[monthIndex] ?? month} ${year}`;
}

function sharePercent(amount: number, total: number): number {
  if (total <= 0) return 0;
  return Math.max(0, Math.min(100, (amount / total) * 100));
}

function summaryValue(state: LoadState, value: number | null): string {
  if (state.status === "loading") return "Pending…";
  if (state.status === "error") return "Unavailable";
  return value === null ? "—" : formatMinor(value);
}

function SummaryCard({
  kind,
  label,
  amount,
  count,
  state,
}: {
  kind: "income" | "expense";
  label: string;
  amount: number | null;
  count: number | null;
  state: LoadState;
}) {
  return (
    <article className={`report-total-card report-total-${kind}`}>
      <p className="section-kicker">{label}</p>
      <strong data-testid={`items-${kind}-total`}>
        {summaryValue(state, amount)}
      </strong>
      <span data-testid={`items-${kind}-count`}>
        {state.status === "loading"
          ? "Pending…"
          : state.status === "error"
            ? "Unavailable"
            : `${count ?? 0} ${count === 1 ? "entry" : "entries"}`}
      </span>
    </article>
  );
}

function ItemTable({
  kind,
  title,
  items,
  total,
  month,
}: {
  kind: "income" | "expense";
  title: string;
  items: ReportItem[];
  total: number;
  month: string;
}) {
  return (
    <section className="report-item-section" aria-labelledby={`items-${kind}-title`}>
      <div className="panel-heading">
        <div>
          <p className="section-kicker">{monthLabel(month)}</p>
          <h3 id={`items-${kind}-title`}>{title}</h3>
        </div>
        <strong>{formatMinor(total)}</strong>
      </div>
      <div className="report-table-wrap">
        <table className="report-table report-item-table">
          <caption className="sr-only">
            {title} for {monthLabel(month)}
          </caption>
          <thead>
            <tr>
              <th scope="col">Rank</th>
              <th scope="col">Item / payee</th>
              <th scope="col" className="number-cell">Entries</th>
              <th scope="col" className="number-cell">Amount</th>
              <th scope="col">Share</th>
            </tr>
          </thead>
          <tbody>
            {items.length === 0 ? (
              <tr>
                <td colSpan={5} className="report-item-empty">
                  No {kind === "income" ? "income" : "expense"} items this month.
                </td>
              </tr>
            ) : (
              items.map((item, index) => (
                <tr
                  key={item.name}
                  data-testid={`items-${kind}-row-${index}`}
                >
                  <td data-testid={`items-${kind}-row-${index}-rank`}>
                    {item.rank}
                  </td>
                  <th scope="row">{item.name}</th>
                  <td
                    className="number-cell"
                    data-testid={`items-${kind}-row-${index}-count`}
                  >
                    {item.entry_count}
                  </td>
                  <td
                    className="number-cell"
                    data-testid={`items-${kind}-row-${index}-amount`}
                  >
                    {formatMinor(item.total_minor)}
                  </td>
                  <td>
                    <div className="report-item-share">
                      <span
                        className={`report-item-bar report-item-bar-${kind}`}
                        style={{ width: `${sharePercent(item.total_minor, total)}%` }}
                        aria-hidden="true"
                      />
                      <span>{Math.round(sharePercent(item.total_minor, total))}%</span>
                    </div>
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </section>
  );
}

export default function ItemsSummary() {
  const [month, setMonth] = useState(() => localDateValue().slice(0, 7));
  const [result, setResult] = useState<ItemsSummaryReport | null>(null);
  const [state, setState] = useState<LoadState>({ month, status: "loading" });
  const requestSequence = useRef(0);
  const mounted = useRef(false);

  const load = useCallback(async () => {
    const request = ++requestSequence.current;
    const requestMonth = month;
    setState({ month: requestMonth, status: "loading" });
    try {
      const next = await api.itemsSummary(requestMonth);
      if (!mounted.current || request !== requestSequence.current || next.month !== requestMonth) {
        return;
      }
      setResult(next);
      setState({ month: requestMonth, status: "ready" });
    } catch (reason) {
      if (!mounted.current || request !== requestSequence.current) return;
      setState({
        month: requestMonth,
        status: "error",
        message: reason instanceof Error ? reason.message : "Could not load Items Summary",
      });
    }
  }, [month]);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      requestSequence.current += 1;
    };
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  function selectMonth(nextMonth: string) {
    if (!nextMonth || nextMonth === month) return;
    requestSequence.current += 1;
    setResult(null);
    setState({ month: nextMonth, status: "loading" });
    setMonth(nextMonth);
  }

  const activeState = state.month === month ? state : { month, status: "loading" as const };
  const visibleResult =
    result?.month === month && activeState.status === "ready" ? result : null;

  return (
    <section className="reports-section">
      <div className="panel-heading ledger-heading report-heading">
        <div>
          <p className="section-kicker">Income and expenses by exact name</p>
          <h2>Items Summary</h2>
          <p className="report-intro">
            Ranked transaction names and entry counts for the selected calendar month.
          </p>
        </div>
        <label className="report-month-control">
          Report month
          <input
            data-testid="items-month"
            type="month"
            min={MIN_MONTH}
            max={MAX_MONTH}
            value={month}
            onChange={(event) => selectMonth(event.target.value)}
          />
        </label>
      </div>

      <p className="field-hint report-range-hint">
        Supported report months: {MIN_MONTH} through {MAX_MONTH}.
      </p>

      <div className="report-total-grid">
        <SummaryCard
          kind="income"
          label="Income"
          amount={visibleResult?.current.income_minor ?? null}
          count={visibleResult?.current.income_count ?? null}
          state={activeState}
        />
        <SummaryCard
          kind="expense"
          label="Spending"
          amount={visibleResult?.current.expense_minor ?? null}
          count={visibleResult?.current.expense_count ?? null}
          state={activeState}
        />
      </div>

      {activeState.status === "loading" && (
        <div className="empty-state" data-testid="items-loading" role="status">
          Loading Items Summary…
        </div>
      )}

      {activeState.status === "error" && (
        <div className="empty-state report-error" data-testid="items-error" role="alert">
          <h3>Could not load Items Summary</h3>
          <p>{activeState.message}</p>
          <button className="secondary-button" type="button" onClick={() => void load()}>
            Try again
          </button>
        </div>
      )}

      {visibleResult && (
        <>
          {visibleResult.income_items.length === 0 && visibleResult.expense_items.length === 0 && (
            <div className="empty-state" data-testid="items-empty">
              No income or expenses for {monthLabel(visibleResult.month)}.
            </div>
          )}

          {visibleResult.invalid_date_entry_count > 0 && (
            <div className="message report-invalid-note" data-testid="items-invalid-dates" role="note">
              <span>i</span>
              {visibleResult.invalid_date_entry_count} posted{" "}
              {visibleResult.invalid_date_entry_count === 1 ? "entry" : "entries"} with invalid
              legacy dates {visibleResult.invalid_date_entry_count === 1 ? "is" : "are"} excluded
              because the period is unknown.
            </div>
          )}

          <p className="field-hint report-visibility-note">
            Names match exactly and case-sensitively; notes are not searched. Equal totals share a
            rank and sort by name. Accounts hidden from reports are excluded, while picker-only
            hidden accounts remain included. Transfers and opening balances are outside this report.
          </p>

          <div className="report-period-grid">
            <ItemTable
              kind="income"
              title="Income by item"
              items={visibleResult.income_items}
              total={visibleResult.current.income_minor}
              month={visibleResult.month}
            />
            <ItemTable
              kind="expense"
              title="Expenses by item"
              items={visibleResult.expense_items}
              total={visibleResult.current.expense_minor}
              month={visibleResult.month}
            />
          </div>
        </>
      )}
    </section>
  );
}
