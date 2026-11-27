import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../../shared/api";
import { localDateValue } from "../../shared/date";
import { formatMinor } from "../../shared/money";
import type { LabelReport, LabelReportRow } from "../../shared/types";

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
  report: LabelReport | null;
}) {
  const rows = [
    { tone: "income", label: "Income", key: "income_minor" },
    { tone: "expense", label: "Spending", key: "expense_minor" },
    { tone: "net", label: "Net earnings", key: "net_minor" },
  ] as const;
  return (
    <div className="report-summary-table-wrap">
      <table className="report-summary-table">
        <caption className="sr-only">Selected month label allocation totals</caption>
        <thead>
          <tr>
            <th scope="col">Measure</th>
            <th scope="col" className="number-cell">
              Selected{report ? ` · ${monthLabel(report.month)}` : ""}
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row.tone} className={`report-summary-${row.tone}`}>
              <th scope="row">{row.label}</th>
              <td className="number-cell" data-testid={`label-report-${row.tone}-total`}>
                {summaryValue(state, report ? report.current[row.key] : null)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function LabelTable({ report }: { report: LabelReport }) {
  function rowId(row: LabelReportRow): string {
    return row.is_no_labels ? "no-labels" : `label-${row.label_id ?? "unknown"}`;
  }

  return (
    <section className="report-category-section label-report-section" aria-labelledby="label-report-table-title">
      <div className="panel-heading">
        <div>
          <p className="section-kicker">{monthLabel(report.month)}</p>
          <h3 id="label-report-table-title">Totals by label</h3>
        </div>
      </div>
      <div className="report-table-wrap">
        <table className="report-table label-report-table">
          <caption className="sr-only">
            Income, expenses, and net earnings allocated by label for {monthLabel(report.month)}
          </caption>
          <thead>
            <tr>
              <th scope="col">Label</th>
              <th scope="col" className="number-cell">Income</th>
              <th scope="col" className="number-cell">Expenses</th>
              <th scope="col" className="number-cell">Net</th>
            </tr>
          </thead>
          <tbody>
            {report.rows.map((row) => {
              const id = rowId(row);
              return (
                <tr
                  key={id}
                  data-testid={`label-report-${id}`}
                  data-label-id={row.label_id ?? ""}
                  data-no-labels={row.is_no_labels}
                >
                  <th scope="row">
                    {row.name}
                    {row.is_no_labels && (
                      <small className="label-report-synthetic">Synthetic · no assigned labels</small>
                    )}
                  </th>
                  <td className="number-cell" data-testid={`label-report-${id}-income`}>
                    {formatMinor(row.income_minor)}
                  </td>
                  <td className="number-cell" data-testid={`label-report-${id}-expense`}>
                    {formatMinor(row.expense_minor)}
                  </td>
                  <td className="number-cell" data-testid={`label-report-${id}-net`}>
                    {formatMinor(row.net_minor)}
                  </td>
                </tr>
              );
            })}
          </tbody>
          <tfoot>
            <tr className="balance-subtotal-row">
              <th scope="row">Allocated total</th>
              <td className="number-cell" data-testid="label-report-allocated-income">
                {formatMinor(report.current.income_minor)}
              </td>
              <td className="number-cell" data-testid="label-report-allocated-expense">
                {formatMinor(report.current.expense_minor)}
              </td>
              <td className="number-cell" data-testid="label-report-allocated-net">
                {formatMinor(report.current.net_minor)}
              </td>
            </tr>
          </tfoot>
        </table>
      </div>
    </section>
  );
}

export default function LabelReport() {
  const [month, setMonth] = useState(() => localDateValue().slice(0, 7));
  const [result, setResult] = useState<LabelReport | null>(null);
  const [state, setState] = useState<LoadState>({ month, status: "loading" });
  const requestSequence = useRef(0);
  const mounted = useRef(false);

  const load = useCallback(async () => {
    const request = ++requestSequence.current;
    const requestMonth = month;
    setState({ month: requestMonth, status: "loading" });
    try {
      const next = await api.labelReport(requestMonth);
      if (!mounted.current || request !== requestSequence.current || next.month !== requestMonth) {
        return;
      }
      setResult(next);
      setState({ month: requestMonth, status: "ready" });
    } catch (reason) {
      if (!mounted.current || request !== requestSequence.current) return;
      setResult(null);
      setState({
        month: requestMonth,
        status: "error",
        message: reason instanceof Error ? reason.message : "Could not load Label Report",
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
  const entryCount = visibleResult
    ? visibleResult.current.income_count + visibleResult.current.expense_count
    : 0;

  return (
    <section className="reports-section">
      <div className="panel-heading ledger-heading report-heading">
        <div>
          <p className="section-kicker">Income and expenses, allocated once</p>
          <h2>Label Report</h2>
          <p className="report-intro">
            See how each posted amount is shared across your current labels for a calendar month.
          </p>
        </div>
        <label className="report-month-control">
          Report month
          <input
            data-testid="label-report-month"
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

      <SummaryTable state={activeState} report={visibleResult} />

      {activeState.status === "loading" && (
        <div className="empty-state" data-testid="label-report-loading" role="status">
          Loading Label Report…
        </div>
      )}

      {activeState.status === "error" && (
        <div className="empty-state report-error" data-testid="label-report-error" role="alert">
          <h3>Could not load Label Report</h3>
          <p>{activeState.message}</p>
          <button className="secondary-button" type="button" onClick={() => void load()}>
            Try again
          </button>
        </div>
      )}

      {visibleResult && (
        <>
          <div className="report-counts">
            <span data-testid="label-report-income-count">
              {visibleResult.current.income_count} income entries this month
            </span>
            <span data-testid="label-report-expense-count">
              {visibleResult.current.expense_count} expense entries this month
            </span>
          </div>

          {entryCount === 0 && (
            <div className="empty-state" data-testid="label-report-empty">
              No report-visible posted income or expenses for {monthLabel(visibleResult.month)}.
            </div>
          )}

          {visibleResult.invalid_date_entry_count > 0 && (
            <div className="message report-invalid-note" data-testid="label-report-invalid-dates" role="note">
              <span>i</span>
              {visibleResult.invalid_date_entry_count} posted{" "}
              {visibleResult.invalid_date_entry_count === 1 ? "entry" : "entries"} with invalid
              legacy dates {visibleResult.invalid_date_entry_count === 1 ? "is" : "are"} excluded
              because the period is unknown.
            </div>
          )}

          <LabelTable report={visibleResult} />

          <section className="card-rules label-report-rules" data-testid="label-report-allocation-rules">
            <h3>Paisa allocation rules</h3>
            <ul>
              <li>
                Each eligible posted income or expense contributes its positive amount exactly once
                across the label rows and No Labels. Income and expense row sums therefore equal the
                selected month’s Net Earnings totals. Expenses remain positive magnitudes.
              </li>
              <li>
                One distinct label receives the full amount. An entry with no current labels puts the
                full amount in No Labels.
              </li>
              <li>
                Multiple distinct labels split each amount equally in whole paise. Any leftover paise
                go one each to the first labels by ascending stable label ID, so renaming a label does
                not change its allocation.
              </li>
              <li>
                No Labels is a synthetic row, separate from any user label with that name. All current
                labels appear, including labels with zero activity.
              </li>
              <li>
                Accounts hidden from reports are excluded; picker-only hidden accounts remain included.
                Transfers, opening balances, and invalid legacy dates do not enter these totals.
              </li>
            </ul>
          </section>
        </>
      )}
    </section>
  );
}
