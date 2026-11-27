import type {
  AccountMetadata,
  AccountUpdate,
  BalanceSheetReport,
  BudgetState,
  Category,
  DailySummaryReport,
  FilterDefinition,
  ItemsSummaryReport,
  LedgerEntry,
  LedgerPage,
  MonthlyCategoryBudgets,
  NetEarningsReport,
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

function ledgerPath(filter: FilterDefinition, offset: number): string {
  const query = filterQuery(filter);
  return `/ledger${query}${query ? "&" : "?"}limit=200&offset=${offset}`;
}

async function downloadCsv(path: string): Promise<Blob> {
  const response = await fetch(`/api${path}`);
  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as { error?: string } | null;
    throw new Error(body?.error || "Could not export ledger");
  }
  return response.blob();
}

export const api = {
  state: () => request<BudgetState>("/state"),
  netEarnings: (month: string) =>
    request<NetEarningsReport>(`/reports/net-earnings?month=${encodeURIComponent(month)}`),
  itemsSummary: (month: string) =>
    request<ItemsSummaryReport>(`/reports/items-summary?month=${encodeURIComponent(month)}`),
  dailySummary: (anchorDate: string) =>
    request<DailySummaryReport>(`/reports/daily-summary?anchor_date=${encodeURIComponent(anchorDate)}`),
  balanceSheet: (month: string) =>
    request<BalanceSheetReport>(`/reports/balance-sheet?month=${encodeURIComponent(month)}`),
  createAccount: (input: {
    name: string;
    currency: string;
    opening_balance_minor: number;
    account_type?: string;
    note?: string | null;
    opening_date?: string | null;
    hide_from_selection?: boolean;
    hide_from_reports?: boolean;
    credit_limit_minor?: number | null;
    cutoff_day?: number | null;
    due_day?: number | null;
  }) =>
    request<AccountMetadata>("/accounts", {
      method: "POST",
      body: JSON.stringify(input),
    }),
  updateAccount: (id: number, input: AccountUpdate) =>
    request<AccountMetadata>(`/accounts/${id}`, {
      method: "PUT",
      body: JSON.stringify(input),
    }),
  updateAccountType: (id: number, accountType: string) =>
    request<AccountMetadata>(`/accounts/${id}`, {
      method: "PUT",
      body: JSON.stringify({ account_type: accountType }),
    }),
  accountHistory: (accountId: number, offset = 0) =>
    request<LedgerPage>(`/accounts/${accountId}/history?limit=200&offset=${offset}`),
  invalidDateExpenses: (offset = 0) =>
    request<LedgerPage>(`/ledger/invalid-date-expenses?limit=200&offset=${offset}`),
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
  ledger: (filter: FilterDefinition, offset = 0) =>
    request<LedgerPage>(ledgerPath(filter, offset)),
  downloadLedgerCsv: (filter: FilterDefinition) =>
    downloadCsv(`/ledger/export${filterQuery(filter)}`),
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
  categoryBudgets: (month: string) =>
    request<MonthlyCategoryBudgets>(`/category-budgets?month=${encodeURIComponent(month)}`),
  setCategoryBudget: (categoryId: number, month: string, amountMinor: number) =>
    request<MonthlyCategoryBudgets>(
      `/category-budgets/${categoryId}?month=${encodeURIComponent(month)}`,
      {
        method: "PUT",
        body: JSON.stringify({ amount_minor: amountMinor }),
      },
    ),
  removeCategoryBudget: (categoryId: number, month: string) =>
    request<MonthlyCategoryBudgets>(
      `/category-budgets/${categoryId}?month=${encodeURIComponent(month)}`,
      { method: "DELETE" },
    ),
  createFilter: (input: { name: string; definition: FilterDefinition }) =>
    request<SavedFilter>("/filters", {
      method: "POST",
      body: JSON.stringify(input),
    }),
  deleteFilter: (id: number) =>
    request<{ ok: boolean }>(`/filters/${id}`, { method: "DELETE" }),
};
