import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  type FormEvent,
} from "react";
import { api } from "./api";
import { dateTimeValue, formatMinor, localDateValue, parseMinorUnits } from "./money";
import type { BudgetState, Entry, TransactionKind } from "./types";

const emptyState: BudgetState = {
  accounts: [],
  totals: { income_minor: 0, expense_minor: 0 },
  entries: [],
};

function initials(name: string): string {
  return name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase())
    .join("");
}

function entryDetails(entry: Entry) {
  switch (entry.kind) {
    case "income":
      return { label: "Income", sign: "+", tone: "positive" };
    case "expense":
      return { label: "Expense", sign: "−", tone: "negative" };
    case "transfer_in":
      return { label: "Transfer in", sign: "+", tone: "transfer" };
    case "transfer_out":
      return { label: "Transfer out", sign: "−", tone: "transfer" };
  }
}

export default function App() {
  const [data, setData] = useState<BudgetState>(emptyState);
  const [loading, setLoading] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const [accountName, setAccountName] = useState("");
  const [openingBalance, setOpeningBalance] = useState("");

  const [transactionKind, setTransactionKind] = useState<TransactionKind>("expense");
  const [transactionAccountId, setTransactionAccountId] = useState("");
  const [transactionAmount, setTransactionAmount] = useState("");
  const [transactionNote, setTransactionNote] = useState("");
  const [transactionDate, setTransactionDate] = useState(localDateValue());

  const [transferFrom, setTransferFrom] = useState("");
  const [transferTo, setTransferTo] = useState("");
  const [transferAmount, setTransferAmount] = useState("");
  const [transferNote, setTransferNote] = useState("");
  const [transferDate, setTransferDate] = useState(localDateValue());

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      setData(await api.state());
      setError(null);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Could not load your ledger");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    setTransactionAccountId((current) => current || `${data.accounts[0]?.id ?? ""}`);
    setTransferFrom((current) => current || `${data.accounts[0]?.id ?? ""}`);
    setTransferTo(
      (current) => current || `${data.accounts[1]?.id ?? data.accounts[0]?.id ?? ""}`,
    );
  }, [data.accounts]);

  const displayCurrency = data.accounts[0]?.currency ?? "INR";
  const totalBalance = useMemo(
    () =>
      data.accounts
        .filter((account) => account.currency === displayCurrency)
        .reduce((sum, account) => sum + account.balance_minor, 0),
    [data.accounts, displayCurrency],
  );

  async function handleCreateAccount(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSubmitting(true);
    setError(null);
    setNotice(null);
    try {
      const openingMinor = parseMinorUnits(openingBalance || "0", true);
      if (openingMinor === null) throw new Error("Enter a valid opening balance");
      await api.createAccount({
        name: accountName,
        currency: "INR",
        opening_balance_minor: openingMinor,
      });
      setAccountName("");
      setOpeningBalance("");
      setNotice("Account created. Its opening balance is now part of the ledger.");
      await refresh();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Could not create account");
    } finally {
      setSubmitting(false);
    }
  }

  async function handleTransaction(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSubmitting(true);
    setError(null);
    setNotice(null);
    try {
      const amountMinor = parseMinorUnits(transactionAmount);
      if (amountMinor === null || amountMinor <= 0) {
        throw new Error("Enter an amount greater than zero, with at most two decimal places");
      }
      if (!transactionAccountId) throw new Error("Choose an account");
      await api.createTransaction({
        kind: transactionKind,
        account_id: Number(transactionAccountId),
        amount_minor: amountMinor,
        note: transactionNote,
        occurred_at: dateTimeValue(transactionDate),
      });
      setTransactionAmount("");
      setTransactionNote("");
      setNotice(`${transactionKind === "income" ? "Income" : "Expense"} recorded.`);
      await refresh();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Could not record transaction");
    } finally {
      setSubmitting(false);
    }
  }

  async function handleTransfer(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSubmitting(true);
    setError(null);
    setNotice(null);
    try {
      const amountMinor = parseMinorUnits(transferAmount);
      if (amountMinor === null || amountMinor <= 0) {
        throw new Error("Enter a transfer amount greater than zero");
      }
      if (!transferFrom || !transferTo) throw new Error("Choose both accounts");
      if (transferFrom === transferTo) throw new Error("Choose two different accounts");
      await api.createTransfer({
        from_account_id: Number(transferFrom),
        to_account_id: Number(transferTo),
        amount_minor: amountMinor,
        note: transferNote,
        occurred_at: dateTimeValue(transferDate),
      });
      setTransferAmount("");
      setTransferNote("");
      setNotice("Transfer complete. Both balances updated; income and expenses did not change.");
      await refresh();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Could not complete transfer");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="app-shell">
      <header className="topbar">
        <a className="brand" href="/" aria-label="Paisa home">
          <span className="brand-mark">₹</span>
          <span>
            <strong>Paisa</strong>
            <small>a calmer money ledger</small>
          </span>
        </a>
        <div className="local-badge">
          <span className="status-dot" />
          Local-first · SQLite
        </div>
      </header>

      <main>
        <section className="hero">
          <div>
            <p className="eyebrow">A clear view, without the clutter</p>
            <h1>Make room for what matters.</h1>
            <p className="hero-copy">
              Track the money in your accounts, record what comes in and goes out, and move
              funds between accounts without distorting your cash flow.
            </p>
          </div>
          <div className="hero-note">
            <span className="hero-note-icon">↗</span>
            <p>
              Transfers are movements, not earnings or spending. Your income and expense
              totals stay honest.
            </p>
          </div>
        </section>

        {(error || notice) && (
          <div
            className={`message ${error ? "message-error" : "message-success"}`}
            role={error ? "alert" : "status"}
          >
            <span>{error ? "!" : "✓"}</span>
            {error ?? notice}
          </div>
        )}

        <section className="summary-grid" aria-label="Money summary">
          <article className="summary-card summary-balance">
            <p>Total balance · {displayCurrency}</p>
            <h2 data-testid="total-balance">{formatMinor(totalBalance, displayCurrency)}</h2>
            <span>Across {data.accounts.length} account{data.accounts.length === 1 ? "" : "s"}</span>
          </article>
          <article className="summary-card summary-income">
            <p>Income</p>
            <h2 data-testid="income-total">{formatMinor(data.totals.income_minor)}</h2>
            <span>Money received</span>
          </article>
          <article className="summary-card summary-expense">
            <p>Expenses</p>
            <h2 data-testid="expense-total">{formatMinor(data.totals.expense_minor)}</h2>
            <span>Money spent</span>
          </article>
        </section>

        <div className="dashboard-grid">
          <section className="panel accounts-panel">
            <div className="panel-heading">
              <div>
                <p className="section-kicker">Where your money lives</p>
                <h2>Accounts</h2>
              </div>
              <button className="text-button" type="button" onClick={() => void refresh()}>
                Refresh
              </button>
            </div>

            {loading && data.accounts.length === 0 ? (
              <div className="empty-state">Loading your ledger…</div>
            ) : data.accounts.length === 0 ? (
              <div className="empty-state">
                <span className="empty-icon">◌</span>
                <h3>Start with an account</h3>
                <p>Add cash, a bank account, or a wallet and give it an opening balance.</p>
              </div>
            ) : (
              <div className="account-grid">
                {data.accounts.map((account) => (
                  <article
                    className="account-card"
                    key={account.id}
                    data-testid={`account-${account.id}`}
                    data-account-name={account.name}
                  >
                    <div className="account-card-top">
                      <span className={`account-avatar avatar-${account.id % 4}`}>
                        {initials(account.name)}
                      </span>
                      <span className="currency-chip">{account.currency}</span>
                    </div>
                    <h3>{account.name}</h3>
                    <p className="account-balance" data-testid="account-balance">
                      {formatMinor(account.balance_minor, account.currency)}
                    </p>
                    <small>
                      Opened with {formatMinor(account.opening_balance_minor, account.currency)}
                    </small>
                  </article>
                ))}
              </div>
            )}
          </section>

          <aside className="panel form-panel">
            <div className="panel-heading">
              <div>
                <p className="section-kicker">New account</p>
                <h2>Add an account</h2>
              </div>
            </div>
            <form className="stack-form" onSubmit={handleCreateAccount}>
              <label>
                Account name
                <input
                  data-testid="account-name"
                  value={accountName}
                  onChange={(event) => setAccountName(event.target.value)}
                  placeholder="e.g. Daily cash"
                  maxLength={80}
                  required
                />
              </label>
              <div className="form-row">
                <label>
                  Opening balance
                  <input
                    data-testid="opening-balance"
                    value={openingBalance}
                    onChange={(event) => setOpeningBalance(event.target.value)}
                    inputMode="decimal"
                    placeholder="0.00"
                    required
                  />
                </label>
                <label className="currency-field">
                  Currency
                  <input value="INR" readOnly aria-readonly="true" />
                  <small className="field-hint">INR only in this milestone.</small>
                </label>
              </div>
              <button className="primary-button" data-testid="create-account" disabled={submitting}>
                {submitting ? "Saving…" : "Create account"}
              </button>
            </form>
          </aside>
        </div>

        <div className="workspace-grid">
          <section className="panel form-panel">
            <div className="panel-heading">
              <div>
                <p className="section-kicker">Cash flow</p>
                <h2>Record income or expense</h2>
              </div>
            </div>
            <div className="segmented" aria-label="Transaction type">
              <button
                type="button"
                className={transactionKind === "income" ? "active income-active" : ""}
                onClick={() => setTransactionKind("income")}
              >
                Income
              </button>
              <button
                type="button"
                className={transactionKind === "expense" ? "active expense-active" : ""}
                onClick={() => setTransactionKind("expense")}
              >
                Expense
              </button>
            </div>
            <form className="stack-form" onSubmit={handleTransaction}>
              <label>
                Account
                <select
                  data-testid="transaction-account"
                  value={transactionAccountId}
                  onChange={(event) => setTransactionAccountId(event.target.value)}
                  required
                  disabled={data.accounts.length === 0}
                >
                  {data.accounts.length === 0 && <option value="">Create an account first</option>}
                  {data.accounts.map((account) => (
                    <option key={account.id} value={account.id}>
                      {account.name} · {account.currency}
                    </option>
                  ))}
                </select>
              </label>
              <div className="form-row">
                <label>
                  Amount
                  <input
                    data-testid="transaction-amount"
                    value={transactionAmount}
                    onChange={(event) => setTransactionAmount(event.target.value)}
                    inputMode="decimal"
                    placeholder="0.00"
                    required
                  />
                </label>
                <label>
                  Date
                  <input
                    type="date"
                    value={transactionDate}
                    onChange={(event) => setTransactionDate(event.target.value)}
                    required
                  />
                </label>
              </div>
              <label>
                Note <span className="optional">optional</span>
                <input
                  value={transactionNote}
                  onChange={(event) => setTransactionNote(event.target.value)}
                  placeholder="What was it for?"
                  maxLength={500}
                />
              </label>
              <button
                className={`primary-button ${transactionKind === "income" ? "income-button" : ""}`}
                disabled={submitting || data.accounts.length === 0}
              >
                {submitting
                  ? "Saving…"
                  : `Record ${transactionKind === "income" ? "income" : "expense"}`}
              </button>
            </form>
          </section>

          <section className="panel form-panel transfer-panel">
            <div className="panel-heading">
              <div>
                <p className="section-kicker">Move money</p>
                <h2>Transfer between accounts</h2>
              </div>
              <span className="transfer-symbol">⇄</span>
            </div>
            <form className="stack-form" onSubmit={handleTransfer}>
              <div className="form-row">
                <label>
                  From
                  <select
                    data-testid="transfer-from"
                    value={transferFrom}
                    onChange={(event) => setTransferFrom(event.target.value)}
                    required
                    disabled={data.accounts.length === 0}
                  >
                    {data.accounts.length === 0 && <option value="">No accounts yet</option>}
                    {data.accounts.map((account) => (
                      <option key={account.id} value={account.id}>
                        {account.name}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  To
                  <select
                    data-testid="transfer-to"
                    value={transferTo}
                    onChange={(event) => setTransferTo(event.target.value)}
                    required
                    disabled={data.accounts.length < 2}
                  >
                    {data.accounts.length < 2 && <option value="">Add two accounts first</option>}
                    {data.accounts.map((account) => (
                      <option key={account.id} value={account.id}>
                        {account.name}
                      </option>
                    ))}
                  </select>
                </label>
              </div>
              <div className="form-row">
                <label>
                  Amount
                  <input
                    data-testid="transfer-amount"
                    value={transferAmount}
                    onChange={(event) => setTransferAmount(event.target.value)}
                    inputMode="decimal"
                    placeholder="250.00"
                    required
                  />
                </label>
                <label>
                  Date
                  <input
                    type="date"
                    value={transferDate}
                    onChange={(event) => setTransferDate(event.target.value)}
                    required
                  />
                </label>
              </div>
              <label>
                Note <span className="optional">optional</span>
                <input
                  data-testid="transfer-note"
                  value={transferNote}
                  onChange={(event) => setTransferNote(event.target.value)}
                  placeholder="e.g. Move to savings"
                  maxLength={500}
                />
              </label>
              <button
                className="primary-button transfer-button"
                data-testid="submit-transfer"
                disabled={submitting || data.accounts.length < 2}
              >
                {submitting ? "Moving…" : "Complete transfer"}
              </button>
              <p className="form-footnote">
                Both sides are written atomically. Income and expense totals are untouched.
              </p>
            </form>
          </section>
        </div>

        <section className="panel activity-panel">
          <div className="panel-heading">
            <div>
              <p className="section-kicker">Latest activity</p>
              <h2>Ledger entries</h2>
            </div>
            <span className="entry-count">{data.entries.length} recent</span>
          </div>
          {data.entries.length === 0 ? (
            <div className="activity-empty">No entries yet. Your first transaction will appear here.</div>
          ) : (
            <div className="entry-list">
              {data.entries.map((entry) => {
                const details = entryDetails(entry);
                return (
                  <article className="entry-row" key={entry.id}>
                    <span className={`entry-icon ${details.tone}`}>
                      {entry.kind === "transfer_in" || entry.kind === "transfer_out" ? "⇄" : details.sign}
                    </span>
                    <div className="entry-main">
                      <strong>{entry.note || details.label}</strong>
                      <span>
                        {details.label} · {entry.account_name}
                      </span>
                    </div>
                    <time dateTime={entry.occurred_at}>{entry.occurred_at.slice(0, 10)}</time>
                    <strong className={`entry-amount ${details.tone}`}>
                      {details.sign}
                      {formatMinor(entry.amount_minor)}
                    </strong>
                  </article>
                );
              })}
            </div>
          )}
        </section>
      </main>

      <footer>
        <span>Paisa · first working milestone</span>
        <span>Amounts are stored as integer minor units (paise).</span>
      </footer>
    </div>
  );
}
