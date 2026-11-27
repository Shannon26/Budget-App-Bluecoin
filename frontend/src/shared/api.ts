import type {
  AccountMetadata,
  AccountUpdate,
  AssignmentSelection,
  AssignmentView,
  BalanceSheetReport,
  BudgetState,
  Category,
  CashFlowTrendReport,
  CreditCardReport,
  DailySummaryReport,
  DecisionResponse,
  Draft,
  DraftResponse,
  DraftRow,
  FilterDefinition,
  HouseholdAccountView,
  ItemsSummaryReport,
  Label,
  LabelReport,
  LedgerEntry,
  LedgerPage,
  Member,
  MonthlyCategoryBudgets,
  NetEarningsReport,
  OkResponse,
  PromptRequest,
  SaveReceipt,
  SavedFilter,
  TransactionKind,
  TransferRequestRecord,
  TransferRequestsResponse,
} from "./types";

const runtimeEnv = (import.meta as unknown as { env?: Record<string, string | undefined> }).env ?? {};

/** Member headers are only sent in household mode; single-member installs stay unchanged. */
function householdHeaders(): Record<string, string> {
  if (runtimeEnv.VITE_PAISA_HOUSEHOLD_ENABLED !== "true") return {};
  let memberId = "1";
  let token = runtimeEnv.VITE_PAISA_MEMBER_TOKEN ?? "";
  try {
    if (typeof localStorage !== "undefined") {
      memberId = localStorage.getItem("paisa.memberId") || memberId;
      token = localStorage.getItem("paisa.memberToken") || token;
    }
  } catch {
    // Storage can be blocked; fall back to configured defaults.
  }
  return {
    "x-paisa-member": memberId,
    "x-paisa-member-token": token,
  };
}

async function request<T>(path: string, options?: RequestInit): Promise<T> {
  const response = await fetch(`/api${path}`, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      ...householdHeaders(),
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
  const response = await fetch(`/api${path}`, { headers: householdHeaders() });
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
  labelReport: (month: string) =>
    request<LabelReport>(`/reports/labels?month=${encodeURIComponent(month)}`),
  dailySummary: (anchorDate: string) =>
    request<DailySummaryReport>(`/reports/daily-summary?anchor_date=${encodeURIComponent(anchorDate)}`),
  balanceSheet: (month: string) =>
    request<BalanceSheetReport>(`/reports/balance-sheet?month=${encodeURIComponent(month)}`),
  cashFlowTrend: (month: string, months: number) =>
    request<CashFlowTrendReport>(`/reports/cash-flow-trend?month=${encodeURIComponent(month)}&months=${months}`),
  creditCard: (accountId: number, anchorDate: string) =>
    request<CreditCardReport>(
      `/reports/credit-card?account_id=${accountId}&anchor_date=${encodeURIComponent(anchorDate)}`,
    ),
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
    labels?: number[];
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
      labels?: number[];
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
    labels?: number[];
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
      labels?: number[];
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
  setOverallBudget: (month: string, amountMinor: number) =>
    request<MonthlyCategoryBudgets>(`/overall-budget?month=${encodeURIComponent(month)}`, {
      method: "PUT",
      body: JSON.stringify({ amount_minor: amountMinor }),
    }),
  removeOverallBudget: (month: string) =>
    request<MonthlyCategoryBudgets>(`/overall-budget?month=${encodeURIComponent(month)}`, {
      method: "DELETE",
    }),
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
  labels: () => request<Label[]>("/labels"),
  createLabel: (name: string) =>
    request<Label>("/labels", { method: "POST", body: JSON.stringify({ name }) }),
  updateLabel: (id: number, name: string) =>
    request<Label>(`/labels/${id}`, { method: "PUT", body: JSON.stringify({ name }) }),
  createFilter: (input: { name: string; definition: FilterDefinition }) =>
    request<SavedFilter>("/filters", {
      method: "POST",
      body: JSON.stringify(input),
    }),
  updateFilter: (id: number, input: { name: string; definition: FilterDefinition }) =>
    request<SavedFilter>(`/filters/${id}`, {
      method: "PUT",
      body: JSON.stringify(input),
    }),
  deleteFilter: (id: number) =>
    request<{ ok: boolean }>(`/filters/${id}`, { method: "DELETE" }),

  // Command Center: private drafts and reviewed batch save.
  draft: () => request<Draft>("/draft"),
  appendPrompt: (text: string, clientPromptId: string, localDate?: string) =>
    request<DraftResponse>("/draft/prompt", {
      method: "POST",
      body: JSON.stringify({
        text,
        client_prompt_id: clientPromptId,
        local_date: localDate,
      } satisfies PromptRequest),
    }),
  addDraftRow: (row: DraftRow) =>
    request<Draft>("/draft/rows", { method: "POST", body: JSON.stringify(row) }),
  replaceDraftRow: (rowId: number | string, row: DraftRow) =>
    request<Draft>(`/draft/rows/${encodeURIComponent(String(rowId))}`, {
      method: "PUT",
      body: JSON.stringify(row),
    }),
  removeDraftRow: (rowId: string) =>
    request<Draft>(`/draft/rows/${encodeURIComponent(rowId)}`, { method: "DELETE" }),
  assignDraftAccount: (rowIds: string[], accountId: number) =>
    request<Draft>("/draft/rows/assign-account", {
      method: "POST",
      body: JSON.stringify({ row_ids: rowIds, account_id: accountId }),
    }),
  rememberAlias: (alias: string, accountId?: number, proposedRowId?: string) =>
    request<OkResponse>("/draft/remember-alias", {
      method: "POST",
      body: JSON.stringify({
        alias,
        account_id: accountId,
        proposed_row_id: proposedRowId,
      }),
    }),
  discardDraft: () => request<OkResponse>("/draft/discard", { method: "POST" }),
  saveDraft: (revision: number) =>
    request<SaveReceipt>("/draft/save", {
      method: "POST",
      body: JSON.stringify({ revision }),
    }),

  // Transfer requests and posted cross-member reversals.
  transferRequests: () => request<TransferRequestsResponse>("/transfer-requests"),
  approveRequest: (id: number) =>
    request<DecisionResponse>(`/transfer-requests/${id}/approve`, { method: "POST" }),
  declineRequest: (id: number) =>
    request<DecisionResponse>(`/transfer-requests/${id}/decline`, { method: "POST" }),
  cancelRequest: (id: number) =>
    request<DecisionResponse>(`/transfer-requests/${id}/cancel`, { method: "POST" }),
  reverseTransfer: (id: number) =>
    request<TransferRequestRecord>(`/transfers/${id}/reverse`, { method: "POST" }),

  // Household views and ownership assignment.
  householdMembers: () => request<Member[]>("/household/members"),
  householdAccounts: (memberId?: number) =>
    request<HouseholdAccountView>(
      `/household/accounts${memberId === undefined ? "" : `?member_id=${encodeURIComponent(String(memberId))}`}`,
    ),
  householdAssignments: () => request<AssignmentView>("/household/assignments"),
  proposeAssignments: (proposals: AssignmentSelection[]) =>
    request<AssignmentView>("/household/assignments/propose", {
      method: "POST",
      body: JSON.stringify({ proposals }),
    }),
  confirmAssignments: (assignments: AssignmentSelection[]) =>
    request<OkResponse>("/household/assignments/confirm", {
      method: "POST",
      body: JSON.stringify({ assignments }),
    }),

  transactionDetail: (id: number) => request<LedgerEntry>(`/transactions/${id}`),
  transferDetail: (id: number) => request<LedgerEntry>(`/transfers/${id}`),
};
