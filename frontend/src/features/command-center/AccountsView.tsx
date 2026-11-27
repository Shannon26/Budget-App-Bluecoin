import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../../shared/api";
import { formatMinor } from "../../shared/money";
import type {
  Account,
  AssignmentSelection,
  AssignmentView,
  HouseholdAccountView,
  Member,
} from "../../shared/types";

interface AccountsViewProps {
  accounts: Account[];
  onBack: () => void;
  onMessage: (message: string, error?: boolean) => void;
  refresh: () => Promise<void>;
}

function currentMemberId(): number {
  try {
    const raw = typeof localStorage === "undefined" ? null : localStorage.getItem("paisa.memberId");
    const parsed = raw === null ? NaN : Number(raw);
    return Number.isFinite(parsed) && parsed > 0 ? parsed : 1;
  } catch {
    return 1;
  }
}

function fallbackView(
  owner: Member,
  accounts: Account[],
  readOnly: boolean,
): HouseholdAccountView {
  return {
    owner,
    read_only: readOnly,
    accounts: accounts.map((account) => ({
      id: account.id,
      name: account.name,
      account_type: account.account_type,
      currency: account.currency,
      opening_balance_minor: account.opening_balance_minor,
      balance_minor: account.balance_minor,
      owner_member_id: owner.id,
      can_write: !readOnly,
    })),
  };
}

export default function AccountsView({
  accounts,
  onBack,
  onMessage,
  refresh,
}: AccountsViewProps) {
  const myId = currentMemberId();
  const defaultMembers: Member[] = [{ id: myId, name: "You", is_designated: false }];
  const membersRef = useRef<Member[]>(defaultMembers);
  const [members, setMembers] = useState<Member[]>(defaultMembers);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [view, setView] = useState<HouseholdAccountView | null>(null);
  const [assignments, setAssignments] = useState<AssignmentView | null>(null);
  const [ownerChoices, setOwnerChoices] = useState<Record<number, number>>({});
  const [confirmIds, setConfirmIds] = useState<number[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    let current = true;
    void (async () => {
      try {
        const list = await api.householdMembers();
        if (current) {
          const resolved = list.length > 0 ? list : defaultMembers;
          membersRef.current = resolved;
          setMembers(resolved);
        }
      } catch {
        // Single-member installs have no membership table entry.
      }
    })();
    void (async () => {
      try {
        const result = await api.householdAccounts(selectedId ?? undefined);
        if (current) setView(result);
      } catch (reason) {
        if (!current) return;
        const targetId = selectedId ?? myId;
        const owner =
          membersRef.current.find((member) => member.id === targetId) ??
          (selectedId === null
            ? { id: myId, name: "You", is_designated: false }
            : { id: selectedId, name: `Member ${selectedId}`, is_designated: false });
        setView(fallbackView(owner, accounts, selectedId !== null));
        if (selectedId === null && reason instanceof Error) setError(reason.message);
      }
    })();
    void (async () => {
      try {
        const result = await api.householdAssignments();
        if (!current) return;
        setAssignments(result);
        const choices: Record<number, number> = {};
        const toConfirm: number[] = [];
        for (const row of result.accounts) {
          const choice = row.proposed_owner_member_id ?? row.current_owner_member_id ?? row.confirmed_owner_member_id;
          if (choice !== null && choice !== undefined) choices[row.account_id] = choice;
          if (row.proposed_owner_member_id !== null && row.proposed_owner_member_id !== undefined) {
            toConfirm.push(row.account_id);
          }
        }
        setOwnerChoices(choices);
        setConfirmIds(toConfirm);
      } catch {
        if (current) setAssignments(null);
      }
    })();
    return () => {
      current = false;
    };
  }, [accounts, myId, selectedId]);

  useEffect(() => load(), [load]);

  const readOnly = selectedId !== null;
  const owner = view?.owner ?? { id: myId, name: "You", is_designated: false };

  async function submitAssignments(kind: "propose" | "confirm") {
    if (!assignments) return;
    setBusy(true);
    setError(null);
    try {
      const selections: AssignmentSelection[] = assignments.accounts
        .filter((row) => (kind === "confirm" ? confirmIds.includes(row.account_id) : ownerChoices[row.account_id] !== undefined))
        .map((row) => ({
          account_id: row.account_id,
          owner_member_id: ownerChoices[row.account_id] ?? myId,
        }));
      if (kind === "propose") {
        setAssignments(await api.proposeAssignments(selections));
        onMessage(`Proposed ${selections.length} assignment${selections.length === 1 ? "" : "s"}.`);
      } else {
        await api.confirmAssignments(selections);
        onMessage(`Confirmed ${selections.length} assignment${selections.length === 1 ? "" : "s"}.`);
      }
      await refresh();
      load();
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : "Could not update assignments";
      setError(message);
      onMessage(message, true);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="cc-shell cc-accounts" data-testid="cc-accounts">
      <div className="cc-page-heading">
        <button className="cc-back text-button" type="button" onClick={onBack}>
          ← Command Center
        </button>
        <h1>Accounts</h1>
        <p className="cc-lede">My accounts is the default view. Switch members explicitly to view their saved accounts.</p>
      </div>

      {error && (
        <div className="cc-inline-error" role="alert">
          {error}
        </div>
      )}

      <div className="cc-account-owner-bar">
        <label className="cc-field-label" htmlFor="cc-owner-select">
          View
          <select
            id="cc-owner-select"
            data-testid="cc-account-owner-select"
            value={selectedId === null ? "me" : String(selectedId)}
            onChange={(event) =>
              setSelectedId(event.target.value === "me" ? null : Number(event.target.value))
            }
          >
            <option value="me">My accounts</option>
            {members
              .filter((member) => member.id !== myId)
              .map((member) => (
                <option key={member.id} value={member.id}>
                  {member.name}
                </option>
              ))}
          </select>
        </label>
        <p>
          Owner: <strong data-testid="cc-account-owner">{owner.name}</strong>
          {owner.is_designated ? " · designated member" : ""}
        </p>
      </div>

      {readOnly && (
        <p className="cc-readonly" data-testid="cc-account-readonly" role="note">
          Read-only view · You can see these accounts and their saved activity but cannot write to them.
        </p>
      )}

      <div className="cc-account-list">
        {view?.accounts.length === 0 && <div className="empty-state">No accounts for this member yet.</div>}
        {view?.accounts.map((account) => (
          <article className="cc-account-card" key={account.id}>
            <div className="cc-account-card-top">
              <strong>{account.name}</strong>
              <span>{account.can_write ? "Editable" : "Read-only"}</span>
            </div>
            <p>{formatMinor(account.balance_minor, account.currency)}</p>
            <small>
              Opened with {formatMinor(account.opening_balance_minor, account.currency)} ·{" "}
              {account.account_type}
            </small>
          </article>
        ))}
      </div>

      {assignments?.can_assign && (
        <section className="cc-assignment-review" data-testid="cc-assignment-review">
          <h2>Assignment review</h2>
          <p className="cc-field-hint">
            Propose an owner for an existing account, then confirm the proposals once checked.
            Confirmed ownership cannot change later.
          </p>
          <ul className="cc-assignment-list">
            {assignments.accounts.map((row) => (
              <li key={row.account_id}>
                <label className="cc-assignment-row">
                  <input
                    type="checkbox"
                    data-testid="cc-assignment-select"
                    aria-label={`Confirm assignment for ${row.account_name}`}
                    checked={confirmIds.includes(row.account_id)}
                    onChange={(event) =>
                      setConfirmIds((current) =>
                        event.target.checked
                          ? [...current, row.account_id]
                          : current.filter((id) => id !== row.account_id),
                      )
                    }
                  />
                  <span>{row.account_name}</span>
                  <select
                    data-testid={`cc-assignment-owner-${row.account_id}`}
                    aria-label={`Owner for ${row.account_name}`}
                    value={ownerChoices[row.account_id] ?? ""}
                    onChange={(event) =>
                      setOwnerChoices((current) => ({
                        ...current,
                        [row.account_id]: Number(event.target.value),
                      }))
                    }
                  >
                    <option value="">Unassigned</option>
                    {members.map((member) => (
                      <option key={member.id} value={member.id}>
                        {member.name}
                      </option>
                    ))}
                  </select>
                </label>
              </li>
            ))}
          </ul>
          <div className="cc-assignment-actions">
            <button
              className="cc-button"
              data-testid="cc-assignment-propose"
              type="button"
              disabled={busy}
              onClick={() => void submitAssignments("propose")}
            >
              Propose assignments
            </button>
            <button
              className="primary-button"
              data-testid="cc-assignment-confirm"
              type="button"
              disabled={busy || confirmIds.length === 0}
              onClick={() => void submitAssignments("confirm")}
            >
              Confirm assignments
            </button>
          </div>
        </section>
      )}
    </section>
  );
}
