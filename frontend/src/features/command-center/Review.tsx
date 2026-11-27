import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../../shared/api";
import { formatMinor } from "../../shared/money";
import type {
  Account,
  BudgetState,
  Category,
  Draft,
  DraftRow,
  DraftRowKind,
} from "../../shared/types";
import {
  accountLabel,
  amountToInput,
  formatAmount,
  groupRowsByDate,
  hasUnresolvedAccount,
  isReadyToSave,
  ownPrompts,
  parseAmountInput,
} from "./draft";

interface ReviewProps {
  draft: Draft;
  data: BudgetState;
  busy: boolean;
  error: string | null;
  notice: string | null;
  onAssign: (rowIds: string[], accountId: number) => Promise<boolean>;
  onReplace: (rowId: string, row: DraftRow) => Promise<boolean>;
  onRemove: (rowId: string) => void;
  onSave: () => void;
  onAddDetails: (text: string) => Promise<boolean>;
  onBack: () => void;
  onAddManual: () => void;
  onAddAccounts: () => void;
  onDiscard: () => void;
  onOpenTransfers: () => void;
}

const KIND_LABELS: Record<DraftRowKind, string> = {
  expense: "Expense",
  income: "Income",
  transfer: "Transfer",
  new_account: "New account",
};

function accountSelectValue(row: DraftRow, destination = false): string {
  const proposed = destination ? null : row.proposed_row_id;
  const id = destination ? row.to_account_id : row.account_id;
  if (id !== null) return String(id);
  return proposed ? `proposed:${proposed}` : "";
}

function accountFields(value: string, destination: boolean): Partial<DraftRow> {
  if (destination) {
    return value
      ? { to_account_id: Number(value), to_account_name: null }
      : { to_account_id: null, to_account_name: null };
  }
  if (!value) {
    return { account_id: null, account_name: null, proposed_row_id: null };
  }
  if (value.startsWith("proposed:")) {
    return {
      account_id: null,
      account_name: null,
      proposed_row_id: value.slice("proposed:".length),
    };
  }
  return { account_id: Number(value), account_name: null, proposed_row_id: null };
}

interface RowEditorProps {
  row: DraftRow;
  accounts: Account[];
  categories: Category[];
  draftRows: DraftRow[];
  onSave: (row: DraftRow) => Promise<boolean>;
  onCancel: () => void;
}

/** Inline editor: it uses the same field patterns as the rest of the app and stays on the row. */
function RowEditor({ row, accounts, categories, draftRows, onSave, onCancel }: RowEditorProps) {
  const [name, setName] = useState(row.name);
  const [kind, setKind] = useState<DraftRowKind>(row.kind);
  const [amountText, setAmountText] = useState(amountToInput(row.amount_minor));
  const [date, setDate] = useState(row.date ?? "");
  const [accountValue, setAccountValue] = useState(accountSelectValue(row));
  const [toAccountValue, setToAccountValue] = useState(accountSelectValue(row, true));
  const [categoryValue, setCategoryValue] = useState(
    row.category_id === null ? "" : String(row.category_id),
  );
  const [note, setNote] = useState(row.note);
  const [accountType, setAccountType] = useState(row.account_type ?? "cash");
  const [openingText, setOpeningText] = useState(amountToInput(row.opening_balance_minor));
  const [currentText, setCurrentText] = useState(amountToInput(row.current_balance_minor));
  const [formError, setFormError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [aliasSaved, setAliasSaved] = useState(false);

  const rememberedName = row.account_name?.trim() ?? "";
  const canRememberAlias =
    rememberedName.length > 0 && accountValue.length > 0 && !accountValue.startsWith("proposed:");

  async function rememberAlias() {
    if (!canRememberAlias) return;
    setFormError(null);
    try {
      await api.rememberAlias(rememberedName, Number(accountValue));
      setAliasSaved(true);
    } catch (error) {
      setFormError(error instanceof Error ? error.message : "Could not remember that nickname.");
    }
  }

  const proposedAccounts = draftRows.filter(
    (candidate) => candidate.kind === "new_account" && candidate.row_id !== row.row_id,
  );

  async function saveEditor() {
    setFormError(null);
    if (!name.trim()) {
      setFormError("Add a description for this row.");
      return;
    }
    if (!date) {
      setFormError("Choose a date.");
      return;
    }
    let amount: number | null = null;
    if (kind !== "new_account") {
      amount = parseAmountInput(amountText);
      if (amount === null || amount <= 0) {
        setFormError("Enter an amount greater than zero.");
        return;
      }
    }
    const opening = kind === "new_account" ? parseAmountInput(openingText) : null;
    const current = kind === "new_account" ? parseAmountInput(currentText) : null;
    if (kind === "new_account") {
      if (
        (openingText.trim() && opening === null) ||
        (currentText.trim() && current === null)
      ) {
        setFormError("Enter balances as rupees, for example 5000 or 5000.50.");
        return;
      }
      if (opening === null && current === null) {
        setFormError("Add an opening or current balance.");
        return;
      }
    }

    const next: DraftRow = {
      ...row,
      name: name.trim(),
      kind,
      amount_minor: kind === "new_account" ? null : amount,
      date,
      ...accountFields(accountValue, false),
      ...accountFields(toAccountValue, true),
      category_id: kind === "new_account" || kind === "transfer" || !categoryValue ? null : Number(categoryValue),
      note,
      manual: true,
      account_type: kind === "new_account" ? accountType : null,
      opening_balance_minor: opening,
      current_balance_minor: current,
      status: "needs_input",
      issues: [],
    };
    setSaving(true);
    try {
      const saved = await onSave(next);
      if (!saved) setFormError("This row was not updated. Correct it and try again.");
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="cc-row-editor">
      <div className="cc-editor-grid">
        <label className="cc-edit-field">
          Description
          <input
            data-testid="cc-edit-name"
            value={name}
            onChange={(event) => setName(event.target.value)}
          />
        </label>
        <label className="cc-edit-field">
          Type
          <select
            data-testid="cc-edit-kind"
            value={kind}
            onChange={(event) => setKind(event.target.value as DraftRowKind)}
          >
            {(Object.keys(KIND_LABELS) as DraftRowKind[]).map((value) => (
              <option key={value} value={value}>
                {KIND_LABELS[value]}
              </option>
            ))}
          </select>
        </label>
        {kind !== "new_account" && (
          <>
            <label className="cc-edit-field">
              Amount (₹)
              <input
                data-testid="cc-edit-amount"
                inputMode="decimal"
                value={amountText}
                placeholder="240"
                onChange={(event) => setAmountText(event.target.value)}
              />
            </label>
            <label className="cc-edit-field">
              Date
              <input
                type="date"
                data-testid="cc-edit-date"
                value={date}
                onChange={(event) => setDate(event.target.value)}
              />
            </label>
            <label className="cc-edit-field">
              {kind === "transfer" ? "Sending account" : "Account"}
              <select
                data-testid="cc-edit-account"
                value={accountValue}
                onChange={(event) => setAccountValue(event.target.value)}
              >
                <option value="">Choose an account</option>
                {accounts.map((account) => (
                  <option key={account.id} value={account.id}>
                    {account.name}
                  </option>
                ))}
                {proposedAccounts.map((proposed) => (
                  <option key={proposed.row_id} value={`proposed:${proposed.row_id}`}>
                    New: {proposed.name || "account"}
                  </option>
                ))}
              </select>
            </label>
            {canRememberAlias && (
              <button
                className="cc-button cc-remember-alias"
                data-testid="cc-remember-alias"
                type="button"
                onClick={() => void rememberAlias()}
              >
                {aliasSaved ? `Remembered ${rememberedName}` : `Remember ${rememberedName} for this account`}
              </button>
            )}
            {kind === "transfer" && (
              <label className="cc-edit-field">
                Receiving account
                <select
                  data-testid="cc-edit-to-account"
                  value={toAccountValue}
                  onChange={(event) => setToAccountValue(event.target.value)}
                >
                  <option value="">Choose a receiving account</option>
                  {accounts.map((account) => (
                    <option key={account.id} value={account.id}>
                      {account.name}
                    </option>
                  ))}
                </select>
              </label>
            )}
            {kind !== "transfer" && (
              <label className="cc-edit-field">
                Category
                <select
                  data-testid="cc-edit-category"
                  value={categoryValue}
                  onChange={(event) => setCategoryValue(event.target.value)}
                >
                  <option value="">Uncategorized</option>
                  {categories.map((category) => (
                    <option key={category.id} value={category.id}>
                      {category.name}
                    </option>
                  ))}
                </select>
              </label>
            )}
          </>
        )}
        {kind === "new_account" && (
          <>
            <label className="cc-edit-field">
              Account type
              <select
                data-testid="cc-edit-account-type"
                value={accountType}
                onChange={(event) => setAccountType(event.target.value)}
              >
                <option value="cash">Cash</option>
                <option value="checking">Checking</option>
                <option value="savings">Savings</option>
                <option value="wallet">Wallet</option>
                <option value="credit_card">Credit Card</option>
                <option value="unclassified">Unclassified</option>
              </select>
            </label>
            <label className="cc-edit-field">
              Opening balance (₹)
              <input
                data-testid="cc-edit-opening-balance"
                inputMode="decimal"
                value={openingText}
                placeholder="5000"
                onChange={(event) => setOpeningText(event.target.value)}
              />
            </label>
            <label className="cc-edit-field">
              Current balance (₹) <span className="optional">optional</span>
              <input
                data-testid="cc-edit-current-balance"
                inputMode="decimal"
                value={currentText}
                onChange={(event) => setCurrentText(event.target.value)}
              />
            </label>
          </>
        )}
        <label className="cc-edit-field cc-edit-note">
          Note <span className="optional">optional</span>
          <textarea
            data-testid="cc-edit-note"
            value={note}
            onChange={(event) => setNote(event.target.value)}
            rows={2}
          />
        </label>
      </div>
      {formError && (
        <p className="cc-inline-error cc-editor-error" role="alert">
          {formError}
        </p>
      )}
      <div className="cc-editor-actions">
        <button
          className="primary-button"
          data-testid="cc-edit-save"
          type="button"
          disabled={saving}
          onClick={() => void saveEditor()}
        >
          {saving ? "Saving…" : "Save row"}
        </button>
        <button className="cc-button" data-testid="cc-edit-cancel" type="button" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </div>
  );
}

export default function Review({
  draft,
  data,
  busy,
  error,
  notice,
  onAssign,
  onReplace,
  onRemove,
  onSave,
  onAddDetails,
  onBack,
  onAddManual,
  onAddAccounts,
  onDiscard,
  onOpenTransfers,
}: ReviewProps) {
  const [selected, setSelected] = useState<string[]>([]);
  const [accountChoice, setAccountChoice] = useState("");
  const [details, setDetails] = useState("");
  const [detailsBusy, setDetailsBusy] = useState(false);
  const [editingRowId, setEditingRowId] = useState<string | null>(null);
  const rowRefs = useRef<Record<string, HTMLElement | null>>({});

  const groups = useMemo(
    () => groupRowsByDate(draft.rows, draft.local_date),
    [draft.rows, draft.local_date],
  );
  const assignableRows = useMemo(
    () => draft.rows.filter(hasUnresolvedAccount),
    [draft.rows],
  );
  const prompts = useMemo(() => ownPrompts(draft.prompts), [draft.prompts]);
  const canSave = !busy && isReadyToSave(draft.rows);

  useEffect(() => {
    setSelected((current) =>
      current.filter((rowId) => assignableRows.some((row) => row.row_id === rowId)),
    );
  }, [assignableRows]);

  useEffect(() => {
    if (!accountChoice && data.accounts.length > 0) {
      setAccountChoice(String(data.accounts[0].id));
    }
  }, [accountChoice, data.accounts]);

  function focusRow(rowId: string) {
    requestAnimationFrame(() => rowRefs.current[rowId]?.focus());
  }

  function toggleSelected(rowId: string) {
    setSelected((current) =>
      current.includes(rowId)
        ? current.filter((value) => value !== rowId)
        : [...current, rowId],
    );
  }

  async function assignSelected() {
    if (!accountChoice || selected.length === 0) return;
    const ok = await onAssign(selected, Number(accountChoice));
    if (ok) setSelected([]);
  }

  async function submitDetails() {
    if (!details.trim() || detailsBusy) return;
    setDetailsBusy(true);
    try {
      const ok = await onAddDetails(details);
      if (ok) setDetails("");
    } finally {
      setDetailsBusy(false);
    }
  }

  return (
    <section className="cc-shell cc-review" data-testid="cc-review">
      <div className="cc-page-heading cc-review-heading">
        <button className="cc-back text-button" type="button" onClick={onBack}>
          ← Command Center
        </button>
        <h1>Review entries</h1>
        <p className="cc-lede">
          Check every row before saving. Dates are shown in your local calendar.
        </p>
      </div>

      {error && (
        <div className="cc-inline-error" role="alert">
          {error}
        </div>
      )}
      {notice && (
        <div className="cc-inline-notice" role="status">
          {notice}
        </div>
      )}

      <div className="cc-review-toolbar">
        <button className="cc-button" type="button" data-testid="cc-review-add-manually" onClick={onAddManual}>
          Add manually
        </button>
        <button className="cc-button" type="button" data-testid="cc-review-add-accounts" onClick={onAddAccounts}>
          Add accounts
        </button>
        <button className="cc-button" type="button" onClick={onOpenTransfers}>
          Transfers to review
        </button>
      </div>

      <div className="cc-assign-bar">
        <p className="cc-assign-title">Assign selected rows to an account</p>
        <div className="cc-assign-controls">
          <select
            data-testid="cc-assign-account"
            aria-label="Account for selected rows"
            value={accountChoice}
            onChange={(event) => setAccountChoice(event.target.value)}
          >
            <option value="">Choose an account</option>
            {data.accounts.map((account) => (
              <option key={account.id} value={account.id}>
                {account.name}
              </option>
            ))}
          </select>
          <button
            className="cc-button"
            type="button"
            disabled={busy || selected.length === 0 || !accountChoice}
            onClick={() => void assignSelected()}
          >
            Assign to account
          </button>
        </div>
        <p className="cc-field-hint">
          {selected.length === 0
            ? `${assignableRows.length} row${assignableRows.length === 1 ? "" : "s"} need an account.`
            : `${selected.length} selected.`}
        </p>
      </div>

      <div className="cc-review-list">
        {groups.length === 0 && (
          <div className="empty-state">
            <span className="empty-icon">◌</span>
            <h3>No rows to review</h3>
            <p>Add a row manually or go back and describe your activity.</p>
          </div>
        )}
        {groups.map((group) => (
          <section className="cc-date-group" key={group.date}>
            <h2 className="cc-date-heading">
              {group.label} <time dateTime={`${group.date}T12:00:00`}>{group.date}</time>
            </h2>
            {group.rows.map((row) => {
              const unresolved = hasUnresolvedAccount(row);
              const isEditing = editingRowId === row.row_id;
              return (
                <article
                  className={`cc-review-row${unresolved ? " cc-row-unresolved" : ""}`}
                  data-testid="cc-review-row"
                  data-kind={row.kind}
                  key={row.row_id}
                  ref={(node) => {
                    rowRefs.current[row.row_id] = node;
                  }}
                  tabIndex={-1}
                >
                  <div className="cc-row-top">
                    {unresolved && (
                      <label className="cc-row-check">
                        <input
                          type="checkbox"
                          data-testid="cc-row-select"
                          aria-label={`Select ${row.name || "row"} for account assignment`}
                          checked={selected.includes(row.row_id)}
                          onChange={() => toggleSelected(row.row_id)}
                        />
                        <span>Select</span>
                      </label>
                    )}
                    <div className="cc-row-info">
                      <strong>{row.name || "Untitled row"}</strong>
                      <span className="cc-row-amount">{formatAmount(row.amount_minor)}</span>
                      <span className="cc-row-accounts">
                        {row.kind === "new_account"
                          ? `${formatMinor(row.opening_balance_minor ?? row.current_balance_minor ?? 0)} opening balance`
                          : row.kind === "transfer"
                            ? `${accountLabel(row, data.accounts, draft.rows)} → ${accountLabel(row, data.accounts, draft.rows, true)}`
                            : accountLabel(row, data.accounts, draft.rows)}
                      </span>
                    </div>
                    <div className="cc-row-actions">
                      <button
                        className="cc-button"
                        data-testid="cc-row-edit"
                        type="button"
                        aria-label={`Edit ${row.name || "row"}`}
                        onClick={() => setEditingRowId(row.row_id)}
                      >
                        Edit
                      </button>
                      <button
                        className="cc-button cc-remove"
                        type="button"
                        aria-label={`Remove ${row.name || "row"}`}
                        disabled={busy}
                        onClick={() => onRemove(row.row_id)}
                      >
                        Remove
                      </button>
                    </div>
                  </div>
                  <p className="cc-row-meta">
                    {KIND_LABELS[row.kind]} · {row.status === "ready" ? "Ready" : "Needs input"}
                    {row.manual ? " · Manually edited" : ""}
                  </p>
                  {(row.issues.length > 0 || row.transfer_request) && (
                    <ul className="cc-row-issues">
                      {row.issues.map((issue, index) => (
                        <li data-testid="cc-row-issue" key={`${issue}-${index}`}>
                          Needs attention: {issue}
                        </li>
                      ))}
                      {row.transfer_request && (
                        <li data-testid="cc-row-issue">Pending approval after save</li>
                      )}
                    </ul>
                  )}
                  {isEditing && (
                    <RowEditor
                      key={row.row_id}
                      row={row}
                      accounts={data.accounts}
                      categories={data.categories}
                      draftRows={draft.rows}
                      onSave={async (updated) => {
                        const ok = await onReplace(row.row_id, updated);
                        if (ok) {
                          setEditingRowId(null);
                          focusRow(row.row_id);
                        }
                        return ok;
                      }}
                      onCancel={() => {
                        setEditingRowId(null);
                        focusRow(row.row_id);
                      }}
                    />
                  )}
                </article>
              );
            })}
          </section>
        ))}
      </div>

      <div className="cc-followup">
        <label className="cc-field-label" htmlFor="cc-add-details-input">
          Add details
        </label>
        <textarea
          id="cc-add-details-input"
          data-testid="cc-add-details"
          placeholder="Add to this draft, for example: the coffee was from the HDFC card."
          value={details}
          rows={3}
          onChange={(event) => setDetails(event.target.value)}
        />
        <button
          className="cc-button"
          data-testid="cc-add-details-submit"
          type="button"
          disabled={detailsBusy || !details.trim()}
          onClick={() => void submitDetails()}
        >
          {detailsBusy ? "Adding…" : "Add details"}
        </button>
        <details className="cc-history" data-testid="cc-prompt-history">
          <summary>Prompt history</summary>
          {prompts.length === 0 ? (
            <p>No prompts in this draft yet.</p>
          ) : (
            <ol>
              {prompts.map((prompt) => (
                <li key={`${prompt.submission_id}-${prompt.seq}`}>
                  <span>{prompt.text}</span>
                  <time dateTime={prompt.created_at}>{prompt.created_at.slice(0, 10)}</time>
                </li>
              ))}
            </ol>
          )}
        </details>
      </div>

      <div className="cc-save-bar">
        <button
          className="primary-button cc-save"
          data-testid="cc-save"
          type="button"
          disabled={!canSave}
          onClick={onSave}
        >
          Save{draft.rows.length > 0 ? ` ${draft.rows.length} row${draft.rows.length === 1 ? "" : "s"}` : ""}
        </button>
        <button className="cc-button cc-discard" type="button" onClick={onDiscard}>
          Discard draft
        </button>
      </div>
    </section>
  );
}
