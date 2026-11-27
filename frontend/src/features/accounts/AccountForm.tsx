import type { OverviewForms } from "../overview/useOverviewForms";
import AccountTypeSelect from "./AccountTypeSelect";

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
        <AccountTypeSelect
          definitions={forms.accountTypes}
          value={forms.accountType}
          onChange={forms.setAccountType}
          testId="account-type"
        />
        <label>
          Note <span className="optional">optional</span>
          <textarea
            data-testid="account-note"
            value={forms.accountNote}
            onChange={(event) => forms.setAccountNote(event.target.value)}
            placeholder="What is this account for?"
            maxLength={500}
          />
        </label>
        <label>
          Opening date <span className="optional">optional</span>
          <input
            data-testid="account-opening-date"
            type="date"
            value={forms.accountOpeningDate}
            onChange={(event) => forms.setAccountOpeningDate(event.target.value)}
          />
          <small className="field-hint">Metadata only; posted entries keep their dates.</small>
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
        <label className="check-label">
          <input
            data-testid="account-hide-from-selection"
            type="checkbox"
            checked={forms.accountHideFromSelection || forms.accountHideFromReports}
            onChange={(event) => forms.setAccountHideFromSelection(event.target.checked)}
            disabled={forms.accountHideFromReports}
          />
          Hide from new transaction and transfer pickers
        </label>
        <label className="check-label">
          <input
            data-testid="account-hide-from-reports"
            type="checkbox"
            checked={forms.accountHideFromReports}
            onChange={(event) => {
              const checked = event.target.checked;
              forms.setAccountHideFromReports(checked);
              if (checked) forms.setAccountHideFromSelection(true);
            }}
          />
          Hide from new pickers and future reports
        </label>
        <p className="field-hint">
          Report hiding is saved for future report screens. The current ledger, overview, CSV,
          and balances keep their semantics.
        </p>
        <button
          className="primary-button"
          data-testid="create-account"
          disabled={forms.submitting || forms.accountTypes.length === 0}
        >
          {forms.submitting ? "Saving…" : "Create account"}
        </button>
      </form>
    </aside>
  );
}
