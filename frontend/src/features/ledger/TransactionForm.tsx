import type { BudgetState } from "../../shared/types";
import { isAccountSelectable } from "../accounts/accountClassification";
import LabelMultiSelect from "../labels/LabelMultiSelect";
import type { OverviewForms } from "../overview/useOverviewForms";

export default function TransactionForm({
  data,
  forms,
}: {
  data: BudgetState;
  forms: OverviewForms;
}) {
  const selectableAccounts = data.accounts.filter(isAccountSelectable);

  return (
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
          className={forms.transactionKind === "income" ? "active income-active" : ""}
          onClick={() => {
            forms.setTransactionKind("income");
            forms.setTransactionCategoryId("");
          }}
        >
          Income
        </button>
        <button
          type="button"
          className={forms.transactionKind === "expense" ? "active expense-active" : ""}
          onClick={() => {
            forms.setTransactionKind("expense");
            forms.setTransactionCategoryId("");
          }}
        >
          Expense
        </button>
      </div>
      <form className="stack-form" onSubmit={forms.handleTransaction}>
        <label>
          Name <span className="optional">optional</span>
          <input
            data-testid="transaction-name"
            value={forms.transactionName}
            onChange={(event) => forms.setTransactionName(event.target.value)}
            placeholder="e.g. Dinner or Monthly salary"
          />
        </label>
        <div className="unnamed-name-preference">
          <label htmlFor="unnamed-transaction-name">
            Preferred name for unnamed transactions
          </label>
          <div className="unnamed-name-preference-controls">
            <input
              id="unnamed-transaction-name"
              data-testid="unnamed-transaction-name"
              value={forms.unnamedTransactionName}
              onChange={(event) => forms.setUnnamedTransactionName(event.target.value)}
              placeholder="e.g. Everyday expense"
              maxLength={120}
              disabled={forms.submitting}
              aria-describedby="unnamed-transaction-name-hint"
            />
            <button
              className="secondary-button"
              data-testid="clear-unnamed-transaction-name"
              type="button"
              onClick={() => forms.setUnnamedTransactionName("")}
              disabled={forms.submitting || !forms.unnamedTransactionName.trim()}
              aria-label="Clear preferred name"
            >
              Clear
            </button>
          </div>
          <p id="unnamed-transaction-name-hint" className="field-hint">
            Saved in this browser when available. Used only when Name is blank; a typed name
            takes precedence.
          </p>
        </div>
        <label>
          Account
          <select
            data-testid="transaction-account"
            value={forms.transactionAccountId}
            onChange={(event) => forms.setTransactionAccountId(event.target.value)}
            required
            disabled={selectableAccounts.length === 0}
          >
            <option value="">
              {selectableAccounts.length === 0 ? "No selectable accounts" : "Choose an account"}
            </option>
            {selectableAccounts.map((account) => (
              <option key={account.id} value={account.id}>
                {account.name} · {account.currency}
              </option>
            ))}
          </select>
        </label>
        <label>
          Category
          <select
            data-testid="transaction-category"
            value={forms.transactionCategoryId}
            onChange={(event) => forms.setTransactionCategoryId(event.target.value)}
          >
            <option value="">Uncategorized</option>
            {forms.transactionCategories.map((category) => (
              <option key={category.id} value={category.id}>
                {category.parent_name ? `${category.parent_name} / ` : ""}
                {category.name}
              </option>
            ))}
          </select>
        </label>
        <div className="form-row">
          <label>
            Amount
            <input
              data-testid="transaction-amount"
              value={forms.transactionAmount}
              onChange={(event) => forms.setTransactionAmount(event.target.value)}
              inputMode="decimal"
              placeholder="0.00"
              required
            />
          </label>
          <label>
            Date
            <input
              type="date"
              value={forms.transactionDate}
              onChange={(event) => forms.setTransactionDate(event.target.value)}
              required
            />
          </label>
        </div>
        <label>
          Note <span className="optional">optional</span>
          <input
            value={forms.transactionNote}
            onChange={(event) => forms.setTransactionNote(event.target.value)}
            placeholder="What was it for?"
            maxLength={500}
          />
        </label>
        <LabelMultiSelect
          labels={data.labels}
          selected={forms.transactionLabelIds}
          onChange={forms.setTransactionLabelIds}
          testIdPrefix="transaction-label"
          disabled={forms.submitting}
        />
        <label className="check-label">
          <input
            type="checkbox"
            checked={forms.transactionReconciled}
            onChange={(event) => forms.setTransactionReconciled(event.target.checked)}
          />
          Reconciled
        </label>
        <button
          className={`primary-button ${forms.transactionKind === "income" ? "income-button" : ""}`}
          disabled={forms.submitting || selectableAccounts.length === 0}
        >
          {forms.submitting
            ? "Saving…"
            : `Record ${forms.transactionKind === "income" ? "income" : "expense"}`}
        </button>
      </form>
    </section>
  );
}
