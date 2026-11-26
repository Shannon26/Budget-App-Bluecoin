import type { BudgetState } from "../../shared/types";
import type { OverviewForms } from "../overview/useOverviewForms";

export default function TransferForm({
  data,
  forms,
}: {
  data: BudgetState;
  forms: OverviewForms;
}) {
  return (
    <section className="panel form-panel transfer-panel">
      <div className="panel-heading">
        <div>
          <p className="section-kicker">Move money</p>
          <h2>Transfer between accounts</h2>
        </div>
        <span className="transfer-symbol">⇄</span>
      </div>
      <form className="stack-form" onSubmit={forms.handleTransfer}>
        <label>
          Name
          <input
            data-testid="transfer-name"
            value={forms.transferName}
            onChange={(event) => forms.setTransferName(event.target.value)}
            placeholder="e.g. Move to savings"
          />
        </label>
        <div className="form-row">
          <label>
            From
            <select
              data-testid="transfer-from"
              value={forms.transferFrom}
              onChange={(event) => forms.setTransferFrom(event.target.value)}
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
              value={forms.transferTo}
              onChange={(event) => forms.setTransferTo(event.target.value)}
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
              value={forms.transferAmount}
              onChange={(event) => forms.setTransferAmount(event.target.value)}
              inputMode="decimal"
              placeholder="250.00"
              required
            />
          </label>
          <label>
            Date
            <input
              type="date"
              value={forms.transferDate}
              onChange={(event) => forms.setTransferDate(event.target.value)}
              required
            />
          </label>
        </div>
        <label>
          Note <span className="optional">optional</span>
          <input
            data-testid="transfer-note"
            value={forms.transferNote}
            onChange={(event) => forms.setTransferNote(event.target.value)}
            placeholder="e.g. Move to savings"
            maxLength={500}
          />
        </label>
        <label className="check-label">
          <input
            type="checkbox"
            checked={forms.transferReconciled}
            onChange={(event) => forms.setTransferReconciled(event.target.checked)}
          />
          Reconciled
        </label>
        <button
          className="primary-button transfer-button"
          data-testid="submit-transfer"
          disabled={forms.submitting || data.accounts.length < 2}
        >
          {forms.submitting ? "Moving…" : "Complete transfer"}
        </button>
        <p className="form-footnote">
          Both sides are written atomically. Income and expense totals are untouched.
        </p>
      </form>
    </section>
  );
}
