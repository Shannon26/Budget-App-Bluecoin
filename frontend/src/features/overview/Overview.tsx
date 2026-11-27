import { useMemo } from "react";
import { formatMinor } from "../../shared/money";
import type { BudgetState } from "../../shared/types";
import AccountForm from "../accounts/AccountForm";
import AccountTypeEditor from "../accounts/AccountTypeEditor";
import {
  accountGroupLabel,
  accountTypeDefinition,
  isAccountSelectable,
} from "../accounts/accountClassification";
import TransactionForm from "../ledger/TransactionForm";
import TransferForm from "../transfers/TransferForm";
import { activityDetails } from "./activity";
import type { OverviewForms } from "./useOverviewForms";

interface OverviewProps {
  data: BudgetState;
  loading: boolean;
  refresh: () => Promise<void>;
  forms: OverviewForms;
  onMessage: (message: string, error?: boolean) => void;
  onAccountHistory: (accountId: number) => void;
  onOpenLedger: () => void;
  onOpenCategories: () => void;
}

function initials(name: string): string {
  return name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase())
    .join("");
}

export default function Overview({
  data,
  loading,
  refresh,
  forms,
  onMessage,
  onAccountHistory,
  onOpenLedger,
  onOpenCategories,
}: OverviewProps) {
  const totalBalance = useMemo(
    () => data.accounts.reduce((sum, account) => sum + account.balance_minor, 0),
    [data.accounts],
  );
  const selectableAccounts = useMemo(
    () => data.accounts.filter(isAccountSelectable),
    [data.accounts],
  );
  const categorySpotlight = useMemo(
    () =>
      data.category_totals
        .filter((total) => total.category_id !== null)
        .sort((left, right) => right.total_minor - left.total_minor)
        .slice(0, 6),
    [data.category_totals],
  );

  return (
    <>
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

      <section className="summary-grid" aria-label="Money summary">
        <article className="summary-card summary-balance">
          <p>Total balance · INR</p>
          <h2 data-testid="total-balance">{formatMinor(totalBalance)}</h2>
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
              {data.accounts.map((account) => {
                const definition = accountTypeDefinition(data.account_types, account.account_type);
                return (
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
                    <div
                      className="account-classification"
                      data-testid={`account-classification-${account.id}`}
                    >
                      <span className="account-type-chip">{definition.label}</span>
                      <span className="account-group-chip">
                        {accountGroupLabel(account.account_group)}
                      </span>
                    </div>
                    <p className="account-balance" data-testid="account-balance">
                      {formatMinor(account.balance_minor, account.currency)}
                    </p>
                    {account.account_type === "credit_card" && (
                      <small data-testid={`account-balance-state-${account.id}`}>
                        Latest balance ·{" "}
                        {account.balance_minor < 0
                          ? "Amount owed"
                          : account.balance_minor > 0
                            ? "Credit balance"
                            : "Settled"}
                      </small>
                    )}
                    <small>
                      Opened with {formatMinor(account.opening_balance_minor, account.currency)}
                    </small>
                    <small data-testid={`account-opening-date-${account.id}`}>
                      Opening date: {account.opening_date || "Not set"}
                    </small>
                    {account.account_type === "credit_card" && (
                      <>
                        <small data-testid={`account-credit-limit-${account.id}`}>
                          Credit limit:{" "}
                          {account.credit_limit_minor === null
                            ? "Not set"
                            : formatMinor(account.credit_limit_minor, account.currency)}
                        </small>
                        <small data-testid={`account-cutoff-day-${account.id}`}>
                          Cutoff day: {account.cutoff_day === null ? "Not set" : account.cutoff_day}
                        </small>
                        <small data-testid={`account-due-day-${account.id}`}>
                          Due day: {account.due_day === null ? "Not set" : account.due_day}
                        </small>
                      </>
                    )}
                    {account.note && (
                      <p className="account-note" data-testid={`account-note-${account.id}`}>
                        {account.note}
                      </p>
                    )}
                    {(account.hide_from_selection || account.hide_from_reports) && (
                      <div
                        className="account-visibility"
                        data-testid={`account-visibility-${account.id}`}
                      >
                        {(account.hide_from_selection || account.hide_from_reports) && (
                          <span>Hidden from new pickers</span>
                        )}
                        {account.hide_from_reports && <span>Hidden from future reports</span>}
                      </div>
                    )}
                    <div className="account-card-actions">
                      <AccountTypeEditor
                        account={account}
                        definitions={data.account_types}
                        refresh={refresh}
                        onMessage={onMessage}
                      />
                      <button
                        className="text-button account-history-button"
                        data-testid={`account-history-${account.id}`}
                        type="button"
                        onClick={() => onAccountHistory(account.id)}
                      >
                        View history →
                      </button>
                    </div>
                  </article>
                );
              })}
            </div>
          )}
        </section>

        <AccountForm forms={forms} />
      </div>

      {data.accounts.length > 0 && selectableAccounts.length === 0 && (
        <div className="message account-picker-empty" role="status">
          <span>i</span>
          All accounts are hidden from new entries. Edit an account to make one selectable.
        </div>
      )}

      <div className="workspace-grid">
        <TransactionForm data={data} forms={forms} />
        <TransferForm data={data} forms={forms} />
      </div>

      {categorySpotlight.length > 0 && (
        <section className="panel category-spotlight">
          <div className="panel-heading">
            <div>
              <p className="section-kicker">Where it goes</p>
              <h2>Category totals</h2>
            </div>
            <button className="text-button" type="button" onClick={onOpenCategories}>
              Manage categories
            </button>
          </div>
          <div className="category-total-grid">
            {categorySpotlight.map((total) => (
              <article key={`${total.kind}-${total.category_id}`}>
                <span>{total.name}</span>
                <strong className={total.kind === "income" ? "positive-text" : "negative-text"}>
                  {formatMinor(total.total_minor)}
                </strong>
                <small>{total.entry_count} entries</small>
              </article>
            ))}
          </div>
        </section>
      )}

      <section className="panel activity-panel">
        <div className="panel-heading">
          <div>
            <p className="section-kicker">Latest activity</p>
            <h2>Ledger entries</h2>
          </div>
          <button className="text-button" type="button" onClick={onOpenLedger}>
            Open ledger
          </button>
        </div>
        {data.entries.length === 0 ? (
          <div className="activity-empty">No entries yet. Your first transaction will appear here.</div>
        ) : (
          <div className="entry-list">
            {data.entries.slice(0, 12).map((entry) => {
              const details = activityDetails(entry);
              return (
                <article className="entry-row" key={`${entry.kind}-${entry.id}`}>
                  <span className={`entry-icon ${details.tone}`}>{details.sign}</span>
                  <div className="entry-main">
                    <strong>{entry.name || details.label}</strong>
                    <span>
                      {details.account} · {details.category}
                    </span>
                  </div>
                  {entry.reconciled && <span className="reconciled-badge">✓</span>}
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
    </>
  );
}
