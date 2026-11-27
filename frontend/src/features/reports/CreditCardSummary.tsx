import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "../../shared/api";
import { localDateValue } from "../../shared/date";
import { formatMinor } from "../../shared/money";
import type {
  Account,
  CreditCardReport,
  CreditCardStatement,
  CreditCardStatementEvent,
} from "../../shared/types";

const MIN_ANCHOR = "0001-01-01";
const MAX_ANCHOR = "9999-10-31";

type LoadState =
  | { key: string; status: "loading" }
  | { key: string; status: "ready" }
  | { key: string; status: "error"; message: string };

function balanceState(minor: number): string {
  if (minor < 0) return "Amount owed";
  if (minor > 0) return "Credit balance";
  return "Settled";
}

function moneyValue(state: LoadState, value: number | null): string {
  if (state.status === "loading") return "Pending…";
  if (state.status === "error") return "Unavailable";
  return value === null ? "—" : formatMinor(value);
}

function eventType(event: CreditCardStatementEvent): string {
  if (event.entry_kind === "expense") return "Charge";
  if (event.entry_kind === "income") return "Income / refund credit";
  if (event.entry_kind === "transfer_in") return "Card-directed payment";
  if (event.entry_kind === "transfer_out") return "Advance / withdrawal";
  return event.entry_kind;
}

function ActivityTable({
  events,
  testId,
  emptyText,
}: {
  events: CreditCardStatementEvent[];
  testId: string;
  emptyText: string;
}) {
  return (
    <div className="report-table-wrap">
      <table className="report-table card-event-table">
        <caption className="sr-only">{testId.replaceAll("-", " ")}</caption>
        <thead>
          <tr>
            <th scope="col">Date</th>
            <th scope="col">Entry</th>
            <th scope="col">Type</th>
            <th scope="col" className="number-cell">Amount</th>
          </tr>
        </thead>
        <tbody data-testid={testId}>
          {events.length === 0 ? (
            <tr>
              <td colSpan={4} className="report-item-empty">{emptyText}</td>
            </tr>
          ) : (
            events.map((event) => (
              <tr key={event.entry_id} data-testid={`${testId}-${event.entry_id}`}>
                <td>{event.occurred_at.slice(0, 10)}</td>
                <th scope="row">{event.name}</th>
                <td>{eventType(event)}</td>
                <td className="number-cell">{formatMinor(event.amount_minor)}</td>
              </tr>
            ))
          )}
        </tbody>
      </table>
    </div>
  );
}

function PaymentTable({
  payments,
  testId = "card-payment-events",
}: {
  payments: CreditCardStatementEvent[];
  testId?: string;
}) {
  return (
    <div className="report-table-wrap">
      <table className="report-table card-event-table">
        <caption className="sr-only">Card-directed incoming transfer payments</caption>
        <thead>
          <tr>
            <th scope="col">Date</th>
            <th scope="col">Payment</th>
            <th scope="col">Counterpart</th>
            <th scope="col" className="number-cell">Amount</th>
          </tr>
        </thead>
        <tbody data-testid={testId}>
          {payments.length === 0 ? (
            <tr>
              <td colSpan={4} className="report-item-empty">No incoming card-directed transfer payments are recorded in this window.</td>
            </tr>
          ) : (
            payments.map((payment) => (
              <tr key={payment.entry_id} data-testid={`${testId}-${payment.entry_id}`}>
                <td>{payment.occurred_at.slice(0, 10)}</td>
                <th scope="row">{payment.name}</th>
                <td>{payment.counterpart_account_name ?? "—"}</td>
                <td className="number-cell">{formatMinor(payment.amount_minor)}</td>
              </tr>
            ))
          )}
        </tbody>
      </table>
    </div>
  );
}

function StatementDetails({ statement }: { statement: CreditCardStatement }) {
  const outstanding = statement.owed_at_cutoff_minor > 0
    ? statement.owed_at_cutoff_minor
    : statement.credit_at_cutoff_minor;
  return (
    <>
      <div className="report-summary-table-wrap">
        <table className="report-summary-table card-statement-table">
          <caption className="sr-only">Selected credit-card statement period</caption>
          <thead>
            <tr>
              <th scope="col">Measure</th>
              <th scope="col" className="number-cell">Ledger-derived value</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <th scope="row">Statement period</th>
              <td className="number-cell card-date-cell" data-testid="card-statement-period">
                {statement.start_date} through {statement.cutoff_date} (inclusive)
              </td>
            </tr>
            <tr>
              <th scope="row">Opening baseline before period · signed</th>
              <td className="number-cell" data-testid="card-statement-baseline-before">
                {formatMinor(statement.opening_baseline_before_period_minor)}
              </td>
            </tr>
            <tr>
              <th scope="row">Balance before statement period · signed</th>
              <td className="number-cell" data-testid="card-statement-opening-balance">
                {formatMinor(statement.balance_before_period_minor)}
              </td>
            </tr>
            <tr>
              <th scope="row">Opening baseline activated inside period · signed</th>
              <td className="number-cell" data-testid="card-statement-baseline-inside">
                {formatMinor(statement.opening_baseline_inside_period_minor)}
              </td>
            </tr>
            <tr>
              <th scope="row">Statement balance at cutoff · signed</th>
              <td className="number-cell" data-testid="card-statement-balance">
                {formatMinor(statement.balance_at_cutoff_minor)}
              </td>
            </tr>
            <tr>
              <th scope="row">State at cutoff</th>
              <td data-testid="card-statement-state">{balanceState(statement.balance_at_cutoff_minor)}</td>
            </tr>
            <tr>
              <th scope="row">Outstanding / credit at cutoff</th>
              <td className="number-cell" data-testid="card-statement-outstanding">
                {formatMinor(outstanding)}
              </td>
            </tr>
            <tr>
              <th scope="row">Charges in statement period</th>
              <td className="number-cell" data-testid="card-statement-charges">
                {formatMinor(statement.charge_minor)}
              </td>
            </tr>
            <tr>
              <th scope="row">Advances / withdrawals in statement period</th>
              <td className="number-cell" data-testid="card-statement-advances">
                {formatMinor(statement.statement_advance_minor)}
              </td>
            </tr>
            <tr>
              <th scope="row">Non-transfer income / refund credits in statement</th>
              <td className="number-cell" data-testid="card-statement-non-transfer-credits">
                {formatMinor(statement.non_transfer_credit_minor)}
              </td>
            </tr>
            <tr>
              <th scope="row">Card-directed payments in statement period</th>
              <td className="number-cell" data-testid="card-statement-payments">
                {formatMinor(statement.statement_payment_minor)}
              </td>
            </tr>
            <tr>
              <th scope="row">Due date · Paisa convention</th>
              <td data-testid="card-due-date">{statement.due_date}</td>
            </tr>
            <tr>
              <th scope="row">Post-cutoff payment date window</th>
              <td className="card-date-cell" data-testid="card-payment-window">
                {statement.payment_start_date} through {statement.due_date} (inclusive)
              </td>
            </tr>
            <tr>
              <th scope="row">Card-directed payments after cutoff</th>
              <td className="number-cell" data-testid="card-payments">
                {formatMinor(statement.post_cutoff_payment_minor)}
              </td>
            </tr>
            <tr>
              <th scope="row">Charges after cutoff · not in statement</th>
              <td className="number-cell" data-testid="card-post-cutoff-charges-total">
                {formatMinor(statement.post_cutoff_charge_minor)}
              </td>
            </tr>
            <tr>
              <th scope="row">Advances / withdrawals after cutoff</th>
              <td className="number-cell" data-testid="card-post-cutoff-advances-total">
                {formatMinor(statement.post_cutoff_advance_minor)}
              </td>
            </tr>
            <tr>
              <th scope="row">Non-transfer income / refund credits after cutoff</th>
              <td className="number-cell" data-testid="card-post-cutoff-non-transfer-credits-total">
                {formatMinor(statement.post_cutoff_non_transfer_credit_minor)}
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      {statement.invalid_date_entry_count > 0 && (
        <div className="message report-invalid-note" data-testid="card-invalid-dates" role="note">
          <span>i</span>
          {statement.invalid_date_entry_count} card {statement.invalid_date_entry_count === 1 ? "entry has" : "entries have"}
          {" "}an invalid legacy date and {statement.invalid_date_entry_count === 1 ? "is" : "are"} excluded from this statement,
          but still included in the current ledger balance.
        </div>
      )}
      {statement.legacy_opening_balance && (
        <p className="field-hint report-visibility-note" data-testid="card-opening-policy">
          This card has no opening date, so its opening balance is included as the historical baseline.
        </p>
      )}

      <div className="card-statement-grid">
        <section className="report-item-section" aria-labelledby="card-statement-activity-heading">
          <div className="panel-heading">
            <div>
              <p className="section-kicker">Cutoff period</p>
              <h3 id="card-statement-activity-heading">Charges, advances, and non-transfer credits</h3>
            </div>
          </div>
          <ActivityTable
            events={statement.statement_activities}
            testId="card-statement-activities"
            emptyText="No charges, advances, or non-transfer credits fall in this statement period."
          />
        </section>
        <section className="report-item-section" aria-labelledby="card-statement-payments-heading">
          <div className="panel-heading">
            <div>
              <p className="section-kicker">Included in cutoff balance</p>
              <h3 id="card-statement-payments-heading">Card-directed payments in statement</h3>
            </div>
          </div>
          <PaymentTable payments={statement.statement_payments} testId="card-statement-payment-events" />
        </section>
        <section className="report-item-section" aria-labelledby="card-post-payments-heading">
          <div className="panel-heading">
            <div>
              <p className="section-kicker">After cutoff through due date</p>
              <h3 id="card-post-payments-heading">Card-directed payments after cutoff</h3>
            </div>
          </div>
          <PaymentTable payments={statement.post_cutoff_payments} />
        </section>
        <section className="report-item-section" aria-labelledby="card-post-activity-heading">
          <div className="panel-heading">
            <div>
              <p className="section-kicker">Separate from the cutoff statement</p>
              <h3 id="card-post-activity-heading">Activity after cutoff</h3>
            </div>
          </div>
          <h4 className="card-event-heading">Charges after cutoff</h4>
          <ActivityTable
            events={statement.post_cutoff_charges}
            testId="card-post-cutoff-charges"
            emptyText="No charges are recorded after cutoff through the due date."
          />
          <h4 className="card-event-heading">Advances / withdrawals after cutoff</h4>
          <ActivityTable
            events={statement.post_cutoff_advances}
            testId="card-post-cutoff-advances"
            emptyText="No card-to-other-account advances are recorded in this window."
          />
          <h4 className="card-event-heading">Non-transfer income / refund credits after cutoff</h4>
          <ActivityTable
            events={statement.post_cutoff_non_transfer_credits}
            testId="card-post-cutoff-credits"
            emptyText="No non-transfer income or refund credits are recorded in this window."
          />
        </section>
      </div>
    </>
  );
}

export default function CreditCardSummary({ accounts }: { accounts: Account[] }) {
  const cards = useMemo(
    () => accounts.filter((account) => account.account_type === "credit_card" && !account.hide_from_reports),
    [accounts],
  );
  const [selectedAccountId, setSelectedAccountId] = useState("");
  const [anchorDate, setAnchorDate] = useState(() => localDateValue());
  const [result, setResult] = useState<CreditCardReport | null>(null);
  const selectionKey = `${selectedAccountId}:${anchorDate}`;
  const [state, setState] = useState<LoadState>({ key: selectionKey, status: "loading" });
  const requestSequence = useRef(0);
  const mounted = useRef(false);

  useEffect(() => {
    if (selectedAccountId && !cards.some((card) => String(card.id) === selectedAccountId)) {
      requestSequence.current += 1;
      setResult(null);
      setSelectedAccountId(cards[0] ? String(cards[0].id) : "");
    } else if (!selectedAccountId && cards.length > 0) {
      setSelectedAccountId(String(cards[0].id));
    }
  }, [cards, selectedAccountId]);

  const load = useCallback(async () => {
    if (!selectedAccountId) {
      setResult(null);
      setState({ key: selectionKey, status: "ready" });
      return;
    }
    const request = ++requestSequence.current;
    setState({ key: selectionKey, status: "loading" });
    try {
      const next = await api.creditCard(Number(selectedAccountId), anchorDate);
      if (
        !mounted.current ||
        request !== requestSequence.current ||
        next.account_id !== Number(selectedAccountId) ||
        next.anchor_date !== anchorDate
      ) {
        return;
      }
      setResult(next);
      setState({ key: selectionKey, status: "ready" });
    } catch (reason) {
      if (!mounted.current || request !== requestSequence.current) return;
      setResult(null);
      setState({
        key: selectionKey,
        status: "error",
        message: reason instanceof Error ? reason.message : "Could not load Card Summary",
      });
    }
  }, [anchorDate, selectedAccountId, selectionKey]);

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

  function selectAccount(nextId: string) {
    if (!nextId || nextId === selectedAccountId) return;
    requestSequence.current += 1;
    setResult(null);
    setState({ key: `${nextId}:${anchorDate}`, status: "loading" });
    setSelectedAccountId(nextId);
  }

  function selectAnchorDate(nextDate: string) {
    if (!nextDate || nextDate === anchorDate) return;
    requestSequence.current += 1;
    setResult(null);
    setState({ key: `${selectedAccountId}:${nextDate}`, status: "loading" });
    setAnchorDate(nextDate);
  }

  const activeState = state.key === selectionKey ? state : { key: selectionKey, status: "loading" as const };
  const visibleResult = activeState.status === "ready" && result?.account_id === Number(selectedAccountId) && result.anchor_date === anchorDate
    ? result
    : null;

  return (
    <section className="reports-section">
      <div className="panel-heading ledger-heading report-heading">
        <div>
          <p className="section-kicker">What the card ledger shows</p>
          <h2>Card Summary</h2>
          <p className="report-intro">
            Signed current balances and a ledger-derived cutoff statement. This is not an issuer-provided bill or minimum due.
          </p>
        </div>
        <div className="card-report-controls">
          <label className="report-month-control">
            Credit card
            <select
              data-testid="card-account"
              value={selectedAccountId}
              disabled={cards.length === 0}
              onChange={(event) => selectAccount(event.target.value)}
            >
              {cards.length === 0 && <option value="">No report-visible credit cards</option>}
              {cards.map((card) => (
                <option key={card.id} value={card.id}>{card.name}</option>
              ))}
            </select>
          </label>
          <label className="report-month-control">
            Cutoff-date anchor
            <input
              data-testid="card-anchor-date"
              type="date"
              min={MIN_ANCHOR}
              max={MAX_ANCHOR}
              value={anchorDate}
              onChange={(event) => selectAnchorDate(event.target.value)}
            />
          </label>
        </div>
      </div>

      {cards.length === 0 ? (
        <div className="empty-state" data-testid="card-empty">
          No report-visible credit cards are available. Add or retype a Credit Card account in Overview.
        </div>
      ) : (
        <>
          <div className="report-summary-table-wrap">
            <table className="report-summary-table card-overview-table">
              <caption className="sr-only">Report-visible credit-card current balances</caption>
              <thead>
                <tr>
                  <th scope="col">Card</th>
                  <th scope="col" className="number-cell">Current signed balance</th>
                  <th scope="col">State</th>
                  <th scope="col" className="number-cell">Credit limit</th>
                </tr>
              </thead>
              <tbody>
                {cards.map((card) => (
                  <tr key={card.id} data-testid={`card-summary-account-${card.id}`}>
                    <th scope="row">{card.name}</th>
                    <td className="number-cell" data-testid={`card-summary-balance-${card.id}`}>
                      {formatMinor(card.balance_minor)}
                    </td>
                    <td>{balanceState(card.balance_minor)}</td>
                    <td className="number-cell">{card.credit_limit_minor === null ? "—" : formatMinor(card.credit_limit_minor)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {activeState.status === "loading" && (
            <div className="empty-state" data-testid="card-loading" role="status">Loading Card Summary…</div>
          )}
          {activeState.status === "error" && (
            <div className="empty-state report-error" data-testid="card-error" role="alert">
              <h3>Could not load Card Summary</h3>
              <p>{activeState.message}</p>
              <button className="secondary-button" type="button" onClick={() => void load()}>Try again</button>
            </div>
          )}

          {visibleResult && (
            <>
              <div className="panel-heading card-selected-heading">
                <div>
                  <p className="section-kicker">Selected card</p>
                  <h3 data-testid="card-report-name">{visibleResult.account_name}</h3>
                </div>
              </div>
              <div className="report-summary-table-wrap">
                <table className="report-summary-table card-current-table">
                  <caption className="sr-only">Selected card current ledger figures</caption>
                  <thead>
                    <tr>
                      <th scope="col">Measure</th>
                      <th scope="col" className="number-cell">Current value</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr>
                      <th scope="row">Current ledger balance · signed</th>
                      <td className="number-cell" data-testid="card-current-balance">
                        {moneyValue(activeState, visibleResult.current_balance_minor)}
                      </td>
                    </tr>
                    <tr>
                      <th scope="row">Current state</th>
                      <td data-testid="card-current-state">{balanceState(visibleResult.current_balance_minor)}</td>
                    </tr>
                    <tr>
                      <th scope="row">Current amount owed</th>
                      <td className="number-cell" data-testid="card-current-owed">
                        {formatMinor(visibleResult.current_owed_minor)}
                      </td>
                    </tr>
                    <tr>
                      <th scope="row">Current credit balance</th>
                      <td className="number-cell" data-testid="card-current-credit">
                        {formatMinor(visibleResult.current_credit_minor)}
                      </td>
                    </tr>
                    <tr>
                      <th scope="row">Credit limit</th>
                      <td className="number-cell" data-testid="card-credit-limit">
                        {moneyValue(activeState, visibleResult.credit_limit_minor)}
                      </td>
                    </tr>
                    <tr>
                      <th scope="row">Available limit · ledger-based</th>
                      <td className="number-cell" data-testid="card-available-credit">
                        {moneyValue(activeState, visibleResult.available_credit_minor)}
                      </td>
                    </tr>
                  </tbody>
                </table>
              </div>

              {visibleResult.statement ? (
                <StatementDetails statement={visibleResult.statement} />
              ) : (
                <div className="empty-state" data-testid="card-statement-unavailable" role="note">
                  Statement unavailable: set both Cutoff day and Due day in Overview → Edit account.
                  {" "}Missing: {visibleResult.missing_settings.join(", ")}.
                </div>
              )}

              <div className="card-rules" data-testid="card-calculation-rules">
                <h3>Paisa calculation rules</h3>
                <ul>
                  <li>The statement starts the day after the previous clamped cutoff and includes the selected cutoff day. The due date is the first clamped due-day strictly after cutoff.</li>
                  <li>Canonical signed delta: income and card transfer_in add to the balance; expenses and card transfer_out subtract from it.</li>
                  <li>A bank/asset-account → card transfer is the card's incoming transfer_in leg: it reduces debt and is a card-directed payment. A card → bank/asset-account transfer_out is an advance/withdrawal: it increases debt and is never a payment.</li>
                  <li>Non-transfer income is a separate income/refund credit, never a transfer payment. Statement and post-cutoff windows classify charges, advances, payments, and non-transfer credits separately.</li>
                  <li>Displayed reconciliation: cutoff balance = balance before period + opening baseline activated inside period − statement charges − statement advances + non-transfer credits + card-directed statement payments.</li>
                  <li>The anchor selects a cutoff cycle, not an as-of date. Valid ledger-dated activity in the selected windows is included even when it is later than the anchor or today; no issuer allocation is implied.</li>
                  <li>Statement balance at cutoff is historical. Current balance includes all posted card entries, including later activity and invalid legacy dates.</li>
                  <li>Report-hidden cards are excluded; picker-only hidden cards remain. Archived categories retain their historical names and do not change these card totals.</li>
                </ul>
              </div>
            </>
          )}
        </>
      )}
    </section>
  );
}
