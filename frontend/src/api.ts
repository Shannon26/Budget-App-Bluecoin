import type { BudgetState } from "./types";

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
  createTransaction: (input: {
    kind: "income" | "expense";
    account_id: number;
    amount_minor: number;
    note?: string;
    occurred_at?: string;
  }) =>
    request<{ id: number }>("/transactions", {
      method: "POST",
      body: JSON.stringify(input),
    }),
  createTransfer: (input: {
    from_account_id: number;
    to_account_id: number;
    amount_minor: number;
    note?: string;
    occurred_at?: string;
  }) =>
    request<{ id: number }>("/transfers", {
      method: "POST",
      body: JSON.stringify(input),
    }),
};
