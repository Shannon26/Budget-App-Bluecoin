import { useState, type FormEvent } from "react";
import { api } from "../../shared/api";
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
    };
    setName(draft.name);
    setAccountType(draft.accountType);
    setNote(draft.note);
    setOpeningDate(draft.openingDate);
    setHideFromSelection(draft.hideFromSelection);
    setHideFromReports(draft.hideFromReports);
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
