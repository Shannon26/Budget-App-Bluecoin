import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "../../shared/api";
import { localDateValue } from "../../shared/date";
import { formatMinor } from "../../shared/money";
import type {
  NetEarningsReport,
  ReportCategory,
} from "../../shared/types";

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

function shareLabel(amount: number, total: number): string {
  if (total === 0) return amount === 0 ? "0%" : "—";
  return `${Math.round((amount / total) * 100)}%`;
}

function summaryValue(state: LoadState, value: number | null): string {
  if (state.status === "loading") return "Pending…";
  if (state.status === "error") return "Unavailable";
  return value === null ? "—" : formatMinor(value);
}

function SummaryTable({
  state,
  report,
}: {
  state: LoadState;
  report: NetEarningsReport | null;
}) {
  const rows = [
    { tone: "income", label: "Income", key: "income_minor" },
    { tone: "expense", label: "Spending", key: "expense_minor" },
    { tone: "net", label: "Net earnings", key: "net_minor" },
  ] as const;
  return (
    <div className="report-summary-table-wrap">
      <table className="report-summary-table">
        <caption className="sr-only">Prior and selected month Net Earnings</caption>
        <thead>
          <tr>
            <th scope="col">Measure</th>
            <th scope="col" className="number-cell">
              Prior{report ? ` · ${monthLabel(report.prior_month)}` : ""}
            </th>
            <th scope="col" className="number-cell">
              Selected{report ? ` · ${monthLabel(report.month)}` : ""}
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row.tone} className={`report-summary-${row.tone}`}>
              <th scope="row">{row.label}</th>
              <td className="number-cell" data-testid={`report-${row.tone}-prior`}>
                {summaryValue(state, report ? report.prior[row.key] : null)}
              </td>
              <td className="number-cell" data-testid={`report-${row.tone}-current`}>
                {summaryValue(state, report ? report.current[row.key] : null)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function categoryDepth(categories: ReportCategory[]): Map<number, number> {
  const byId = new Map(
    categories
      .map((category) => [category.category_id, category] as const)
      .filter((entry): entry is [number, ReportCategory] => entry[0] !== null),
  );
  const depths = new Map<number, number>();
  for (const category of categories) {
    if (category.category_id === null) continue;
    let current: ReportCategory | undefined = category;
    let depth = 0;
    const seen = new Set<number>();
    while (current?.parent_id !== null && current?.parent_id !== undefined) {
      if (seen.has(current.parent_id)) {
        depth = 0;
        break;
      }
      seen.add(current.parent_id);
      depth += 1;
      current = byId.get(current.parent_id);
    }
    depths.set(category.category_id, depth);
  }
  return depths;
}

function CategoryTable({
  title,
  categories,
  report,
  period,
  kind,
}: {
  title: string;
  categories: ReportCategory[];
  report: NetEarningsReport;
  period: "prior" | "current";
  kind: "income" | "expense";
}) {
  const depths = useMemo(() => categoryDepth(categories), [categories]);
  const parentIds = useMemo(
    () => new Set(categories.map((category) => category.parent_id).filter((id) => id !== null)),
    [categories],
  );
  const sectionTotal = report[period][`${kind}_minor`];
  const rows = categories.filter(
    (category) => category.kind === kind || category.kind === "uncategorized",
  );

  return (
    <section className="report-category-section" aria-labelledby={`report-${kind}-title`}>
      <div className="panel-heading">
        <div>
          <p className="section-kicker">{monthLabel(report[period].start_date.slice(0, 7))}</p>
          <h3 id={`report-${kind}-title`}>{title}</h3>
        </div>
        <strong>{formatMinor(sectionTotal)}</strong>
      </div>
      <div className="report-table-wrap">
        <table className="report-table">
          <caption className="sr-only">
            {title} for {monthLabel(report[period].start_date.slice(0, 7))}
          </caption>
          <thead>
            <tr>
              <th scope="col">Category</th>
              <th scope="col" className="number-cell">Amount</th>
              <th scope="col" className="number-cell">Contribution</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((category) => {
              const values = category[period][kind];
              const id = category.category_id ?? "uncategorized";
              const depth = category.category_id === null ? 0 : (depths.get(category.category_id) ?? 0);
              const isParent = category.category_id !== null && parentIds.has(category.category_id);
              return (
                <tr
                  key={id}
                  className={isParent ? "report-parent-row" : ""}
                  data-testid={`report-${kind}-${period}-category-${id}`}
                >
                  <th scope="row">
                    <span
                      className="report-category-name"
                      style={{ paddingLeft: `${depth * 16}px` }}
                    >
                      <span className="report-category-icon" style={{ background: category.color }}>
                        {category.icon}
                      </span>
                      <span>
                        {category.name}
                        {category.archived_at && <small className="report-archived">Archived</small>}
                        {isParent && <small className="report-direct">Direct {formatMinor(values.direct_minor)}</small>}
                      </span>
                    </span>
                  </th>
                  <td
                    className="number-cell"
                    data-testid={`report-${kind}-${period}-category-${id}-amount`}
                  >
                    {formatMinor(values.rollup_minor)}
                  </td>
                  <td className="number-cell">{shareLabel(values.rollup_minor, sectionTotal)}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </section>
  );
}

export default function NetEarnings() {
  const [month, setMonth] = useState(() => localDateValue().slice(0, 7));
  const [result, setResult] = useState<NetEarningsReport | null>(null);
  const [state, setState] = useState<LoadState>({ month, status: "loading" });
  const requestSequence = useRef(0);
  const mounted = useRef(false);

  const load = useCallback(async () => {
    const request = ++requestSequence.current;
    const requestMonth = month;
    setState({ month: requestMonth, status: "loading" });
    try {
      const next = await api.netEarnings(requestMonth);
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
        message: reason instanceof Error ? reason.message : "Could not load Net Earnings",
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
          <p className="section-kicker">Income, spending, and what remains</p>
          <h2>Net Earnings</h2>
          <p className="report-intro">
            Compare the selected calendar month with the month immediately before it.
          </p>
        </div>
        <label className="report-month-control">
          Report month
          <input
            data-testid="report-month"
            type="month"
            min={MIN_MONTH}
            max={MAX_MONTH}
            value={month}
            onChange={(event) => selectMonth(event.target.value)}
          />
        </label>
      </div>

      <p className="field-hint report-range-hint">
        Supported comparison months: {MIN_MONTH} through {MAX_MONTH}.
      </p>

      <SummaryTable state={activeState} report={visibleResult} />

      {activeState.status === "loading" && (
        <div className="empty-state" data-testid="report-loading" role="status">
          Loading Net Earnings…
        </div>
      )}

      {activeState.status === "error" && (
        <div className="empty-state report-error" data-testid="report-error" role="alert">
          <h3>Could not load Net Earnings</h3>
          <p>{activeState.message}</p>
          <button className="secondary-button" type="button" onClick={() => void load()}>
            Try again
          </button>
        </div>
      )}

      {visibleResult && (
        <>
          <div className="report-counts">
            <span>{visibleResult.current.income_count} income entries this month</span>
            <span>{visibleResult.current.expense_count} expense entries this month</span>
          </div>

          {visibleResult.invalid_date_entry_count > 0 && (
            <div className="message report-invalid-note" data-testid="report-invalid-dates" role="note">
              <span>i</span>
              {visibleResult.invalid_date_entry_count} posted{" "}
              {visibleResult.invalid_date_entry_count === 1 ? "entry" : "entries"} with invalid
              legacy dates {visibleResult.invalid_date_entry_count === 1 ? "is" : "are"} excluded
              because the period is unknown.
            </div>
          )}

          <p className="field-hint report-visibility-note">
            Accounts hidden from reports are excluded. Picker-only hidden accounts remain included.
            Transfers and opening balances do not affect this report.
          </p>

          <div className="report-period-grid">
            <CategoryTable
              title="Income by category"
              kind="income"
              period="prior"
              categories={visibleResult.categories}
              report={visibleResult}
            />
            <CategoryTable
              title="Income by category"
              kind="income"
              period="current"
              categories={visibleResult.categories}
              report={visibleResult}
            />
            <CategoryTable
              title="Spending by category"
              kind="expense"
              period="prior"
              categories={visibleResult.categories}
              report={visibleResult}
            />
            <CategoryTable
              title="Spending by category"
              kind="expense"
              period="current"
              categories={visibleResult.categories}
              report={visibleResult}
            />
          </div>
        </>
      )}
    </section>
  );
}
