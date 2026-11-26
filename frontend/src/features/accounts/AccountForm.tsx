import type { OverviewForms } from "../overview/useOverviewForms";

export default function AccountForm({ forms }: { forms: OverviewForms }) {
  return (
    <aside className="panel form-panel">
      <div className="panel-heading">
        <div>
          <p className="section-kicker">New account</p>
          <h2>Add an account</h2>
        </div>
      </div>
      <form className="stack-form" onSubmit={forms.handleCreateAccount}>
        <label>
          Account name
          <input
            data-testid="account-name"
            value={forms.accountName}
            onChange={(event) => forms.setAccountName(event.target.value)}
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
              value={forms.openingBalance}
              onChange={(event) => forms.setOpeningBalance(event.target.value)}
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
        <button className="primary-button" data-testid="create-account" disabled={forms.submitting}>
          {forms.submitting ? "Saving…" : "Create account"}
        </button>
      </form>
    </aside>
  );
}
