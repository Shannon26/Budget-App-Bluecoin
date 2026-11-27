import { readFile } from "node:fs/promises";
import { expect, test, type APIRequestContext, type Page, type Request } from "@playwright/test";
import type { BudgetState } from "../src/shared/types";

const ACCOUNT_NAME = "F29 preferred-name account";
const PREFERRED_NAME = "F29 preferred unnamed";
const EXPLICIT_NAME = "F29 explicit typed name";
const DEFAULT_NOTE = "F29 preferred transaction note";
const NOTE_FALLBACK = "F29 note fallback";
const DATE = "2026-11-20";
const CSV_HEADERS = [
  "Date",
  "Type",
  "Name",
  "Account",
  "Source Account",
  "Destination Account",
  "Category",
  "Subcategory",
  "Amount",
  "Currency",
  "Status",
  "Note",
];

function parseCsv(text: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let field = "";
  let quoted = false;

  for (let index = 0; index < text.length; index += 1) {
    const character = text[index];
    if (quoted) {
      if (character === '"') {
        if (text[index + 1] === '"') {
          field += '"';
          index += 1;
        } else {
          quoted = false;
        }
      } else {
        field += character;
      }
    } else if (character === '"') {
      quoted = true;
    } else if (character === ",") {
      row.push(field);
      field = "";
    } else if (character === "\n") {
      row.push(field);
      rows.push(row);
      row = [];
      field = "";
    } else if (character === "\r") {
      if (text[index + 1] === "\n") index += 1;
      row.push(field);
      rows.push(row);
      row = [];
      field = "";
    } else {
      field += character;
    }
  }

  if (field.length > 0 || row.length > 0) {
    row.push(field);
    rows.push(row);
  }
  return rows;
}

async function downloadLedgerCsv(page: Page): Promise<string[][]> {
  const downloadPromise = page.waitForEvent("download");
  await page.getByTestId("download-csv").click();
  const download = await downloadPromise;
  expect(download.suggestedFilename()).toBe("paisa-ledger.csv");
  const downloadPath = await download.path();
  expect(downloadPath).not.toBeNull();
  return parseCsv(await readFile(downloadPath!, "utf8"));
}

async function createAccount(
  request: APIRequestContext,
  name = ACCOUNT_NAME,
): Promise<number> {
  const response = await request.post("/api/accounts", {
    data: {
      name,
      currency: "INR",
      opening_balance_minor: 100_000,
    },
  });
  expect(response.ok()).toBeTruthy();
  return (await response.json()).id as number;
}

async function createLabel(request: APIRequestContext): Promise<number> {
  const response = await request.post("/api/labels", {
    data: { name: "F29 preserved label" },
  });
  expect(response.ok()).toBeTruthy();
  return (await response.json()).id as number;
}

async function getState(request: APIRequestContext): Promise<BudgetState> {
  const response = await request.get("/api/state");
  expect(response.ok()).toBeTruthy();
  return (await response.json()) as BudgetState;
}

function watchApiWrites(page: Page) {
  const writes: string[] = [];
  const listener = (request: Request) => {
    const url = new URL(request.url());
    if (
      url.pathname.startsWith("/api/") &&
      ["POST", "PUT", "PATCH", "DELETE"].includes(request.method())
    ) {
      writes.push(`${request.method()} ${url.pathname}`);
    }
  };
  page.on("request", listener);
  return {
    writes,
    stop: () => page.off("request", listener),
  };
}

async function transactionDraft(page: Page) {
  const form = page.locator("form").filter({ has: page.getByTestId("transaction-name") });
  return {
    name: await page.getByTestId("transaction-name").inputValue(),
    account: await page.getByTestId("transaction-account").inputValue(),
    category: await page.getByTestId("transaction-category").inputValue(),
    amount: await page.getByTestId("transaction-amount").inputValue(),
    date: await form.locator('input[type="date"]').inputValue(),
    note: await form.locator('input[placeholder="What was it for?"]').inputValue(),
    reconciled: await form.locator(".check-label input").isChecked(),
    labels: await form.locator(".label-option input:checked").evaluateAll((inputs) =>
      inputs.map((input) => input.getAttribute("data-testid")).sort(),
    ),
  };
}

async function fillTransaction(
  page: Page,
  input: {
    name?: string;
    amount: string;
    categoryId?: number;
    note?: string;
    labelId?: number;
    reconciled?: boolean;
  },
) {
  await page.getByTestId("transaction-name").fill(input.name ?? "");
  await page.getByTestId("transaction-amount").fill(input.amount);
  if (input.categoryId !== undefined) {
    await page.getByTestId("transaction-category").selectOption(String(input.categoryId));
  }
  const form = page.locator("form").filter({ has: page.getByTestId("transaction-name") });
  await form.locator('input[type="date"]').fill(DATE);
  await form.locator('input[placeholder="What was it for?"]').fill(input.note ?? "");
  if (input.labelId !== undefined) {
    await page.getByTestId(`transaction-label-${input.labelId}`).check();
  }
  if (input.reconciled) {
    await form.locator(".check-label input").check();
  }
}

test("preferred unnamed transaction name persists locally and only fills saved blank names", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);

  const accountId = await createAccount(request);
  const labelId = await createLabel(request);
  const initialState = await getState(request);
  const categoryId = initialState.categories.find(
    (category) => category.name === "Groceries",
  )!.id;

  await page.goto("/");

  await page.getByTestId("nav-overview").click();
  await expect(page.locator(`[data-account-name="${ACCOUNT_NAME}"]`)).toBeVisible();
  const preference = page.getByLabel("Preferred name for unnamed transactions", { exact: true });
  const clearPreference = page.getByRole("button", { name: "Clear preferred name", exact: true });
  await expect(clearPreference).toBeDisabled();

  const preferenceWrites = watchApiWrites(page);
  await preference.fill(PREFERRED_NAME);
  await expect(clearPreference).toBeEnabled();
  expect(preferenceWrites.writes).toEqual([]);

  await page.reload();

  await page.getByTestId("nav-overview").click();
  await expect(preference).toHaveValue(PREFERRED_NAME);
  expect(preferenceWrites.writes).toEqual([]);

  await page.getByTestId("transaction-name").fill(EXPLICIT_NAME);
  await page.getByTestId("transaction-amount").fill("10.00");
  await page.getByTestId("transaction-category").selectOption(String(categoryId));
  const form = page.locator("form").filter({ has: page.getByTestId("transaction-name") });
  await form.locator('input[type="date"]').fill(DATE);
  await form.locator('input[placeholder="What was it for?"]').fill("F29 explicit note");
  await page.getByTestId(`transaction-label-${labelId}`).check();
  await form.locator(".check-label input").check();

  const preservedDraft = await transactionDraft(page);
  await preference.fill("F29 changed preference");
  expect(await transactionDraft(page)).toEqual(preservedDraft);
  await clearPreference.click();
  await expect(preference).toHaveValue("");
  expect(await transactionDraft(page)).toEqual(preservedDraft);
  await preference.fill(PREFERRED_NAME);

  await page.getByTestId("nav-ledger").click();
  await page.getByTestId("nav-overview").click();
  expect(await transactionDraft(page)).toEqual(preservedDraft);
  expect(preferenceWrites.writes).toEqual([]);
  preferenceWrites.stop();

  await page.getByRole("button", { name: "Record expense", exact: true }).click();
  await expect(page.getByTestId("transaction-name")).toHaveValue("");
  await expect(page.getByTestId("transaction-amount")).toHaveValue("");
  await expect(preference).toHaveValue(PREFERRED_NAME);

  const afterExplicit = await getState(request);
  const explicitEntry = afterExplicit.entries.find((entry) => entry.name === EXPLICIT_NAME)!;
  expect(explicitEntry).toMatchObject({
    kind: "expense",
    account_id: accountId,
    amount_minor: 1_000,
    category_id: categoryId,
    note: "F29 explicit note",
    occurred_at: `${DATE}T12:00:00`,
    reconciled: true,
    labels: [{ id: labelId, name: "F29 preserved label" }],
  });
  expect(afterExplicit.accounts.find((account) => account.id === accountId)?.balance_minor).toBe(99_000);

  await fillTransaction(page, {
    amount: "25.00",
    categoryId,
    note: DEFAULT_NOTE,
    labelId,
    reconciled: true,
  });
  await page.getByRole("button", { name: "Record expense", exact: true }).click();
  await expect(page.getByTestId("transaction-name")).toHaveValue("");
  await expect(page.getByTestId("transaction-amount")).toHaveValue("");

  const afterPreferred = await getState(request);
  const preferredEntry = afterPreferred.entries.find((entry) => entry.name === PREFERRED_NAME)!;
  expect(preferredEntry).toMatchObject({
    kind: "expense",
    account_id: accountId,
    amount_minor: 2_500,
    category_id: categoryId,
    note: DEFAULT_NOTE,
    occurred_at: `${DATE}T12:00:00`,
    reconciled: true,
    labels: [{ id: labelId, name: "F29 preserved label" }],
  });
  expect(afterPreferred.accounts.find((account) => account.id === accountId)?.balance_minor).toBe(96_500);
  expect(afterPreferred.totals).toEqual({ income_minor: 0, expense_minor: 3_500 });

  await page.getByTestId("nav-ledger").click();
  const preferredRow = page
    .locator('[data-testid^="ledger-row-expense-"]')
    .filter({ hasText: PREFERRED_NAME });
  await expect(preferredRow).toHaveCount(1);
  await preferredRow.click();
  const details = page.getByRole("dialog", { name: "Entry details" });
  await expect(details.getByRole("heading", { name: PREFERRED_NAME, exact: true })).toBeVisible();
  await expect(details).toContainText(DEFAULT_NOTE);
  await expect(details).toContainText("Food & Drink / Groceries");
  await expect(details).toContainText("F29 preserved label");
  await expect(details).toContainText("Reconciled");
  await details.getByRole("button", { name: "Close details" }).click();

  await page.getByTestId("nav-overview").click();
  const clearWrites = watchApiWrites(page);
  await clearPreference.click();
  await expect(preference).toHaveValue("");
  expect(clearWrites.writes).toEqual([]);
  await page.reload();
  await page.getByTestId("nav-overview").click();
  await expect(preference).toHaveValue("");
  expect(clearWrites.writes).toEqual([]);
  clearWrites.stop();

  await fillTransaction(page, { amount: "5.00", note: NOTE_FALLBACK });
  await page.getByRole("button", { name: "Record expense", exact: true }).click();
  await expect(page.getByTestId("transaction-name")).toHaveValue("");
  await expect(page.getByTestId("transaction-amount")).toHaveValue("");

  await page.getByRole("button", { name: "Income", exact: true }).click();
  await fillTransaction(page, { amount: "20.00" });
  await page.getByRole("button", { name: "Record income", exact: true }).click();
  await expect(page.getByTestId("transaction-name")).toHaveValue("");
  await expect(page.getByTestId("transaction-amount")).toHaveValue("");

  const finalState = await getState(request);
  expect(finalState.entries.find((entry) => entry.name === NOTE_FALLBACK)).toMatchObject({
    kind: "expense",
    amount_minor: 500,
    note: NOTE_FALLBACK,
  });
  expect(finalState.entries.find((entry) => entry.name === "income")).toMatchObject({
    kind: "income",
    amount_minor: 2_000,
    note: "",
  });
  expect(finalState.accounts.find((account) => account.id === accountId)?.balance_minor).toBe(98_000);
  expect(finalState.totals).toEqual({ income_minor: 2_000, expense_minor: 4_000 });
  await expect(page.locator(`[data-account-name="${ACCOUNT_NAME}"]`).getByTestId("account-balance")).toContainText("980.00");
  await expect(page.getByTestId("income-total")).toContainText("20.00");
  await expect(page.getByTestId("expense-total")).toContainText("40.00");

  await page.getByTestId("nav-ledger").click();
  const exported = await downloadLedgerCsv(page);
  expect(exported[0]).toEqual(CSV_HEADERS);
  expect(exported.slice(1)).toHaveLength(4);
  expect(exported.slice(1).find((row) => row[2] === PREFERRED_NAME)).toEqual([
    DATE,
    "Expense",
    PREFERRED_NAME,
    ACCOUNT_NAME,
    "",
    "",
    "Food & Drink",
    "Groceries",
    "25.00",
    "INR",
    "Reconciled",
    DEFAULT_NOTE,
  ]);
  expect(exported.slice(1).find((row) => row[2] === EXPLICIT_NAME)?.[8]).toBe("10.00");
  expect(exported.slice(1).find((row) => row[2] === NOTE_FALLBACK)?.[8]).toBe("5.00");
  expect(exported.slice(1).find((row) => row[2] === "income")?.[8]).toBe("20.00");
});

test("unavailable browser storage leaves the preferred name usable for the session", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);

  await page.addInitScript(() => {
    const denyStorage = () => {
      throw new DOMException("Storage is unavailable", "SecurityError");
    };
    for (const method of ["getItem", "setItem", "removeItem"] as const) {
      Object.defineProperty(Storage.prototype, method, {
        configurable: true,
        value: denyStorage,
      });
    }
  });

  const storageAccountName = "F29 storage-unavailable account";
  const accountId = await createAccount(request, storageAccountName);
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));

  await page.goto("/");

  await page.getByTestId("nav-overview").click();
  await expect(page.locator(`[data-account-name="${storageAccountName}"]`)).toBeVisible();
  await page.getByTestId("transaction-account").selectOption(String(accountId));
  const preference = page.getByLabel("Preferred name for unnamed transactions", { exact: true });
  const sessionPreference = "F29 session-only preference";
  const writes = watchApiWrites(page);
  await preference.fill(sessionPreference);
  await expect(preference).toHaveValue(sessionPreference);
  expect(writes.writes).toEqual([]);

  await fillTransaction(page, { amount: "1.23", note: "F29 storage unavailable note" });
  await page.getByRole("button", { name: "Record expense", exact: true }).click();
  await expect(page.getByTestId("transaction-name")).toHaveValue("");
  await expect(page.getByTestId("transaction-amount")).toHaveValue("");

  const afterSessionDefault = await getState(request);
  expect(afterSessionDefault.entries.find((entry) => entry.name === sessionPreference)).toMatchObject({
    kind: "expense",
    account_id: accountId,
    amount_minor: 123,
    note: "F29 storage unavailable note",
  });
  expect(afterSessionDefault.accounts.find((account) => account.id === accountId)?.balance_minor).toBe(99_877);

  await page.getByRole("button", { name: "Clear preferred name", exact: true }).click();
  await expect(preference).toHaveValue("");
  await fillTransaction(page, { amount: "2.34" });
  await page.getByRole("button", { name: "Record expense", exact: true }).click();
  await expect(page.getByTestId("transaction-name")).toHaveValue("");
  await expect(page.getByTestId("transaction-amount")).toHaveValue("");

  const afterClear = await getState(request);
  expect(afterClear.entries.find((entry) => entry.name === "expense")).toMatchObject({
    kind: "expense",
    amount_minor: 234,
    note: "",
  });
  expect(afterClear.accounts.find((account) => account.id === accountId)?.balance_minor).toBe(99_643);
  expect(writes.writes).toEqual(["POST /api/transactions", "POST /api/transactions"]);
  writes.stop();
  expect(pageErrors).toEqual([]);

  await page.reload();

  await page.getByTestId("nav-overview").click();
  await expect(preference).toHaveValue("");
  await expect(page.locator(`[data-account-name="${storageAccountName}"]`)).toBeVisible();
});
