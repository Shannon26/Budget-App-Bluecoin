import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../../shared/api";
import { localDateValue } from "../../shared/date";
import { formatMinor } from "../../shared/money";
import type { BalanceSheetGroup, BalanceSheetReport } from "../../shared/types";

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
  report: BalanceSheetReport | null;
}) {
  const group = (key: string) => report?.groups.find((item) => item.key === key);
  const rows = [
    {
      key: "asset",
      label: "Assets",
      prior: group("asset")?.prior_subtotal_minor ?? null,
      current: group("asset")?.current_subtotal_minor ?? null,
    },
    {
      key: "liability",
      label: "Liabilities · signed",
      prior: group("liability")?.prior_subtotal_minor ?? null,
      current: group("liability")?.current_subtotal_minor ?? null,
    },
    {
      key: "unclassified",
      label: "Unclassified",
      prior: group("unclassified")?.prior_subtotal_minor ?? null,
      current: group("unclassified")?.current_subtotal_minor ?? null,
    },
    {
      key: "net",
      label: "Net worth",
      prior: report?.prior_net_worth_minor ?? null,
      current: report?.current_net_worth_minor ?? null,
    },
  ];
  return (
    <div className="report-summary-table-wrap">
      <table className="report-summary-table balance-summary-table">
        <caption className="sr-only">Prior and selected month-end Balance Sheet</caption>
        <thead>
          <tr>
            <th scope="col">Measure</th>
            <th scope="col" className="number-cell">
              Prior{report ? ` · ${report.prior_date}` : ""}
            </th>
            <th scope="col" className="number-cell">
              Selected{report ? ` · ${report.current_date}` : ""}
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row.key} className={row.key === "net" ? "balance-net-row" : ""}>
              <th scope="row">{row.label}</th>
              <td className="number-cell" data-testid={`balance-${row.key}-prior`}>
                {summaryValue(state, row.prior)}
              </td>
              <td className="number-cell" data-testid={`balance-${row.key}-current`}>
                {summaryValue(state, row.current)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function GroupTable({ group, report }: { group: BalanceSheetGroup; report: BalanceSheetReport }) {
  return (
    <section
      className="report-category-section balance-group-section"
      aria-labelledby={`balance-group-${group.key}`}
      data-testid={`balance-group-${group.key}`}
    >
      <div className="panel-heading">
        <div>
          <p className="section-kicker">Signed account balances</p>
          <h3 id={`balance-group-${group.key}`}>{group.label}</h3>
        </div>
      </div>
      <div className="report-table-wrap">
        <table className="report-table balance-account-table">
          <caption className="sr-only">
            {group.label} at {report.prior_date} and {report.current_date}
          </caption>
          <thead>
            <tr>
              <th scope="col">Account</th>
              <th scope="col" className="number-cell">Prior · {report.prior_date}</th>
              <th scope="col" className="number-cell">Selected · {report.current_date}</th>
            </tr>
          </thead>
          <tbody>
            {group.accounts.length === 0 ? (
              <tr>
                <td colSpan={3} className="report-item-empty">
                  No report-visible accounts in this group.
                </td>
              </tr>
            ) : (
              group.accounts.map((account) => (
                <tr
                  key={account.account_id}
                  data-testid={`balance-account-${account.account_id}`}
                >
                  <th scope="row">
                    <span className="balance-account-name">{account.name}</span>
                    <small className="balance-account-type">{account.account_type_label}</small>
                  </th>
                  <td
                    className="number-cell"
                    data-testid={`balance-account-${account.account_id}-prior`}
                  >
                    {formatMinor(account.prior_minor)}
                  </td>
                  <td
                    className="number-cell"
                    data-testid={`balance-account-${account.account_id}-current`}
                  >
                    {formatMinor(account.current_minor)}
                  </td>
                </tr>
              ))
            )}
            <tr className="balance-subtotal-row">
              <th scope="row">{group.label} subtotal</th>
              <td
                className="number-cell"
                data-testid={`balance-${group.key}-subtotal-prior`}
              >
                {formatMinor(group.prior_subtotal_minor)}
              </td>
              <td
                className="number-cell"
                data-testid={`balance-${group.key}-subtotal-current`}
              >
                {formatMinor(group.current_subtotal_minor)}
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>
  );
}

export default function BalanceSheet() {
  const [month, setMonth] = useState(() => localDateValue().slice(0, 7));
  const [result, setResult] = useState<BalanceSheetReport | null>(null);
  const [state, setState] = useState<LoadState>({ month, status: "loading" });
  const requestSequence = useRef(0);
  const mounted = useRef(false);

  const load = useCallback(async () => {
    const request = ++requestSequence.current;
    const requestMonth = month;
    setState({ month: requestMonth, status: "loading" });
    try {
      const next = await api.balanceSheet(requestMonth);
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
        message: reason instanceof Error ? reason.message : "Could not load Balance Sheet",
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
  const accountCount = visibleResult?.groups.reduce((count, group) => count + group.accounts.length, 0) ?? 0;

  return (
    <section className="reports-section">
      <div className="panel-heading ledger-heading report-heading">
        <div>
          <p className="section-kicker">What you own and owe</p>
          <h2>Balance Sheet</h2>
          <p className="report-intro">
            Cumulative signed account balances at the prior and selected calendar month-end.
          </p>
        </div>
        <label className="report-month-control">
          Report month
          <input
            data-testid="balance-month"
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
        <div className="empty-state" data-testid="balance-loading" role="status">
          Loading Balance Sheet…
        </div>
      )}

      {activeState.status === "error" && (
        <div className="empty-state report-error" data-testid="balance-error" role="alert">
          <h3>Could not load Balance Sheet</h3>
          <p>{activeState.message}</p>
          <button className="secondary-button" type="button" onClick={() => void load()}>
            Try again
          </button>
        </div>
      )}

      {visibleResult && (
        <>
          {accountCount === 0 && (
            <div className="empty-state" data-testid="balance-empty">
              No report-visible accounts are available for this comparison.
            </div>
          )}

          {visibleResult.invalid_date_entry_count > 0 && (
            <div className="message report-invalid-note" data-testid="balance-invalid-dates" role="note">
              <span>i</span>
              {visibleResult.invalid_date_entry_count} posted{" "}
              {visibleResult.invalid_date_entry_count === 1 ? "entry" : "entries"} with invalid
              legacy dates {visibleResult.invalid_date_entry_count === 1 ? "is" : "are"} excluded
              from both time-bound snapshots.
            </div>
          )}

          <p className="field-hint report-visibility-note">
            Balances stay signed: a negative liability is owed and a positive liability is account
            credit. Net worth equals assets plus signed liabilities plus unclassified balances.
            Report-hidden accounts are excluded; picker-only hidden accounts remain included.
          </p>
          <p className="field-hint report-visibility-note">
            Paisa period choice: each column includes valid posted income, expenses, and both transfer
            legs before the following month starts. A known opening date gates only the opening
            baseline; posted history is retained even if that metadata is corrected. Accounts use
            their current type/group because type history is not stored.
          </p>
          <p className="field-hint report-visibility-note" data-testid="balance-opening-policy">
            Historical limitation: {visibleResult.legacy_opening_account_count} account
            {visibleResult.legacy_opening_account_count === 1 ? " has" : "s have"} no opening date, so
            the baseline opening amount is shown in both snapshots.
          </p>

          <div className="report-period-grid balance-group-grid">
            {visibleResult.groups.map((group) => (
              <GroupTable key={group.key} group={group} report={visibleResult} />
            ))}
          </div>
        </>
      )}
    </section>
  );
}
