import { useEffect, useState } from "react";
import { api } from "../../shared/api";
import { formatMinor } from "../../shared/money";
import type { Account, SaveReceipt, TransferRequestRecord } from "../../shared/types";
import { receiptCounts } from "./draft";

interface ReceiptProps {
  receipt: SaveReceipt;
  accounts: Account[];
  onViewLedger: () => void;
  onNewDraft: () => void;
  onOpenTransfers: () => void;
}

export default function Receipt({
  receipt,
  accounts,
  onViewLedger,
  onNewDraft,
  onOpenTransfers,
}: ReceiptProps) {
  const [pending, setPending] = useState<TransferRequestRecord[]>([]);
  const counts = receiptCounts(receipt);

  useEffect(() => {
    let current = true;
    if (receipt.pending_request_ids.length === 0) {
      setPending([]);
      return () => {
        current = false;
      };
    }
    void (async () => {
      try {
        const result = await api.transferRequests();
        const all = [...result.incoming, ...result.outgoing];
        if (current) {
          setPending(
            receipt.pending_request_ids
              .map((id) => all.find((request) => request.id === id))
              .filter((request): request is TransferRequestRecord => Boolean(request)),
          );
        }
      } catch {
        // Pending ids still appear below even when their details cannot load.
      }
    })();
    return () => {
      current = false;
    };
  }, [receipt]);

  const accountNames = accounts
    .filter((account) => receipt.accounts_created.includes(account.id))
    .map((account) => account.name);

  return (
    <section className="cc-shell cc-receipt" data-testid="cc-receipt">
      <div className="cc-page-heading">
        <p className="section-kicker">Saved</p>
        <h1>Batch receipt</h1>
        <p className="cc-lede">This batch is in your ledger. Anything awaiting approval is listed separately.</p>
      </div>

      <div className="cc-card cc-receipt-summary">
        <h2 data-testid="cc-receipt-saved">
          Saved {counts.accounts} account{counts.accounts === 1 ? "" : "s"}, {counts.entries} entr
          {counts.entries === 1 ? "y" : "ies"}, {counts.transfers} transfer
          {counts.transfers === 1 ? "" : "s"}
        </h2>
        <p className="cc-field-hint">
          Batch {receipt.batch_id} · revision {receipt.revision} · {receipt.created_at.slice(0, 16)}
        </p>
        {accountNames.length > 0 && <p>New accounts: {accountNames.join(", ")}</p>}
      </div>

      <div className="cc-card" data-testid="cc-receipt-pending">
        <h2>Waiting for approval</h2>
        {counts.pending === 0 ? (
          <p>No transfers were submitted for approval in this batch.</p>
        ) : (
          <ul className="cc-pending-list">
            {receipt.pending_request_ids.map((id) => {
              const request = pending.find((candidate) => candidate.id === id);
              return (
                <li key={id}>
                  <strong>Request #{id}</strong>
                  <span>
                    {request
                      ? `${request.name || "Transfer"} · ${formatMinor(request.amount_minor)} · ${request.requested_date} · ${request.status}`
                      : "Submitted for the receiving member’s approval"}
                  </span>
                </li>
              );
            })}
          </ul>
        )}
        <button className="cc-button" type="button" onClick={onOpenTransfers}>
          Review transfers →
        </button>
      </div>

      <div className="cc-receipt-actions">
        <button className="primary-button" type="button" onClick={onViewLedger}>
          View in Ledger
        </button>
        <button className="cc-button" type="button" onClick={onNewDraft}>
          Start a new draft
        </button>
      </div>
    </section>
  );
}
