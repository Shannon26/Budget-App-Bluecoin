import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../../shared/api";
import { localDateValue } from "../../shared/date";
import { formatMinor } from "../../shared/money";
import type { CashFlowTrendReport } from "../../shared/types";

export default function CashFlowTrend() {
  const [month, setMonth] = useState(() => localDateValue().slice(0, 7));
  const [months, setMonths] = useState(6);
  const [result, setResult] = useState<CashFlowTrendReport | null>(null);
  const [status, setStatus] = useState<"loading" | "ready" | "error">("loading");
  const [error, setError] = useState("");
  const sequence = useRef(0);

  const load = useCallback(async () => {
    const request = ++sequence.current;
    setStatus("loading");
    setResult(null);
    try {
      const report = await api.cashFlowTrend(month, months);
      if (request !== sequence.current) return;
      if (report.month !== month || report.points.length !== months) {
        throw new Error("The report did not match the selected period");
      }
      setResult(report);
      setStatus("ready");
    } catch (reason) {
      if (request !== sequence.current) return;
      setError(reason instanceof Error ? reason.message : "Could not load the report");
      setStatus("error");
    }
  }, [month, months]);

  useEffect(() => {
    void load();
    return () => { sequence.current += 1; };
  }, [load]);

  const prior = result?.points.at(-2);
  const current = result?.points.at(-1);
  return (
    <section className="reports-section">
      <div className="panel-heading ledger-heading report-heading">
        <div>
          <h2>Cash Flow &amp; Net Worth</h2>
          <p className="report-intro">Compare income and spending and follow month-end net worth.</p>
        </div>
        <label className="report-month-control">
          Ending month
          <input type="month" min="0001-02" max="9999-11" value={month}
            onChange={(event) => {
              if (event.target.value && event.target.value !== month) {
                sequence.current += 1;
                setResult(null);
                setStatus("loading");
                setMonth(event.target.value);
              }
            }} />
        </label>
        <label className="report-month-control">
          Months
          <select value={months} onChange={(event) => {
            sequence.current += 1;
            setResult(null);
            setStatus("loading");
            setMonths(Number(event.target.value));
          }}>
            {[2, 3, 6, 12, 24].map((value) => <option key={value} value={value}>{value}</option>)}
          </select>
        </label>
      </div>
      {status === "loading" && <div className="empty-state" role="status">Loading report…</div>}
      {status === "error" && <div className="empty-state report-error" role="alert">
        <p>{error}</p><button type="button" className="secondary-button" onClick={() => void load()}>Try again</button>
      </div>}
      {result && status === "ready" && prior && current && <>
        <div className="report-summary-table-wrap">
          <table className="report-summary-table">
            <caption>Cash flow comparison</caption>
            <thead><tr><th scope="col">Measure</th><th scope="col">{prior.month}</th><th scope="col">{current.month}</th></tr></thead>
            <tbody>
              <tr><th scope="row">Income</th><td>{formatMinor(prior.cash_flow.income_minor)}</td><td>{formatMinor(current.cash_flow.income_minor)}</td></tr>
              <tr><th scope="row">Spending</th><td>{formatMinor(prior.cash_flow.expense_minor)}</td><td>{formatMinor(current.cash_flow.expense_minor)}</td></tr>
              <tr><th scope="row">Net cash flow</th><td>{formatMinor(prior.cash_flow.net_minor)}</td><td>{formatMinor(current.cash_flow.net_minor)}</td></tr>
            </tbody>
          </table>
        </div>
        <div className="report-table-wrap">
          <table className="report-table">
            <caption>Monthly net worth and cash flow</caption>
            <thead><tr><th scope="col">Month end</th><th scope="col">Income</th><th scope="col">Spending</th><th scope="col">Net cash flow</th><th scope="col">Net worth</th></tr></thead>
            <tbody>{result.points.map((point) => <tr key={point.month}>
              <th scope="row">{point.cutoff_date}</th>
              <td>{formatMinor(point.cash_flow.income_minor)}</td>
              <td>{formatMinor(point.cash_flow.expense_minor)}</td>
              <td>{formatMinor(point.cash_flow.net_minor)}</td>
              <td>{formatMinor(point.net_worth_minor)}</td>
            </tr>)}</tbody>
          </table>
        </div>
        {result.invalid_date_entry_count > 0 && <p role="note">{result.invalid_date_entry_count} entries with invalid dates were excluded.</p>}
        <p className="field-hint">Historical values use current account settings. {result.legacy_opening_account_count} accounts have no opening date and include their opening balance throughout the trend.</p>
      </>}
    </section>
  );
}
