import { expect, test, type APIRequestContext, type Page, type Request } from "@playwright/test";
import type { BudgetState } from "../src/shared/types";

const DATE = "2026-11-01";

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

async function createAccount(
  request: APIRequestContext,
  name: string,
  openingBalanceMinor: number,
  hideFromSelection = false,
): Promise<number> {
  const response = await request.post("/api/accounts", {
    data: {
      name,
      currency: "INR",
      opening_balance_minor: openingBalanceMinor,
      hide_from_selection: hideFromSelection,
    },
  });
  expect(response.ok()).toBeTruthy();
  return (await response.json()).id as number;
}

async function createLabel(request: APIRequestContext, name: string): Promise<number> {
  const response = await request.post("/api/labels", { data: { name } });
  expect(response.ok()).toBeTruthy();
  return (await response.json()).id as number;
}

async function getState(request: APIRequestContext): Promise<BudgetState> {
  const response = await request.get("/api/state");
  expect(response.ok()).toBeTruthy();
  return (await response.json()) as BudgetState;
}

async function groceriesId(request: APIRequestContext): Promise<number> {
  const state = await getState(request);
  return state.categories.find((category) => category.name === "Groceries")!.id;
}

function clearSetting(page: Page, field: string) {
  return page.getByTestId(`clear-after-save-${field}`);
}

function saveAddButton(page: Page) {
  return page.getByTestId("save-add-another");
}

function normalSaveButton(page: Page) {
  return page
    .locator("form")
    .filter({ has: page.getByTestId("transaction-name") })
    .locator('button[type="submit"]');
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

async function transactionSnapshot(page: Page) {
  const form = page.locator("form").filter({ has: page.getByTestId("transaction-name") });
  return {
    kind: (await page.locator(".segmented button.active").textContent())?.trim(),
    name: await page.getByTestId("transaction-name").inputValue(),
    account: await page.getByTestId("transaction-account").inputValue(),
    category: await page.getByTestId("transaction-category").inputValue(),
    amount: await page.getByTestId("transaction-amount").inputValue(),
    date: await form.locator('input[type="date"]').inputValue(),
    note: await form.locator('input[placeholder="What was it for?"]').inputValue(),
    reconciled: await form.getByLabel("Reconciled", { exact: true }).isChecked(),
    labels: await form.locator(".label-option input:checked").evaluateAll((inputs) =>
      inputs.map((input) => input.getAttribute("data-testid")).sort(),
    ),
  };
}

async function fillTransaction(
  page: Page,
  input: {
    name?: string;
    accountId?: number;
    categoryId?: number;
    amount: string;
    date?: string;
    note?: string;
    labelId?: number;
    reconciled?: boolean;
  },
) {
  await page.getByTestId("transaction-name").fill(input.name ?? "");
  if (input.accountId !== undefined) {
    await page.getByTestId("transaction-account").selectOption(String(input.accountId));
  }
  if (input.categoryId !== undefined) {
    await page.getByTestId("transaction-category").selectOption(String(input.categoryId));
  }
  await page.getByTestId("transaction-amount").fill(input.amount);
  const form = page.locator("form").filter({ has: page.getByTestId("transaction-name") });
  await form.locator('input[type="date"]').fill(input.date ?? DATE);
  await form.locator('input[placeholder="What was it for?"]').fill(input.note ?? "");
  if (input.labelId !== undefined) {
    await page.getByTestId(`transaction-label-${input.labelId}`).check();
  }
  if (input.reconciled) {
    await form.getByLabel("Reconciled", { exact: true }).check();
  }
}

async function localToday(page: Page): Promise<string> {
  return page.evaluate(() => {
    const now = new Date();
    return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(
      now.getDate(),
    ).padStart(2, "0")}`;
  });
}

test("save and add another persists settings and clears only selected fields", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);

  const hiddenId = await createAccount(request, "F30 hidden account", 50_000, true);
  const primaryId = await createAccount(request, "F30 primary account", 100_000);
  const secondaryId = await createAccount(request, "F30 secondary account", 50_000);
  const labelId = await createLabel(request, "F30 retained label");
  const categoryId = await groceriesId(request);

  await page.goto("/");
  await expect(page.locator('[data-account-name="F30 primary account"]')).toBeVisible();
  await expect(page.getByTestId("transaction-account").locator(`option[value="${hiddenId}"]`)).toHaveCount(0);
  await expect(clearSetting(page, "name")).toBeChecked();
  await expect(clearSetting(page, "amount")).toBeChecked();
  await expect(clearSetting(page, "note")).toBeChecked();
  await expect(clearSetting(page, "labels")).toBeChecked();
  await expect(clearSetting(page, "reconciled")).toBeChecked();
  await expect(clearSetting(page, "account")).not.toBeChecked();
  await expect(clearSetting(page, "category")).not.toBeChecked();
  await expect(clearSetting(page, "date")).not.toBeChecked();

  const writes = watchApiWrites(page);
  await page.getByLabel("Preferred name for unnamed transactions", { exact: true }).fill(
    "F30 preferred unnamed",
  );
  await clearSetting(page, "note").uncheck();
  await clearSetting(page, "labels").uncheck();
  await clearSetting(page, "reconciled").uncheck();
  await clearSetting(page, "account").check();
  await clearSetting(page, "date").check();
  expect(writes.writes).toEqual([]);

  const today = await localToday(page);
  await page.reload();
  await expect(page.getByLabel("Preferred name for unnamed transactions", { exact: true })).toHaveValue(
    "F30 preferred unnamed",
  );
  for (const field of ["name", "amount", "account", "date"]) {
    await expect(clearSetting(page, field)).toBeChecked();
  }
  for (const field of ["category", "note", "labels", "reconciled"]) {
    await expect(clearSetting(page, field)).not.toBeChecked();
  }
  expect(writes.writes).toEqual([]);
  writes.stop();

  await fillTransaction(page, { name: "Preserved draft", accountId: primaryId, amount: "12.00" });
  const form = page.locator("form").filter({ has: page.getByTestId("transaction-name") });
  await form.locator('input[type="date"]').fill("");
  const validationWrites = watchApiWrites(page);
  await saveAddButton(page).click();
  expect(await form.locator('input[type="date"]').evaluate((input: HTMLInputElement) => input.validity.valueMissing)).toBe(true);
  await expect(page.getByTestId("transaction-name")).toHaveValue("Preserved draft");
  expect(validationWrites.writes).toEqual([]);
  validationWrites.stop();

  await fillTransaction(page, {
    name: "F30 first batch entry",
    accountId: secondaryId,
    categoryId,
    amount: "12.34",
    date: DATE,
    note: "F30 retained note",
    labelId,
    reconciled: true,
  });
  await saveAddButton(page).click();
  await expect(page.getByTestId("transaction-amount")).toHaveValue("");
  await expect(saveAddButton(page)).toBeEnabled();

  expect(await transactionSnapshot(page)).toEqual({
    kind: "Expense",
    name: "",
    account: String(primaryId),
    category: String(categoryId),
    amount: "",
    date: today,
    note: "F30 retained note",
    reconciled: true,
    labels: [`transaction-label-${labelId}`],
  });

  const afterFirst = await getState(request);
  expect(afterFirst.entries.find((entry) => entry.name === "F30 first batch entry")).toMatchObject({
    kind: "expense",
    account_id: secondaryId,
    category_id: categoryId,
    amount_minor: 1_234,
    note: "F30 retained note",
    occurred_at: `${DATE}T12:00:00`,
    reconciled: true,
    labels: [{ id: labelId, name: "F30 retained label" }],
  });

  await page.getByTestId("transaction-amount").fill("5.67");
  await saveAddButton(page).click();
  await expect(page.getByTestId("transaction-amount")).toHaveValue("");
  await expect(saveAddButton(page)).toBeEnabled();
  expect(await transactionSnapshot(page)).toEqual({
    kind: "Expense",
    name: "",
    account: String(primaryId),
    category: String(categoryId),
    amount: "",
    date: today,
    note: "F30 retained note",
    reconciled: true,
    labels: [`transaction-label-${labelId}`],
  });

  const afterSecond = await getState(request);
  expect(afterSecond.entries.find((entry) => entry.name === "F30 preferred unnamed")).toMatchObject({
    kind: "expense",
    account_id: primaryId,
    category_id: categoryId,
    amount_minor: 567,
    note: "F30 retained note",
    occurred_at: `${today}T12:00:00`,
    reconciled: true,
    labels: [{ id: labelId, name: "F30 retained label" }],
  });
  expect(afterSecond.accounts.find((account) => account.id === primaryId)?.balance_minor).toBe(99_433);
  expect(afterSecond.accounts.find((account) => account.id === secondaryId)?.balance_minor).toBe(48_766);
  expect(afterSecond.accounts.find((account) => account.id === hiddenId)?.balance_minor).toBe(50_000);
  expect(afterSecond.totals).toEqual({ income_minor: 0, expense_minor: 1_801 });

  const beforeRestore = await transactionSnapshot(page);
  await page.getByTestId("reset-clear-after-save").click();
  for (const field of ["name", "amount", "note", "labels", "reconciled"]) {
    await expect(clearSetting(page, field)).toBeChecked();
  }
  for (const field of ["account", "category", "date"]) {
    await expect(clearSetting(page, field)).not.toBeChecked();
  }
  expect(await transactionSnapshot(page)).toEqual(beforeRestore);
});

test("ordinary save keeps its existing reset behavior regardless of clearing settings", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);

  const primaryId = await createAccount(request, "F30 normal primary", 100_000);
  const secondaryId = await createAccount(request, "F30 normal secondary", 100_000);
  const labelId = await createLabel(request, "F30 normal label");
  const categoryId = await groceriesId(request);

  await page.goto("/");
  await page.getByLabel("Preferred name for unnamed transactions", { exact: true }).fill(
    "F30 normal preferred",
  );
  for (const field of ["name", "amount", "note", "labels", "reconciled"]) {
    await clearSetting(page, field).uncheck();
  }
  for (const field of ["account", "category", "date"]) {
    await clearSetting(page, field).check();
  }

  await fillTransaction(page, {
    name: "F30 normal explicit",
    accountId: secondaryId,
    categoryId,
    amount: "10.00",
    date: DATE,
    note: "F30 normal note",
    labelId,
    reconciled: true,
  });
  await normalSaveButton(page).click();
  await expect(page.getByTestId("transaction-name")).toHaveValue("");
  await expect(page.getByTestId("transaction-amount")).toHaveValue("");
  await expect(normalSaveButton(page)).toBeEnabled();

  expect(await transactionSnapshot(page)).toEqual({
    kind: "Expense",
    name: "",
    account: String(secondaryId),
    category: String(categoryId),
    amount: "",
    date: DATE,
    note: "",
    reconciled: false,
    labels: [],
  });
  await expect(clearSetting(page, "name")).not.toBeChecked();
  await expect(clearSetting(page, "account")).toBeChecked();

  const afterExplicit = await getState(request);
  expect(afterExplicit.entries.find((entry) => entry.name === "F30 normal explicit")).toMatchObject({
    account_id: secondaryId,
    category_id: categoryId,
    amount_minor: 1_000,
    note: "F30 normal note",
    occurred_at: `${DATE}T12:00:00`,
    reconciled: true,
    labels: [{ id: labelId, name: "F30 normal label" }],
  });
  expect(afterExplicit.accounts.find((account) => account.id === secondaryId)?.balance_minor).toBe(99_000);

  await page.getByTestId("transaction-amount").fill("2.00");
  await normalSaveButton(page).click();
  await expect(page.getByTestId("transaction-amount")).toHaveValue("");
  await expect(normalSaveButton(page)).toBeEnabled();
  const afterPreferred = await getState(request);
  expect(afterPreferred.entries.find((entry) => entry.name === "F30 normal preferred")).toMatchObject({
    account_id: secondaryId,
    category_id: categoryId,
    amount_minor: 200,
    occurred_at: `${DATE}T12:00:00`,
  });
  expect(afterPreferred.accounts.find((account) => account.id === secondaryId)?.balance_minor).toBe(98_800);
  expect(afterPreferred.accounts.find((account) => account.id === primaryId)?.balance_minor).toBe(100_000);
});

test("failed and in-flight saves preserve drafts and both actions deduplicate requests", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);

  const accountId = await createAccount(request, "F30 dedup account", 100_000);
  const labelId = await createLabel(request, "F30 dedup label");
  const categoryId = await groceriesId(request);
  const baselineEntryCount = (await getState(request)).entries.length;

  await page.goto("/");
  await fillTransaction(page, {
    name: "F30 failure draft",
    accountId,
    categoryId,
    amount: "4.56",
    date: DATE,
    note: "F30 failure note",
    labelId,
    reconciled: true,
  });
  const failureDraft = await transactionSnapshot(page);
  let failureCalls = 0;
  await page.route("**/api/transactions", async (route) => {
    if (route.request().method() !== "POST") {
      await route.continue();
      return;
    }
    failureCalls += 1;
    await route.fulfill({
      status: 500,
      contentType: "application/json",
      body: JSON.stringify({ error: "F30 forced save failure" }),
    });
  });
  await saveAddButton(page).click();
  await expect(page.getByRole("alert")).toContainText("F30 forced save failure");
  expect(failureCalls).toBe(1);
  expect(await transactionSnapshot(page)).toEqual(failureDraft);
  expect((await getState(request)).entries).toHaveLength(baselineEntryCount);
  expect(
    (await getState(request)).entries.find((entry) => entry.name === "F30 failure draft"),
  ).toBeUndefined();
  await page.unroute("**/api/transactions");

  let normalCalls = 0;
  const normalStarted = deferred();
  const releaseNormal = deferred();
  await page.route("**/api/transactions", async (route) => {
    if (route.request().method() !== "POST") {
      await route.continue();
      return;
    }
    normalCalls += 1;
    normalStarted.resolve();
    await releaseNormal.promise;
    await route.continue();
  });
  await normalSaveButton(page).evaluate((button) => {
    (button as HTMLButtonElement).click();
    (button as HTMLButtonElement).click();
  });
  await normalStarted.promise;
  expect(normalCalls).toBe(1);
  await expect(normalSaveButton(page)).toBeDisabled();
  await expect(saveAddButton(page)).toBeDisabled();
  expect(await transactionSnapshot(page)).toEqual(failureDraft);
  releaseNormal.resolve();
  await expect(page.getByTestId("transaction-name")).toHaveValue("");
  await expect(normalSaveButton(page)).toBeEnabled();
  await page.unroute("**/api/transactions");
  expect((await getState(request)).entries).toHaveLength(baselineEntryCount + 1);

  await fillTransaction(page, {
    name: "F30 add-another draft",
    accountId,
    categoryId,
    amount: "7.89",
    date: DATE,
    note: "F30 dedup note",
    labelId,
    reconciled: true,
  });
  const addAnotherDraft = await transactionSnapshot(page);
  let addAnotherCalls = 0;
  const addAnotherStarted = deferred();
  const releaseAddAnother = deferred();
  await page.route("**/api/transactions", async (route) => {
    if (route.request().method() !== "POST") {
      await route.continue();
      return;
    }
    addAnotherCalls += 1;
    addAnotherStarted.resolve();
    await releaseAddAnother.promise;
    await route.continue();
  });
  await saveAddButton(page).evaluate((button) => {
    (button as HTMLButtonElement).click();
    (button as HTMLButtonElement).click();
  });
  await addAnotherStarted.promise;
  expect(addAnotherCalls).toBe(1);
  await expect(normalSaveButton(page)).toBeDisabled();
  await expect(saveAddButton(page)).toBeDisabled();
  expect(await transactionSnapshot(page)).toEqual(addAnotherDraft);
  releaseAddAnother.resolve();
  await expect(page.getByTestId("transaction-name")).toHaveValue("");
  await expect(page.getByTestId("transaction-amount")).toHaveValue("");
  await expect(saveAddButton(page)).toBeEnabled();
  await page.unroute("**/api/transactions");

  const finalState = await getState(request);
  expect(finalState.entries).toHaveLength(baselineEntryCount + 2);
  expect(finalState.entries.find((entry) => entry.name === "F30 failure draft")).toMatchObject({
    amount_minor: 456,
    category_id: categoryId,
    note: "F30 failure note",
    reconciled: true,
    labels: [{ id: labelId, name: "F30 dedup label" }],
  });
  expect(finalState.entries.find((entry) => entry.name === "F30 add-another draft")).toMatchObject({
    amount_minor: 789,
    category_id: categoryId,
    note: "F30 dedup note",
    reconciled: true,
    labels: [{ id: labelId, name: "F30 dedup label" }],
  });
  expect(finalState.accounts.find((account) => account.id === accountId)?.balance_minor).toBe(98_755);
});

test("clearing settings remain usable in session when browser storage is unavailable", async ({
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

  const accountId = await createAccount(request, "F30 storage account", 100_000);
  const categoryId = await groceriesId(request);
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));

  await page.goto("/");
  await page.getByLabel("Preferred name for unnamed transactions", { exact: true }).fill(
    "F30 session preferred",
  );
  await clearSetting(page, "name").uncheck();
  await clearSetting(page, "amount").uncheck();
  await clearSetting(page, "category").check();
  await fillTransaction(page, {
    accountId,
    categoryId,
    amount: "3.21",
    date: DATE,
    note: "F30 storage note",
  });
  await saveAddButton(page).click();
  await expect(saveAddButton(page)).toBeEnabled();

  expect(await transactionSnapshot(page)).toEqual({
    kind: "Expense",
    name: "",
    account: String(accountId),
    category: "",
    amount: "3.21",
    date: DATE,
    note: "",
    reconciled: false,
    labels: [],
  });
  const state = await getState(request);
  expect(state.entries.find((entry) => entry.name === "F30 session preferred")).toMatchObject({
    account_id: accountId,
    category_id: categoryId,
    amount_minor: 321,
    note: "F30 storage note",
    occurred_at: `${DATE}T12:00:00`,
  });
  expect(state.accounts.find((account) => account.id === accountId)?.balance_minor).toBe(99_679);
  expect(pageErrors).toEqual([]);

  await page.reload();
  await expect(page.getByLabel("Preferred name for unnamed transactions", { exact: true })).toHaveValue("");
  await expect(clearSetting(page, "name")).toBeChecked();
  await expect(clearSetting(page, "amount")).toBeChecked();
  await expect(clearSetting(page, "category")).not.toBeChecked();
  await expect(page.locator(`[data-account-name="F30 storage account"]`)).toBeVisible();
  expect(pageErrors).toEqual([]);
});
