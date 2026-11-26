import type {
  BudgetState,
  Category,
  FilterDefinition,
  LedgerEntry,
  SavedFilter,
  TransactionKind,
} from "./types";

async function request<T>(path: string, options?: RequestInit): Promise<T> {
  const response = await fetch(`/api${path}`, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      ...options?.headers,
    },
  });

  const body = (await response.json().catch(() => null)) as
    | (T & { error?: string })
    | { error?: string }
    | null;

  if (!response.ok) {
    const message = body && "error" in body && body.error ? body.error : "Request failed";
    throw new Error(message);
  }

  return body as T;
}

function filterQuery(filter: FilterDefinition): string {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(filter)) {
    if (value !== null && value !== undefined && value !== "") {
      params.set(key, String(value));
    }
  }
  const query = params.toString();
  return query ? `?${query}` : "";
}

export const api = {
  state: () => request<BudgetState>("/state"),
  createAccount: (input: {
    name: string;
    currency: string;
    opening_balance_minor: number;
  }) =>
    request<{ id: number }>("/accounts", {
      method: "POST",
      body: JSON.stringify(input),
    }),
  accountHistory: (accountId: number) =>
    request<LedgerEntry[]>(`/accounts/${accountId}/history`),
  createTransaction: (input: {
    kind: TransactionKind;
    account_id: number;
    amount_minor: number;
    name: string;
    category_id: number | null;
    reconciled: boolean;
    note?: string;
    occurred_at?: string;
  }) =>
    request<{ id: number }>("/transactions", {
      method: "POST",
      body: JSON.stringify(input),
    }),
  updateTransaction: (
    id: number,
    input: {
      kind: TransactionKind;
      account_id: number;
      amount_minor: number;
      name: string;
      category_id: number | null;
      reconciled: boolean;
      note?: string;
      occurred_at?: string;
    },
  ) =>
    request<{ ok: boolean }>(`/transactions/${id}`, {
      method: "PUT",
      body: JSON.stringify(input),
    }),
  duplicateTransaction: (id: number) =>
    request<{ id: number }>(`/transactions/${id}/duplicate`, { method: "POST" }),
  deleteTransaction: (id: number) =>
    request<{ ok: boolean }>(`/transactions/${id}`, { method: "DELETE" }),
  reconcileTransaction: (id: number, reconciled: boolean) =>
    request<{ ok: boolean }>(`/transactions/${id}/reconciliation`, {
      method: "PATCH",
      body: JSON.stringify({ reconciled }),
    }),
  createTransfer: (input: {
    from_account_id: number;
    to_account_id: number;
    amount_minor: number;
    name: string;
    reconciled: boolean;
    note?: string;
    occurred_at?: string;
  }) =>
    request<{ id: number }>("/transfers", {
      method: "POST",
      body: JSON.stringify(input),
    }),
  updateTransfer: (
    id: number,
    input: {
      from_account_id: number;
      to_account_id: number;
      amount_minor: number;
      name: string;
      reconciled: boolean;
      note?: string;
      occurred_at?: string;
    },
  ) =>
    request<{ ok: boolean }>(`/transfers/${id}`, {
      method: "PUT",
      body: JSON.stringify(input),
    }),
  duplicateTransfer: (id: number) =>
    request<{ id: number }>(`/transfers/${id}/duplicate`, { method: "POST" }),
  deleteTransfer: (id: number) =>
    request<{ ok: boolean }>(`/transfers/${id}`, { method: "DELETE" }),
  reconcileTransfer: (id: number, reconciled: boolean) =>
    request<{ ok: boolean }>(`/transfers/${id}/reconciliation`, {
      method: "PATCH",
      body: JSON.stringify({ reconciled }),
    }),
  ledger: (filter: FilterDefinition) =>
    request<LedgerEntry[]>(`/ledger${filterQuery(filter)}`),
  categories: () => request<Category[]>("/categories"),
  createCategory: (input: {
    name: string;
    kind: TransactionKind;
    parent_id: number | null;
    color: string;
    icon: string;
    sort_order: number;
  }) =>
    request<Category>("/categories", {
      method: "POST",
      body: JSON.stringify(input),
    }),
  updateCategory: (
    id: number,
    input: {
      name: string;
      kind: TransactionKind;
      parent_id: number | null;
      color: string;
      icon: string;
      sort_order: number;
    },
  ) =>
    request<Category>(`/categories/${id}`, {
      method: "PUT",
      body: JSON.stringify(input),
    }),
  deleteCategory: (id: number) =>
    request<{ archived: boolean }>(`/categories/${id}`, { method: "DELETE" }),
  createFilter: (input: { name: string; definition: FilterDefinition }) =>
    request<SavedFilter>("/filters", {
      method: "POST",
      body: JSON.stringify(input),
    }),
  deleteFilter: (id: number) =>
    request<{ ok: boolean }>(`/filters/${id}`, { method: "DELETE" }),
};
