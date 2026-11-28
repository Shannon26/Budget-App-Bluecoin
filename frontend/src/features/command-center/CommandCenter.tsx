import { useCallback, useEffect, useState } from "react";
import { api } from "../../shared/api";
import { localDateValue } from "../../shared/date";
import type {
  AssistantUsageSummary,
  BudgetState,
  Draft,
  DraftRow,
  DraftRowKind,
  SaveReceipt,
} from "../../shared/types";
import AccountsView from "./AccountsView";
import Receipt from "./Receipt";
import Review from "./Review";
import TransferReview from "./TransferReview";
import { blankDraftRow, formatUsd, newLocalRowId, nextOrdinal, ownPrompts } from "./draft";

export type CommandScreen = "command-center" | "review" | "transfer-review" | "accounts";

interface CommandCenterProps {
  screen: CommandScreen;
  onScreen: (screen: CommandScreen) => void;
  data: BudgetState;
  refresh: () => Promise<void>;
  onMessage: (message: string, error?: boolean) => void;
  onOpenLedger: (entryId?: number) => void;
  onOpenOverview: () => void;
}

const EXAMPLE_PROMPT =
  "Yesterday I paid ₹240 for lunch from HDFC and ₹80 for coffee in cash.";

function emptyDraft(): Draft {
  return {
    member_id: 1,
    revision: 0,
    local_date: localDateValue(),
    updated_at: "",
    rows: [],
    prompts: [],
  };
}

function reasonText(reason: unknown, fallback: string): string {
  return reason instanceof Error && reason.message ? reason.message : fallback;
}

export default function CommandCenter({
  screen,
  onScreen,
  data,
  refresh,
  onMessage,
  onOpenLedger,
  onOpenOverview,
}: CommandCenterProps) {
  const [draft, setDraft] = useState<Draft>(emptyDraft);
  const [loadingDraft, setLoadingDraft] = useState(true);
  const [draftUnavailable, setDraftUnavailable] = useState(false);
  const [composer, setComposer] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [receipt, setReceipt] = useState<SaveReceipt | null>(null);
  const [showReceipt, setShowReceipt] = useState(false);
  const [incomingCount, setIncomingCount] = useState(0);
  const [usage, setUsage] = useState<AssistantUsageSummary | null>(null);
  const [usageUnavailable, setUsageUnavailable] = useState(false);

  useEffect(() => {
    let current = true;
    void (async () => {
      try {
        const loaded = await api.draft();
        if (!current) return;
        setDraft(loaded);
        setDraftUnavailable(false);
      } catch (reason) {
        if (!current) return;
        setDraftUnavailable(true);
        setError(reasonText(reason, "Could not load your draft"));
      } finally {
        if (current) setLoadingDraft(false);
      }
    })();
    return () => {
      current = false;
    };
  }, []);

  const loadUsage = useCallback(async () => {
    try {
      setUsage(await api.assistantUsage());
      setUsageUnavailable(false);
    } catch {
      setUsageUnavailable(true);
    }
  }, []);

  useEffect(() => {
    void loadUsage();
  }, [loadUsage]);

  const loadRequestCount = useCallback(async () => {
    try {
      const result = await api.transferRequests();
      setIncomingCount(result.incoming_count ?? result.incoming.length);
    } catch {
      setIncomingCount(0);
    }
  }, []);

  useEffect(() => {
    void loadRequestCount();
  }, [loadRequestCount]);

  // A nav click always leaves the receipt behind and returns to that screen.
  useEffect(() => {
    setShowReceipt(false);
  }, [screen]);

  const applyDraft = useCallback((next: Draft) => {
    setDraft(next);
    setDraftUnavailable(false);
  }, []);

  const submitPrompt = useCallback(
    async (text: string, navigateOnRows = true): Promise<boolean> => {
      const trimmed = text.trim();
      if (!trimmed || busy) return false;
      setBusy(true);
      setError(null);
      setNotice(null);
      try {
        const response = await api.appendPrompt(
          trimmed,
          newLocalRowId("prompt"),
          draft.local_date,
        );
        applyDraft(response.draft);
        const feedback = response.clarification ?? response.message;
        if (feedback) setNotice(feedback);
        if (navigateOnRows && response.draft.rows.length > 0) onScreen("review");
        return true;
      } catch (reason) {
        const message = reasonText(reason, "Could not prepare that prompt");
        setError(message);
        onMessage(message, true);
        return false;
      } finally {
        setBusy(false);
        void loadUsage();
      }
    },
    [applyDraft, busy, draft.local_date, loadUsage, onMessage, onScreen],
  );

  const addRow = useCallback(
    (kind: DraftRowKind) => {
      const newRow = blankDraftRow(
        kind,
        nextOrdinal(draft.rows),
        draft.local_date,
        newLocalRowId("row"),
      );
      setError(null);
      void (async () => {
        try {
          applyDraft(await api.addDraftRow(newRow));
        } catch (reason) {
          // Keep the row in view so the member can still work through a failed API.
          const message = reasonText(reason, "Could not add the row");
          setError(message);
          onMessage(message, true);
          applyDraft({ ...draft, rows: [...draft.rows, newRow] });
        }
        onScreen("review");
      })();
    },
    [applyDraft, draft, onMessage, onScreen],
  );

  const assignAccount = useCallback(
    async (rowIds: string[], accountId: number): Promise<boolean> => {
      if (rowIds.length === 0) return false;
      setBusy(true);
      setError(null);
      try {
        applyDraft(await api.assignDraftAccount(rowIds, accountId));
        setNotice(`Account assigned to ${rowIds.length} row${rowIds.length === 1 ? "" : "s"}.`);
        return true;
      } catch (reason) {
        const message = reasonText(reason, "Could not assign the account");
        setError(message);
        onMessage(message, true);
        return false;
      } finally {
        setBusy(false);
      }
    },
    [applyDraft, onMessage],
  );

  const replaceRow = useCallback(
    async (rowId: string, row: DraftRow): Promise<boolean> => {
      setBusy(true);
      setError(null);
      try {
        applyDraft(await api.replaceDraftRow(rowId, row));
        return true;
      } catch (reason) {
        const message = reasonText(reason, "Could not save that row");
        setError(message);
        onMessage(message, true);
        return false;
      } finally {
        setBusy(false);
      }
    },
    [applyDraft, onMessage],
  );

  const removeRow = useCallback(
    (rowId: string) => {
      setError(null);
      void (async () => {
        try {
          applyDraft(await api.removeDraftRow(rowId));
        } catch (reason) {
          const message = reasonText(reason, "Could not remove that row");
          setError(message);
          onMessage(message, true);
          applyDraft({ ...draft, rows: draft.rows.filter((row) => row.row_id !== rowId) });
        }
      })();
    },
    [applyDraft, draft, onMessage],
  );

  const save = useCallback(() => {
    setBusy(true);
    setError(null);
    void (async () => {
      try {
        const saved = await api.saveDraft(draft.revision);
        setReceipt(saved);
        setDraft(emptyDraft());
        setComposer("");
        setNotice(null);
        setShowReceipt(true);
        onMessage("Draft saved to your ledger.");
        try {
          await refresh();
          await loadRequestCount();
        } catch {
          // The receipt still reflects the committed batch.
        }
      } catch (reason) {
        const message = reasonText(reason, "Could not save the draft");
        setError(message);
        onMessage(message, true);
      } finally {
        setBusy(false);
      }
    })();
  }, [draft.revision, loadRequestCount, onMessage, refresh]);

  const discard = useCallback(() => {
    setError(null);
    setNotice(null);
    void (async () => {
      try {
        await api.discardDraft();
        onMessage("Draft discarded.");
      } catch (reason) {
        onMessage(reasonText(reason, "Could not discard the draft"), true);
      }
      setDraft(emptyDraft());
      setComposer("");
      setReceipt(null);
      setShowReceipt(false);
      onScreen("command-center");
    })();
  }, [onMessage, onScreen]);

  const startNewDraft = useCallback(() => {
    setShowReceipt(false);
    setReceipt(null);
    setComposer("");
    setError(null);
    setNotice(null);
    setDraft(emptyDraft());
    onScreen("command-center");
    void (async () => {
      try {
        applyDraft(await api.draft());
      } catch {
        // A fresh local draft is already in place.
      }
    })();
  }, [applyDraft, onScreen]);

  const prompts = ownPrompts(draft.prompts);

  if (receipt && showReceipt) {
    return (
      <Receipt
        receipt={receipt}
        accounts={data.accounts}
        onViewLedger={() => onOpenLedger(receipt.entry_ids[0])}
        onNewDraft={startNewDraft}
        onOpenTransfers={() => onScreen("transfer-review")}
      />
    );
  }

  if (screen === "review") {
    return (
      <Review
        draft={draft}
        data={data}
        busy={busy}
        error={error}
        notice={notice}
        onAssign={assignAccount}
        onReplace={replaceRow}
        onRemove={removeRow}
        onSave={save}
        onAddDetails={(text) => submitPrompt(text, false)}
        onBack={() => onScreen("command-center")}
        onAddManual={() => addRow("expense")}
        onAddAccounts={() => addRow("new_account")}
        onDiscard={discard}
        onOpenTransfers={() => onScreen("transfer-review")}
      />
    );
  }

  if (screen === "transfer-review") {
    return (
      <TransferReview
        onBack={() => onScreen("command-center")}
        onMessage={onMessage}
        onChanged={() => {
          void loadRequestCount();
          return refresh();
        }}
      />
    );
  }

  if (screen === "accounts") {
    return (
      <AccountsView
        accounts={data.accounts}
        onBack={() => onScreen("command-center")}
        onMessage={onMessage}
        refresh={refresh}
      />
    );
  }

  return (
    <section className="cc-shell cc-composer" data-testid="cc-composer-screen">
      <div className="cc-page-heading">
        <p className="section-kicker">Command Center</p>
        <h1>Log activity</h1>
        <p className="cc-lede">
          Describe a day’s activity in your own words, or build it row by row. Review everything
          before it reaches your ledger.
        </p>
      </div>

      <div className="cc-card cc-compose-card">
        <label className="cc-field-label" htmlFor="cc-composer-input">
          What happened?
        </label>
        <textarea
          id="cc-composer-input"
          data-testid="cc-composer"
          className="cc-composer-input"
          placeholder={EXAMPLE_PROMPT}
          value={composer}
          onChange={(event) => {
            setComposer(event.target.value);
            if (error) setError(null);
          }}
          rows={4}
        />
        <p className="cc-field-hint">Type or use your phone’s dictation. One prompt can cover a whole day.</p>
        <button
          className="primary-button cc-prepare"
          data-testid="cc-prepare"
          type="button"
          disabled={busy || composer.trim().length === 0}
          onClick={() => {
            void submitPrompt(composer);
          }}
        >
          {busy ? "Preparing…" : "Prepare"}
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

      <div className="cc-entry-points">
        <button
          className="cc-action-card"
          data-testid="cc-add-manually"
          type="button"
          onClick={() => addRow("expense")}
        >
          <strong>Add manually</strong>
          <span>Start with a blank entry</span>
        </button>
        <button
          className="cc-action-card"
          data-testid="cc-add-accounts"
          type="button"
          onClick={() => addRow("new_account")}
        >
          <strong>Add accounts</strong>
          <span>Set up a bank or cash account</span>
        </button>
        {(draft.rows.length > 0 || draft.prompts.length > 0) && (
          <button
            className="cc-action-card cc-resume"
            data-testid="cc-continue-draft"
            type="button"
            onClick={() => onScreen("review")}
          >
            <strong>Continue draft</strong>
            <span>
              {draft.rows.length} row{draft.rows.length === 1 ? "" : "s"} · {prompts.length} prompt
              {prompts.length === 1 ? "" : "s"}
            </span>
          </button>
        )}
        <button
          className="cc-action-card cc-transfer-entry"
          data-testid="cc-transfer-badge"
          type="button"
          onClick={() => onScreen("transfer-review")}
        >
          <strong>Transfers to review</strong>
          <span className="cc-count" aria-label={`${incomingCount} incoming requests`}>
            {incomingCount}
          </span>
        </button>
      </div>

      {loadingDraft && <p className="cc-field-hint">Loading your draft…</p>}
      {draftUnavailable && (
        <p className="cc-field-hint">Saved drafts are unavailable; you can still add rows manually.</p>
      )}

      <details className="cc-cost-history" data-testid="cc-cost-history">
        <summary>
          <span>Assistant cost history</span>
          <strong data-testid="cc-cost-total">
            {usage ? `${formatUsd(usage.total_cost_usd)} USD` : "—"}
          </strong>
        </summary>
        <div className="cc-cost-history-body">
          {usageUnavailable ? (
            <p>Cost history is unavailable.</p>
          ) : !usage ? (
            <p>Loading reported assistant cost…</p>
          ) : usage.completed_count === 0 ? (
            <p>No reported assistant usage yet.</p>
          ) : (
            <>
              <p>
                {usage.completed_count} response{usage.completed_count === 1 ? "" : "s"} ·{" "}
                {usage.reported_count} cost{usage.reported_count === 1 ? "" : "s"} reported ·{" "}
                {usage.zero_cost_count} zero-cost · {usage.missing_cost_count} missing
              </p>
              <ul>
                {[...usage.days].reverse().map((day) => (
                  <li key={day.date}>
                    <time dateTime={day.date}>{day.date}</time>
                    <span>{formatUsd(day.total_cost_usd)} USD</span>
                    <small>
                      {day.reported_count} reported · {day.zero_cost_count} zero ·{" "}
                      {day.missing_cost_count} missing
                    </small>
                  </li>
                ))}
              </ul>
              <p>Actual cost reported by OpenRouter. Missing costs are never estimated.</p>
            </>
          )}
        </div>
      </details>

      <div className="cc-secondary-links">
        <button type="button" className="text-button" onClick={() => onScreen("accounts")}>
          Accounts →
        </button>
        <button type="button" className="text-button" onClick={onOpenOverview}>
          Overview →
        </button>
        <button type="button" className="text-button" onClick={() => onOpenLedger()}>
          Ledger →
        </button>
      </div>
    </section>
  );
}
