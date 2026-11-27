import { useCallback, useEffect, useState } from "react";
import { api } from "../../shared/api";
import { formatMinor } from "../../shared/money";
import type { Member, TransferRequestRecord, TransferRequestsResponse } from "../../shared/types";

interface TransferReviewProps {
  onBack: () => void;
  onMessage: (message: string, error?: boolean) => void;
  onChanged: () => Promise<void>;
}

const emptyRequests: TransferRequestsResponse = {
  incoming: [],
  outgoing: [],
  incoming_count: 0,
  outgoing_count: 0,
};

function currentMemberId(): number {
  try {
    const raw = typeof localStorage === "undefined" ? null : localStorage.getItem("paisa.memberId");
    const parsed = raw === null ? NaN : Number(raw);
    return Number.isFinite(parsed) && parsed > 0 ? parsed : 1;
  } catch {
    return 1;
  }
}

export default function TransferReview({ onBack, onMessage, onChanged }: TransferReviewProps) {
  const [requests, setRequests] = useState<TransferRequestsResponse>(emptyRequests);
  const [members, setMembers] = useState<Member[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const [result, memberList] = await Promise.all([
        api.transferRequests(),
        api.householdMembers().catch(() => [] as Member[]),
      ]);
      setRequests(result);
      setMembers(memberList);
      setError(null);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Could not load transfer requests");
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const me = currentMemberId();

  function memberName(id: number): string {
    if (id === me) return "You";
    const member = members.find((candidate) => candidate.id === id);
    return member ? member.name : `Member ${id}`;
  }

  async function act(
    action: "approve" | "decline" | "cancel" | "reverse",
    id: number,
  ) {
    setBusy(true);
    setError(null);
    try {
      if (action === "approve") await api.approveRequest(id);
      else if (action === "decline") await api.declineRequest(id);
      else if (action === "cancel") await api.cancelRequest(id);
      else await api.reverseTransfer(id);
      onMessage(
        action === "reverse"
          ? "Reversal submitted as a new request."
          : `Request #${id} ${action === "approve" ? "approved" : action === "decline" ? "declined" : "canceled"}.`,
      );
      await load();
      await onChanged();
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : "Could not update the request";
      setError(message);
      onMessage(message, true);
    } finally {
      setBusy(false);
    }
  }

  function requestCard(request: TransferRequestRecord, direction: "incoming" | "outgoing") {
    const posted = request.status === "approved" && request.posted_transfer_id !== null;
    return (
      <article
        className={`cc-transfer-request${posted ? " cc-transfer-posted" : ""}`}
        data-testid="cc-transfer-request"
        key={`${direction}-${request.id}`}
      >
        <div className="cc-transfer-top">
          <strong>{request.name || "Transfer"}</strong>
          <span className="cc-transfer-amount">{formatMinor(request.amount_minor)}</span>
        </div>
        <p className="cc-transfer-route">
          {request.source_account_name} → {request.destination_account_name}
        </p>
        <p className="cc-transfer-details">
          {direction === "incoming" ? "From " : "To "}
          {memberName(direction === "incoming" ? request.sender_member_id : request.recipient_member_id)}
          {" · "}Effective {request.requested_date} · Status: {request.status}
        </p>
        {request.note && <p className="cc-transfer-note">{request.note}</p>}
        <div className="cc-transfer-actions">
          {direction === "incoming" && request.status === "pending" && (
            <>
              <button
                className="cc-button cc-approve"
                data-testid="cc-approve"
                type="button"
                disabled={busy}
                onClick={() => void act("approve", request.id)}
              >
                Approve
              </button>
              <button
                className="cc-button cc-decline"
                data-testid="cc-decline"
                type="button"
                disabled={busy}
                onClick={() => void act("decline", request.id)}
              >
                Decline
              </button>
            </>
          )}
          {direction === "outgoing" && request.status === "pending" && (
            <button
              className="cc-button"
              data-testid="cc-cancel"
              type="button"
              disabled={busy}
              onClick={() => void act("cancel", request.id)}
            >
              Cancel
            </button>
          )}
        </div>
      </article>
    );
  }

  const all = [...requests.incoming, ...requests.outgoing];
  const posted = all.filter(
    (request) => request.status === "approved" && request.posted_transfer_id !== null,
  );

  return (
    <section className="cc-shell cc-transfer-review">
      <div className="cc-page-heading">
        <button className="cc-back text-button" type="button" onClick={onBack}>
          ← Command Center
        </button>
        <h1>Transfers to review</h1>
        <p className="cc-lede">
          Pending requests have no effect on balances until the receiving member approves them.
        </p>
      </div>

      {error && (
        <div className="cc-inline-error" role="alert">
          {error}
        </div>
      )}

      <section className="cc-transfer-section">
        <h2>Incoming ({requests.incoming_count})</h2>
        {requests.incoming.length === 0 ? (
          <p className="cc-field-hint">No incoming requests.</p>
        ) : (
          requests.incoming.map((request) => requestCard(request, "incoming"))
        )}
      </section>

      <section className="cc-transfer-section">
        <h2>Outgoing ({requests.outgoing_count})</h2>
        {requests.outgoing.length === 0 ? (
          <p className="cc-field-hint">No outgoing requests.</p>
        ) : (
          requests.outgoing.map((request) => requestCard(request, "outgoing"))
        )}
      </section>

      <section className="cc-transfer-section">
        <h2>Posted cross-member transfers</h2>
        {posted.length === 0 ? (
          <p className="cc-field-hint">No approved cross-member transfers to reverse.</p>
        ) : (
          posted.map((request) => (
            <article className="cc-transfer-request cc-transfer-posted" key={`posted-${request.id}`}>
              <div className="cc-transfer-top">
                <strong>{request.name || "Transfer"}</strong>
                <span className="cc-transfer-amount">{formatMinor(request.amount_minor)}</span>
              </div>
              <p className="cc-transfer-route">
                {request.source_account_name} → {request.destination_account_name} · Effective{" "}
                {request.requested_date} · Approved by {memberName(request.decided_by_member_id ?? me)}
              </p>
              <button
                className="cc-button"
                data-testid="cc-reverse"
                type="button"
                disabled={busy}
                onClick={() => void act("reverse", request.posted_transfer_id!)}
              >
                Reverse this transfer
              </button>
            </article>
          ))
        )}
      </section>
    </section>
  );
}
