import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "../../shared/api";
import { localDateValue } from "../../shared/date";
import { formatMinor } from "../../shared/money";
import type { BudgetState, DailySummaryReport } from "../../shared/types";

const MAX_ANCHOR = "9999-12-31";

type LoadState =
  | { anchor: string; status: "loading" }
  | { anchor: string; status: "ready" }
  | { anchor: string; status: "error"; message: string };

function summaryValue(state: LoadState, value: number | null): string {
  if (state.status === "loading") return "Pending…";
  if (state.status === "error") return "Unavailable";
  return value === null ? "—" : formatMinor(value);
}

function AverageCard({
  label,
  value,
  state,
  testId,
}: {
  label: string;
  value: number | null;
  state: LoadState;
  testId: string;
}) {
  return (
    <article className="daily-average-card">
      <p className="section-kicker">{label}</p>
      <strong data-testid={testId}>{summaryValue(state, value)}</strong>
      <span>Average expense per calendar day</span>
    </article>
  );
}

function DailyChart({ report }: { report: DailySummaryReport }) {
  const maximum = useMemo(
    () => Math.max(0, ...report.days.map((day) => day.expense_minor)),
    [report.days],
  );
  return (
    <div
      className="daily-chart"
      role="list"
      aria-label={`Daily expenses for the seven days ending ${report.anchor_date}`}
      data-testid="daily-chart"
    >
      {report.days.map((day) => {
        const height = maximum > 0 ? (day.expense_minor / maximum) * 100 : 0;
        const label = `${day.date}: ${formatMinor(day.expense_minor)} spent`;
        return (
          <div
            className="daily-chart-day"
            role="listitem"
            aria-label={label}
            key={day.date}
            data-testid={`daily-day-${day.date}`}
          >
            <span className="daily-chart-amount" data-testid={`daily-day-${day.date}-amount`}>
              {formatMinor(day.expense_minor)}
            </span>
            <div className="daily-chart-track" aria-hidden="true">
              <span className="daily-chart-bar" style={{ height: `${height}%` }} />
            </div>
            <time dateTime={day.date}>{day.date.slice(5)}</time>
          </div>
        );
      })}
    </div>
  );
}

export default function DailySummary({ appState }: { appState: BudgetState }) {
  const [anchorDate, setAnchorDate] = useState(() => localDateValue());
  const [result, setResult] = useState<DailySummaryReport | null>(null);
  const [state, setState] = useState<LoadState>({ anchor: anchorDate, status: "loading" });
  const requestSequence = useRef(0);
  const mounted = useRef(false);

  const load = useCallback(async () => {
    const request = ++requestSequence.current;
    const requestAnchor = anchorDate;
    setState({ anchor: requestAnchor, status: "loading" });
    try {
      const next = await api.dailySummary(requestAnchor);
      if (
        !mounted.current ||
        request !== requestSequence.current ||
        next.anchor_date !== requestAnchor
      ) {
        return;
      }
      setResult(next);
      setState({ anchor: requestAnchor, status: "ready" });
    } catch (reason) {
      if (!mounted.current || request !== requestSequence.current) return;
      setState({
        anchor: requestAnchor,
        status: "error",
        message: reason instanceof Error ? reason.message : "Could not load Daily Summary",
      });
    }
  }, [anchorDate]);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      requestSequence.current += 1;
    };
  }, []);

  useEffect(() => {
    void load();
  }, [appState, load]);

  function selectAnchor(nextAnchor: string) {
    if (!nextAnchor || nextAnchor === anchorDate) return;
    requestSequence.current += 1;
    setResult(null);
    setState({ anchor: nextAnchor, status: "loading" });
    setAnchorDate(nextAnchor);
  }

  const activeState = state.anchor === anchorDate ? state : { anchor: anchorDate, status: "loading" as const };
  const visibleResult =
    result?.anchor_date === anchorDate && activeState.status === "ready" ? result : null;

  return (
    <section
      className="panel daily-summary-panel"
      aria-labelledby="daily-summary-title"
      data-testid="daily-summary"
    >
      <div className="panel-heading daily-summary-heading">
        <div>
          <p className="section-kicker">The last seven calendar days</p>
          <h2 id="daily-summary-title">Daily Summary</h2>
          <p className="daily-summary-intro">
            Posted expenses by day, including days with no spending.
          </p>
        </div>
        <label className="daily-anchor-control">
          Summary date
          <input
            data-testid="daily-anchor"
            type="date"
            max={MAX_ANCHOR}
            value={anchorDate}
            onChange={(event) => selectAnchor(event.target.value)}
          />
        </label>
      </div>

      <div className="daily-average-grid">
        <AverageCard
          label="7-day average"
          value={visibleResult?.average_7_minor ?? null}
          state={activeState}
          testId="daily-average-7"
        />
        <AverageCard
          label="30-day average"
          value={visibleResult?.average_30_minor ?? null}
          state={activeState}
          testId="daily-average-30"
        />
      </div>

      {activeState.status === "loading" && (
        <div className="empty-state" data-testid="daily-loading" role="status">
          Loading Daily Summary…
        </div>
      )}

      {activeState.status === "error" && (
        <div className="empty-state report-error" data-testid="daily-error" role="alert">
          <h3>Could not load Daily Summary</h3>
          <p>{activeState.message}</p>
          <button className="secondary-button" type="button" onClick={() => void load()}>
            Try again
          </button>
        </div>
      )}

      {visibleResult && (
        <>
          {visibleResult.average_30_minor === 0 && (
            <div className="empty-state daily-empty" data-testid="daily-empty">
              No posted expenses in the 30 days ending {visibleResult.anchor_date}.
            </div>
          )}

          {visibleResult.invalid_date_entry_count > 0 && (
            <div className="message report-invalid-note" data-testid="daily-invalid-dates" role="note">
              <span>i</span>
              {visibleResult.invalid_date_entry_count} posted{" "}
              {visibleResult.invalid_date_entry_count === 1 ? "entry" : "entries"} with invalid
              legacy dates {visibleResult.invalid_date_entry_count === 1 ? "is" : "are"} excluded
              because the day is unknown.
            </div>
          )}

          <DailyChart report={visibleResult} />

          <p className="field-hint daily-policy-note">
            Averages use all 7 or 30 calendar days, including zero-spend days, and round to the
            nearest paise with exact halves rounded up. Transfers, opening balances, and accounts
            hidden from reports are excluded; picker-only hidden accounts remain included.
          </p>
        </>
      )}
    </section>
  );
}
