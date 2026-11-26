import { useState, type FormEvent } from "react";
import { api } from "../../shared/api";
import { parseMinorUnits } from "../../shared/money";
import type { BudgetState, LedgerEntry, TransactionKind } from "../../shared/types";

export default function EntryEditor({
  entry,
  data,
  onClose,
  onSaved,
}: {
  entry: LedgerEntry;
  data: BudgetState;
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const isTransfer =
    entry.kind === "transfer" || entry.kind === "transfer_in" || entry.kind === "transfer_out";
  const [kind, setKind] = useState<TransactionKind>(
    entry.kind === "income" ? "income" : "expense",
  );
  const [name, setName] = useState(entry.name);
  const [accountId, setAccountId] = useState(
    String(entry.kind === "transfer_in" ? entry.counterpart_account_id : entry.account_id),
  );
  const [counterpartId, setCounterpartId] = useState(
    String(entry.kind === "transfer_in" ? entry.account_id : entry.counterpart_account_id),
  );
  const [categoryId, setCategoryId] = useState(
    entry.category_id ? String(entry.category_id) : "",
  );
  const [amount, setAmount] = useState((entry.amount_minor / 100).toFixed(2));
  const [note, setNote] = useState(entry.note);
  const [date, setDate] = useState(entry.occurred_at.slice(0, 10));
  const [reconciled, setReconciled] = useState(entry.reconciled);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const activeCategories = data.categories.filter(
    (category) =>
      category.kind === kind &&
      (category.archived_at === null || category.id === entry.category_id),
  );

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSaving(true);
    setError(null);
    try {
      const amountMinor = parseMinorUnits(amount);
      if (amountMinor === null || amountMinor <= 0) {
        throw new Error("Enter a valid amount greater than zero");
      }
      if (isTransfer) {
        await api.updateTransfer(entry.id, {
          from_account_id: Number(accountId),
          to_account_id: Number(counterpartId),
          amount_minor: amountMinor,
          name,
          reconciled,
          note,
          occurred_at: date ? `${date}T12:00:00` : undefined,
        });
      } else {
        await api.updateTransaction(entry.id, {
          kind,
          account_id: Number(accountId),
          amount_minor: amountMinor,
          name,
          category_id: categoryId ? Number(categoryId) : null,
          reconciled,
          note,
          occurred_at: date ? `${date}T12:00:00` : undefined,
        });
      }
      await onSaved();
      onClose();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Could not save changes");
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="modal-backdrop" role="presentation">
      <section className="modal-card" role="dialog" aria-modal="true" aria-label="Edit entry">
        <div className="panel-heading">
          <div>
            <p className="section-kicker">{isTransfer ? "Transfer" : "Transaction"}</p>
            <h2>Edit entry</h2>
          </div>
          <button className="icon-button" type="button" onClick={onClose} aria-label="Close editor">
            ×
          </button>
        </div>
        {error && <div className="message message-error">{error}</div>}
        <form className="stack-form" onSubmit={submit}>
          <label>
            Name
            <input
              data-testid="edit-name"
              value={name}
              onChange={(event) => setName(event.target.value)}
              required
            />
          </label>
          {!isTransfer && (
            <div className="form-row">
              <label>
                Type
                <select
                  value={kind}
                  onChange={(event) => {
                    const next = event.target.value as TransactionKind;
                    setKind(next);
                    setCategoryId("");
                  }}
                >
                  <option value="expense">Expense</option>
                  <option value="income">Income</option>
                </select>
              </label>
              <label>
                Category
                <select
                  data-testid="edit-category"
                  value={categoryId}
                  onChange={(event) => setCategoryId(event.target.value)}
                >
                  <option value="">Uncategorized</option>
                  {activeCategories.map((category) => (
                    <option key={category.id} value={category.id}>
                      {category.parent_name ? `${category.parent_name} / ` : ""}
                      {category.name}
                      {category.archived_at ? " (archived)" : ""}
                    </option>
                  ))}
                </select>
              </label>
            </div>
          )}
          <div className={isTransfer ? "form-row" : ""}>
            <label>
              {isTransfer ? "From" : "Account"}
              <select
                data-testid="edit-account"
                value={accountId}
                onChange={(event) => setAccountId(event.target.value)}
                required
              >
                {data.accounts.map((account) => (
                  <option key={account.id} value={account.id}>
                    {account.name}
                  </option>
                ))}
              </select>
            </label>
            {isTransfer && (
              <label>
                To
                <select
                  data-testid="edit-counterpart"
                  value={counterpartId}
                  onChange={(event) => setCounterpartId(event.target.value)}
                  required
                >
                  {data.accounts.map((account) => (
                    <option key={account.id} value={account.id}>
                      {account.name}
                    </option>
                  ))}
                </select>
              </label>
            )}
          </div>
          <div className="form-row">
            <label>
              Amount
              <input
                data-testid="edit-amount"
                value={amount}
                onChange={(event) => setAmount(event.target.value)}
                inputMode="decimal"
                required
              />
            </label>
            <label>
              Date
              <input
                type="date"
                value={date}
                onChange={(event) => setDate(event.target.value)}
                required
              />
            </label>
          </div>
          <label>
            Note
            <textarea value={note} onChange={(event) => setNote(event.target.value)} rows={3} />
          </label>
          <label className="check-label">
            <input
              type="checkbox"
              checked={reconciled}
              onChange={(event) => setReconciled(event.target.checked)}
            />
            Reconciled
          </label>
          <button className="primary-button" data-testid="save-edit" disabled={saving}>
            {saving ? "Saving…" : "Save changes"}
          </button>
        </form>
      </section>
    </div>
  );
}
