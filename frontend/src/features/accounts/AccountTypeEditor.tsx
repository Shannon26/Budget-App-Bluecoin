import { useState, type FormEvent } from "react";
import { api } from "../../shared/api";
import { parseMinorUnits } from "../../shared/money";
import type { Account, AccountTypeDefinition, AccountUpdate } from "../../shared/types";
import AccountTypeSelect from "./AccountTypeSelect";

interface AccountTypeEditorProps {
  account: Account;
  definitions: AccountTypeDefinition[];
  refresh: () => Promise<void>;
  onMessage: (message: string, error?: boolean) => void;
}

interface AccountDraft {
  name: string;
  accountType: string;
  note: string;
  openingDate: string;
  hideFromSelection: boolean;
  hideFromReports: boolean;
  creditLimit: string;
  cutoffDay: string;
  dueDay: string;
}

function optionalDay(value: string, label: string): number | null {
  if (!value) return null;
  const day = Number(value);
  if (!Number.isInteger(day) || day < 1 || day > 31) {
    throw new Error(`${label} must be between 1 and 31`);
  }
  return day;
}

export default function AccountTypeEditor({
  account,
  definitions,
  refresh,
  onMessage,
}: AccountTypeEditorProps) {
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(account.name);
  const [accountType, setAccountType] = useState(account.account_type || "unclassified");
  const [note, setNote] = useState(account.note || "");
  const [openingDate, setOpeningDate] = useState(account.opening_date || "");
  const [hideFromSelection, setHideFromSelection] = useState(account.hide_from_selection);
  const [hideFromReports, setHideFromReports] = useState(account.hide_from_reports);
  const [creditLimit, setCreditLimit] = useState(
    account.credit_limit_minor === null ? "" : (account.credit_limit_minor / 100).toFixed(2),
  );
  const [cutoffDay, setCutoffDay] = useState(
    account.cutoff_day === null ? "" : String(account.cutoff_day),
  );
  const [dueDay, setDueDay] = useState(account.due_day === null ? "" : String(account.due_day));
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [openedDraft, setOpenedDraft] = useState<AccountDraft | null>(null);

  function openEditor() {
    const draft: AccountDraft = {
      name: account.name,
      accountType: account.account_type || "unclassified",
      note: account.note || "",
      openingDate: account.opening_date || "",
      hideFromSelection: account.hide_from_selection,
      hideFromReports: account.hide_from_reports,
      creditLimit:
        account.credit_limit_minor === null ? "" : (account.credit_limit_minor / 100).toFixed(2),
      cutoffDay: account.cutoff_day === null ? "" : String(account.cutoff_day),
      dueDay: account.due_day === null ? "" : String(account.due_day),
    };
    setName(draft.name);
    setAccountType(draft.accountType);
    setNote(draft.note);
    setOpeningDate(draft.openingDate);
    setHideFromSelection(draft.hideFromSelection);
    setHideFromReports(draft.hideFromReports);
    setCreditLimit(draft.creditLimit);
    setCutoffDay(draft.cutoffDay);
    setDueDay(draft.dueDay);
    setOpenedDraft(draft);
    setError(null);
    setEditing(true);
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSubmitting(true);
    setError(null);
    onMessage("", true);
    try {
      if (!openedDraft) return;
      const update: AccountUpdate = {};
      if (name !== openedDraft.name) update.name = name;
      if (note !== openedDraft.note) update.note = note.trim() ? note : null;
      if (openingDate !== openedDraft.openingDate) {
        update.opening_date = openingDate || null;
      }
      if (accountType !== openedDraft.accountType) update.account_type = accountType;
      const hideFromSelectionValue = hideFromSelection || hideFromReports;
      const openedHideFromSelectionValue =
        openedDraft.hideFromSelection || openedDraft.hideFromReports;
      if (hideFromSelectionValue !== openedHideFromSelectionValue) {
        update.hide_from_selection = hideFromSelectionValue;
      }
      if (hideFromReports !== openedDraft.hideFromReports) {
        update.hide_from_reports = hideFromReports;
      }
      if (accountType === "credit_card") {
        if (creditLimit !== openedDraft.creditLimit) {
          const limit = creditLimit.trim() ? parseMinorUnits(creditLimit) : null;
          if (creditLimit.trim() && limit === null) {
            throw new Error("Enter a valid credit limit");
          }
          if (limit !== null && limit <= 0) {
            throw new Error("Credit limit must be greater than zero");
          }
          update.credit_limit_minor = limit;
        }
        if (cutoffDay !== openedDraft.cutoffDay) {
          update.cutoff_day = optionalDay(cutoffDay, "Cutoff day");
        }
        if (dueDay !== openedDraft.dueDay) {
          update.due_day = optionalDay(dueDay, "Due day");
        }
      }
      if (Object.keys(update).length === 0) {
        setEditing(false);
        return;
      }
      await api.updateAccount(account.id, update);
      onMessage("Account updated. Balances and posted history are unchanged.");
      await refresh();
      setEditing(false);
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : "Could not update account";
      setError(message);
      onMessage(message, true);
    } finally {
      setSubmitting(false);
    }
  }

  if (!editing) {
    return (
      <button
        className="text-button account-type-button"
        data-testid={`account-type-edit-${account.id}`}
        type="button"
        onClick={openEditor}
        disabled={definitions.length === 0}
      >
        Edit account
      </button>
    );
  }

  return (
    <form className="account-type-editor account-editor" onSubmit={handleSubmit}>
      <label>
        Account name
        <input
          data-testid={`edit-account-name-${account.id}`}
          value={name}
          onChange={(event) => setName(event.target.value)}
          maxLength={80}
          required
          disabled={submitting}
        />
      </label>
      <AccountTypeSelect
        definitions={definitions}
        value={accountType}
        onChange={setAccountType}
        testId={`edit-account-type-${account.id}`}
        disabled={submitting}
      />
      {accountType === "credit_card" && (
        <>
          <label>
            Credit limit <span className="optional">optional</span>
            <input
              data-testid={`edit-account-credit-limit-${account.id}`}
              value={creditLimit}
              onChange={(event) => setCreditLimit(event.target.value)}
              inputMode="decimal"
              placeholder="14000.00"
              disabled={submitting}
            />
          </label>
          <div className="form-row">
            <label>
              Cutoff day <span className="optional">optional</span>
              <input
                data-testid={`edit-account-cutoff-day-${account.id}`}
                type="number"
                min={1}
                max={31}
                step={1}
                value={cutoffDay}
                onChange={(event) => setCutoffDay(event.target.value)}
                disabled={submitting}
              />
            </label>
            <label>
              Due day <span className="optional">optional</span>
              <input
                data-testid={`edit-account-due-day-${account.id}`}
                type="number"
                min={1}
                max={31}
                step={1}
                value={dueDay}
                onChange={(event) => setDueDay(event.target.value)}
                disabled={submitting}
              />
            </label>
          </div>
          <p className="field-hint">
            Used by Card Summary. Paisa does not infer issuer billing or reminder schedules.
          </p>
        </>
      )}
      <label>
        Note <span className="optional">optional</span>
        <textarea
          data-testid={`edit-account-note-${account.id}`}
          value={note}
          onChange={(event) => setNote(event.target.value)}
          placeholder="What is this account for?"
          maxLength={500}
          disabled={submitting}
        />
      </label>
      <label>
        Opening date <span className="optional">optional</span>
        <input
          data-testid={`edit-account-opening-date-${account.id}`}
          type="date"
          value={openingDate}
          onChange={(event) => setOpeningDate(event.target.value)}
          disabled={submitting}
        />
        <small className="field-hint">Metadata only; posted entries keep their dates.</small>
      </label>
      <label className="check-label">
        <input
          data-testid={`edit-account-hide-selection-${account.id}`}
          type="checkbox"
          checked={hideFromSelection || hideFromReports}
          onChange={(event) => setHideFromSelection(event.target.checked)}
          disabled={submitting || hideFromReports}
        />
        Hide from new transaction and transfer pickers
      </label>
      <label className="check-label">
        <input
          data-testid={`edit-account-hide-reports-${account.id}`}
          type="checkbox"
          checked={hideFromReports}
          onChange={(event) => {
            const checked = event.target.checked;
            setHideFromReports(checked);
            if (checked) setHideFromSelection(true);
          }}
          disabled={submitting}
        />
        Hide from new pickers and future reports
      </label>
      <p className="field-hint">
        Report hiding is saved for future report screens. The current ledger, overview, CSV,
        and balances keep their semantics.
      </p>
      {error && (
        <p className="message-error account-type-error" role="alert">
          {error}
        </p>
      )}
      <div className="account-type-actions">
        <button
          className="primary-button"
          data-testid={`save-account-type-${account.id}`}
          type="submit"
          disabled={submitting || definitions.length === 0}
        >
          {submitting ? "Saving…" : "Save account"}
        </button>
        <button
          className="text-button"
          data-testid={`cancel-account-type-${account.id}`}
          type="button"
          onClick={() => {
            setEditing(false);
            setError(null);
          }}
          disabled={submitting}
        >
          Cancel
        </button>
      </div>
    </form>
  );
}
