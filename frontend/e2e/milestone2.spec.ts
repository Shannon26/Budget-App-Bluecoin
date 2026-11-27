import { readFile } from "node:fs/promises";
import { expect, test, type Page } from "@playwright/test";

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

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
  page: import("@playwright/test").Page,
  name: string,
  opening: string,
  accountType?: string,
) {
  await page.getByTestId("account-name").fill(name);
  if (accountType) await page.getByTestId("account-type").selectOption(accountType);
  await page.getByTestId("opening-balance").fill(opening);
  await page.getByTestId("create-account").click();
  await expect(page.locator(`[data-account-name="${name}"]`)).toBeVisible();
}

function balance(page: import("@playwright/test").Page, name: string) {
  return page.locator(`[data-account-name="${name}"]`).getByTestId("account-balance");
}

async function openEntry(page: import("@playwright/test").Page, kind: string, name: string) {
  const row = page.locator(`[data-testid^="ledger-row-${kind}-"]`).filter({ hasText: name }).first();
  await row.click();
  await expect(page.getByRole("dialog", { name: "Entry details" })).toBeVisible();
}

test("account type catalog is usable in the empty overview", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText("Start with an account", { exact: true })).toBeVisible();
  await expect(page.getByTestId("account-type")).toBeEnabled();
  await expect(page.getByTestId("account-type-group")).toHaveText("Unclassified");
  const optionLabels = await page.getByTestId("account-type").locator("option").allTextContents();
  expect(optionLabels).toContain("Cash");
  expect(optionLabels).toContain("Credit Card");
  expect(optionLabels).toContain("Loan");
});

test("category editor keeps new hierarchy changes at two levels", async ({ page }) => {
  await page.goto("/");
  await page.getByTestId("nav-categories").click();
  await expect(page.locator(".category-row").first()).toBeVisible();

  const categoryNames = await page.locator(".category-row-main strong").allTextContents();
  const entertainmentIndex = categoryNames.findIndex((name) => name === "Entertainment");
  const diningIndex = categoryNames.findIndex((name) => name === "Dining Out");
  expect(entertainmentIndex).toBeGreaterThanOrEqual(0);
  expect(diningIndex).toBe(entertainmentIndex + 1);

  const foodRow = page.locator(".category-row:not(.category-child)").filter({ hasText: "Food & Drink" });
  await foodRow.getByRole("button", { name: "Edit", exact: true }).click();
  await expect(page.getByTestId("category-parent")).toBeDisabled();
  await expect(page.getByTestId("category-hierarchy-message")).toHaveText(
    "Move this category’s subcategories first.",
  );
  await page.getByRole("button", { name: "Close category editor" }).click();

  await page.getByTestId("new-category").click();
  const parentOptions = await page.getByTestId("category-parent").locator("option").allTextContents();
  expect(parentOptions).toContain("Food & Drink");
  expect(parentOptions).toContain("Shopping");
  expect(parentOptions).not.toContain("Dining Out");
  await page.getByRole("button", { name: "Close category editor" }).click();

  const diningRow = page.locator(".category-row.category-child").filter({ hasText: "Dining Out" });
  await diningRow.getByRole("button", { name: "Edit", exact: true }).click();
  await expect(page.getByTestId("category-parent")).toBeEnabled();
  await page.getByTestId("category-parent").selectOption({ label: "Shopping" });
  await page.getByTestId("save-category").click();
  await expect(
    page.locator(".category-row.category-child").filter({ hasText: "Dining Out" }),
  ).toContainText("Shopping /");

  await page
    .locator(".category-row.category-child")
    .filter({ hasText: "Dining Out" })
    .getByRole("button", { name: "Edit", exact: true })
    .click();
  await page.getByTestId("category-parent").selectOption("");
  await page.getByTestId("save-category").click();
  await expect(
    page.locator(".category-row:not(.category-child)").filter({ hasText: "Dining Out" }),
  ).toHaveCount(1);

  await page
    .locator(".category-row:not(.category-child)")
    .filter({ hasText: "Dining Out" })
    .getByRole("button", { name: "Edit", exact: true })
    .click();
  await page.getByTestId("category-parent").selectOption({ label: "Food & Drink" });
  await page.getByTestId("save-category").click();
  await expect(
    page.locator(".category-row.category-child").filter({ hasText: "Dining Out" }),
  ).toContainText("Food & Drink /");
});

test("milestone 2 category and transaction management flow", async ({ page, request }) => {
  await page.goto("/");
  await createAccount(page, "E2E Checking", "5000");
  await createAccount(page, "E2E Savings", "1000");

  // Starter catalog: dining expense and salary income.
  await page.getByTestId("transaction-name").fill("Dinner");
  await page.getByTestId("transaction-account").selectOption({ label: "E2E Checking · INR" });
  await page.getByTestId("transaction-category").selectOption({ label: "Food & Drink / Dining Out" });
  await page.getByTestId("transaction-amount").fill("250");
  await page.getByRole("button", { name: "Record expense", exact: true }).click();
  await expect(balance(page, "E2E Checking")).toContainText("4,750.00");

  await page.getByRole("button", { name: "Income", exact: true }).click();
  await page.getByTestId("transaction-name").fill("Monthly salary");
  await page.getByTestId("transaction-category").selectOption({ label: "Employment / Salary" });
  await page.getByTestId("transaction-amount").fill("1000");
  await page.getByRole("button", { name: "Record income", exact: true }).click();
  await expect(balance(page, "E2E Checking")).toContainText("5,750.00");
  await expect(page.getByTestId("income-total")).toContainText("1,000.00");
  await expect(page.getByTestId("expense-total")).toContainText("250.00");
  await expect(
    page.locator(".category-total-grid article").filter({ hasText: "Dining Out" }).locator("strong"),
  ).toContainText("250.00");

  // Edit the expense to ₹300.
  await page.getByTestId("nav-ledger").click();
  await openEntry(page, "expense", "Dinner");
  await page.getByRole("button", { name: "Edit", exact: true }).click();
  await page.getByTestId("edit-amount").fill("300");
  await page.getByTestId("save-edit").click();
  await expect(page.getByRole("dialog", { name: "Entry details" })).toBeHidden();

  // Duplicate, then permanently delete the duplicate.
  await openEntry(page, "expense", "Dinner");
  await page.getByRole("button", { name: "Duplicate", exact: true }).click();
  await expect(page.locator('[data-testid^="ledger-row-expense-"]').filter({ hasText: "Dinner" })).toHaveCount(2);
  await openEntry(page, "expense", "Dinner");
  await page.getByRole("button", { name: "Delete", exact: true }).click();
  await page.getByTestId("confirm-delete").click();
  await expect(page.locator('[data-testid^="ledger-row-expense-"]').filter({ hasText: "Dinner" })).toHaveCount(1);

  await page.getByTestId("nav-overview").click();
  await expect(balance(page, "E2E Checking")).toContainText("5,700.00");
  await expect(page.getByTestId("expense-total")).toContainText("300.00");
  await expect(
    page.locator(".category-total-grid article").filter({ hasText: "Dining Out" }).locator("strong"),
  ).toContainText("300.00");

  // Reconciled toggle and named filter persistence.
  await page.getByTestId("nav-ledger").click();
  await openEntry(page, "expense", "Dinner");
  await page.getByRole("button", { name: "Mark reconciled", exact: true }).click();
  await page.getByTestId("filter-category").selectOption({ label: "Food & Drink / Dining Out" });
  await page.getByTestId("filter-status").selectOption("true");
  await page.getByTestId("apply-filters").click();
  await page.getByTestId("save-filter-name").fill("Reconciled dining");
  await page.getByTestId("save-filter").click();
  await page.reload();
  await page.getByTestId("nav-ledger").click();
  await page.getByTestId("saved-filter").selectOption({ label: "Reconciled dining" });
  await expect(page.locator('[data-testid^="ledger-row-expense-"]').filter({ hasText: "Dinner" })).toHaveCount(1);

  // Uncategorized fallback.
  await page.getByTestId("nav-overview").click();
  await page.getByTestId("transaction-name").fill("Uncategorized snack");
  await page.getByTestId("transaction-amount").fill("50");
  await page.getByRole("button", { name: "Record expense", exact: true }).click();
  await page.getByTestId("nav-ledger").click();
  await expect(
    page.locator('[data-testid^="ledger-row-expense-"]').filter({ hasText: "Uncategorized snack" }),
  ).toContainText("Uncategorized");

  // Archive referenced Dining Out; history remains, picker hides it.
  await page.getByTestId("nav-categories").click();
  const diningRow = page.locator(".category-row").filter({ hasText: "Dining Out" });
  await diningRow.getByRole("button", { name: "Archive", exact: true }).click();
  await expect(diningRow).toContainText("Archived");
  await page.getByTestId("nav-overview").click();
  await expect(page.getByTestId("transaction-category").locator("option").filter({ hasText: "Dining Out" })).toHaveCount(0);
  await page.getByTestId("nav-ledger").click();
  await expect(page.locator('[data-testid^="ledger-row-expense-"]').filter({ hasText: "Dinner" })).toContainText("Dining Out");

  // Transfer appears once, edits both accounts, and shows the relevant side per account.
  await page.getByTestId("nav-overview").click();
  await page.getByTestId("transfer-name").fill("E2E Transfer");
  await page.getByTestId("transfer-from").selectOption({ label: "E2E Checking" });
  await page.getByTestId("transfer-to").selectOption({ label: "E2E Savings" });
  await page.getByTestId("transfer-amount").fill("500");
  await page.getByTestId("submit-transfer").click();
  await expect(balance(page, "E2E Checking")).toContainText("5,150.00");
  await expect(balance(page, "E2E Savings")).toContainText("1,500.00");
  await expect(page.getByTestId("income-total")).toContainText("1,000.00");
  await expect(page.getByTestId("expense-total")).toContainText("350.00");

  await page.getByTestId("nav-ledger").click();
  await expect(page.locator('[data-testid^="ledger-row-transfer-"]').filter({ hasText: "E2E Transfer" })).toHaveCount(1);
  await openEntry(page, "transfer", "E2E Transfer");
  await page.getByRole("button", { name: "Edit", exact: true }).click();
  await page.getByTestId("edit-amount").fill("600");
  const transferFrom = await page.getByTestId("edit-account").inputValue();
  const transferTo = await page.getByTestId("edit-counterpart").inputValue();
  await page.getByTestId("edit-counterpart").selectOption(transferFrom);
  await expect(page.getByTestId("edit-account")).toHaveValue(transferFrom);
  await expect(page.getByTestId("edit-counterpart")).toHaveValue(transferFrom);
  await expect(page.getByRole("alert")).toContainText("Choose two different accounts");
  await expect(page.getByTestId("save-edit")).toBeDisabled();
  await page.getByTestId("edit-counterpart").selectOption(transferTo);
  await expect(page.getByTestId("save-edit")).toBeEnabled();
  await page.getByTestId("save-edit").click();

  await page.getByTestId("nav-overview").click();
  await expect(balance(page, "E2E Checking")).toContainText("5,050.00");
  await expect(balance(page, "E2E Savings")).toContainText("1,600.00");
  await expect(page.getByTestId("income-total")).toContainText("1,000.00");
  await expect(page.getByTestId("expense-total")).toContainText("350.00");

  await page.getByTestId("account-history-1").click();
  await expect(page.locator('[data-testid^="ledger-row-transfer_out-"]').filter({ hasText: "E2E Transfer" })).toHaveCount(1);
  await expect(page.locator('[data-testid^="ledger-row-transfer_in-"]')).toHaveCount(0);
  await page.getByTestId("nav-overview").click();
  await page.getByTestId("account-history-2").click();
  await expect(page.locator('[data-testid^="ledger-row-transfer_in-"]').filter({ hasText: "E2E Transfer" })).toHaveCount(1);
  await expect(page.locator('[data-testid^="ledger-row-transfer_out-"]')).toHaveCount(0);

  await page.getByTestId("nav-overview").click();
  await page.getByTestId("nav-ledger").click();
  await openEntry(page, "transfer", "E2E Transfer");
  await page.getByRole("button", { name: "Delete", exact: true }).click();
  await page.getByTestId("confirm-delete").click();
  await page.getByTestId("nav-overview").click();
  await expect(balance(page, "E2E Checking")).toContainText("5,650.00");
  await expect(balance(page, "E2E Savings")).toContainText("1,000.00");
  await expect(page.getByTestId("income-total")).toContainText("1,000.00");
  await expect(page.getByTestId("expense-total")).toContainText("350.00");

  const state = await (await request.get("/api/state")).json();
  expect(state.category_totals.find((total: { category_id: number }) => total.category_id !== null && total.name === "Dining Out").total_minor).toBe(30000);
  expect(state.category_totals.find((total: { category_id: null }) => total.category_id === null && total.name === "Uncategorized").total_minor).toBe(5000);
});

test("ledger and account history load older same-day entries with complete daily totals", async ({ page, request }) => {
  const accountResponse = await request.post("/api/accounts", {
    data: { name: "Pagination account", currency: "INR", opening_balance_minor: 100000 },
  });
  expect(accountResponse.ok()).toBeTruthy();
  const accountId = (await accountResponse.json()).id as number;

  for (let start = 1; start <= 201; start += 10) {
    const batch = [];
    for (let value = start; value < Math.min(start + 10, 202); value += 1) {
      batch.push(
        request.post("/api/transactions", {
          data: {
            kind: "expense",
            account_id: accountId,
            amount_minor: 100,
            name: `Pagination expense ${value}`,
            occurred_at: "2026-11-20T12:00:00",
          },
        }),
      );
    }
    const responses = await Promise.all(batch);
    for (const response of responses) expect(response.ok()).toBeTruthy();
  }

  await page.goto("/");
  await page.getByTestId("nav-ledger").click();
  await page.getByTestId("ledger-search").fill("Pagination expense");
  await page.getByTestId("filter-account").selectOption(String(accountId));
  await page.getByTestId("apply-filters").click();

  const rows = page.locator('[data-testid^="ledger-row-expense-"]');
  const day = page.locator(".ledger-day").filter({ hasText: "2026-11-20" });
  await expect(day.locator(".ledger-day-header")).toContainText("Expense ₹201.00");
  await expect(rows).toHaveCount(200);
  await expect(page.locator(".load-more")).toContainText("Showing 200 of 201");

  const exported = await downloadLedgerCsv(page);
  expect(exported[0]).toEqual(CSV_HEADERS);
  expect(exported.slice(1)).toHaveLength(201);
  const exportedNames = exported.slice(1).map((row) => row[2]);
  expect(new Set(exportedNames).size).toBe(201);
  for (let value = 1; value <= 201; value += 1) {
    expect(exportedNames).toContain(`Pagination expense ${value}`);
  }

  await page.getByTestId("load-more").click();
  await expect(rows).toHaveCount(201);
  await expect(page.getByTestId("load-more")).toHaveCount(0);

  const rowIds = await rows.evaluateAll((elements) =>
    elements.map((element) => element.getAttribute("data-testid")),
  );
  expect(new Set(rowIds).size).toBe(201);
  const rowText = await rows.allTextContents();
  for (let value = 1; value <= 201; value += 1) {
    expect(rowText.some((text) => text.includes(`Pagination expense ${value}`))).toBeTruthy();
  }

  await page.getByTestId("nav-overview").click();
  await page.getByTestId(`account-history-${accountId}`).click();
  const historyRows = page.locator('[data-testid^="ledger-row-expense-"]');
  await expect(page.locator(".ledger-day-header")).toContainText("Expense ₹201.00");
  await expect(historyRows).toHaveCount(200);
  await page.getByTestId("load-more").click();
  await expect(historyRows).toHaveCount(201);
  const historyIds = await historyRows.evaluateAll((elements) =>
    elements.map((element) => element.getAttribute("data-testid")),
  );
  expect(new Set(historyIds).size).toBe(201);
});

test("ledger ignores superseded filter responses and stale errors", async ({ page, request }) => {
  const accountIds: number[] = [];
  for (const name of ["Race slow account", "Race fast account"]) {
    const response = await request.post("/api/accounts", {
      data: { name, currency: "INR", opening_balance_minor: 0 },
    });
    expect(response.ok()).toBeTruthy();
    accountIds.push((await response.json()).id as number);
  }
  const [slowId, fastId] = accountIds;
  for (const [accountId, name, amount] of [
    [slowId, "Slow race one", 10000],
    [slowId, "Slow race two", 20000],
    [fastId, "Fast race item", 70000],
  ] as const) {
    const response = await request.post("/api/transactions", {
      data: {
        kind: "expense",
        account_id: accountId,
        amount_minor: amount,
        name,
        occurred_at: "2026-11-20T12:00:00",
      },
    });
    expect(response.ok()).toBeTruthy();
  }

  await page.goto("/");
  await page.getByTestId("nav-ledger").click();
  const releases = [deferred(), deferred()];
  const starts = [deferred(), deferred()];
  const finishes = [deferred(), deferred()];
  let call = 0;
  await page.route("**/api/ledger?**", async (route) => {
    const url = new URL(route.request().url());
    if (
      url.searchParams.get("account_id") !== String(slowId) ||
      url.searchParams.get("offset") !== "0"
    ) {
      await route.continue();
      return;
    }
    const index = call++;
    starts[index].resolve();
    const response = await route.fetch();
    await releases[index].promise;
    if (index === 1) {
      await route.fulfill({
        status: 500,
        contentType: "application/json",
        body: JSON.stringify({ error: "Stale ledger failure" }),
      });
    } else {
      await route.fulfill({ response });
    }
    finishes[index].resolve();
  });

  await page.getByTestId("filter-account").selectOption(String(slowId));
  await page.getByTestId("apply-filters").click();
  await starts[0].promise;
  await expect(page.getByText("Loading ledger…")).toBeVisible();
  await page.getByTestId("filter-account").selectOption(String(fastId));
  await page.getByTestId("apply-filters").click();
  await expect(page.locator('[data-testid^="ledger-row-expense-"]').filter({ hasText: "Fast race item" })).toHaveCount(1);
  await expect(page.locator(".ledger-day-header")).toContainText("Expense ₹700.00");
  releases[0].resolve();
  await finishes[0].promise;
  await expect(page.locator('[data-testid^="ledger-row-expense-"]').filter({ hasText: "Fast race item" })).toHaveCount(1);
  await expect(page.locator('[data-testid^="ledger-row-expense-"]').filter({ hasText: "Slow race" })).toHaveCount(0);
  await expect(page.locator(".ledger-day-header")).toContainText("Expense ₹700.00");

  await page.getByTestId("filter-account").selectOption(String(slowId));
  await page.getByTestId("apply-filters").click();
  await starts[1].promise;
  await page.getByTestId("filter-account").selectOption(String(fastId));
  await page.getByTestId("apply-filters").click();
  await expect(page.locator('[data-testid^="ledger-row-expense-"]').filter({ hasText: "Fast race item" })).toHaveCount(1);
  releases[1].resolve();
  await finishes[1].promise;
  await expect(page.getByText("Stale ledger failure")).toHaveCount(0);
  await expect(page.locator('[data-testid^="ledger-row-expense-"]').filter({ hasText: "Fast race item" })).toHaveCount(1);
  await expect(page.locator(".ledger-day-header")).toContainText("Expense ₹700.00");
});

test("ledger ignores a superseded account-history response", async ({ page, request }) => {
  const accountIds: number[] = [];
  for (const name of ["History slow account", "History fast account"]) {
    const response = await request.post("/api/accounts", {
      data: { name, currency: "INR", opening_balance_minor: 0 },
    });
    expect(response.ok()).toBeTruthy();
    accountIds.push((await response.json()).id as number);
  }
  const [slowId, fastId] = accountIds;
  for (const [accountId, name, amount] of [
    [slowId, "Slow history item", 40000],
    [fastId, "Fast history item", 80000],
  ] as const) {
    const response = await request.post("/api/transactions", {
      data: {
        kind: "expense",
        account_id: accountId,
        amount_minor: amount,
        name,
        occurred_at: "2026-11-21T12:00:00",
      },
    });
    expect(response.ok()).toBeTruthy();
  }

  await page.goto("/");
  const release = deferred();
  const started = deferred();
  const finished = deferred();
  await page.route(new RegExp(`/api/accounts/${slowId}/history\\?limit=200&offset=0$`), async (route) => {
    started.resolve();
    const response = await route.fetch();
    await release.promise;
    await route.fulfill({ response });
    finished.resolve();
  });
  await page.getByTestId(`account-history-${slowId}`).click();
  await started.promise;
  await expect(page.getByText("Loading ledger…")).toBeVisible();
  await page.getByTestId("nav-overview").click();
  await page.getByTestId(`account-history-${fastId}`).click();
  await expect(page.locator('[data-testid^="ledger-row-expense-"]').filter({ hasText: "Fast history item" })).toHaveCount(1);
  await expect(page.locator(".ledger-day-header")).toContainText("Expense ₹800.00");
  release.resolve();
  await finished.promise;
  await expect(page.locator('[data-testid^="ledger-row-expense-"]').filter({ hasText: "Fast history item" })).toHaveCount(1);
  await expect(page.locator('[data-testid^="ledger-row-expense-"]').filter({ hasText: "Slow history item" })).toHaveCount(0);
  await expect(page.locator(".ledger-day-header")).toContainText("Expense ₹800.00");
});

test("ledger ignores an obsolete load-more page after a filter change", async ({ page, request }) => {
  const accountIds: number[] = [];
  for (const name of ["Page race account", "Page active account"]) {
    const response = await request.post("/api/accounts", {
      data: { name, currency: "INR", opening_balance_minor: 0 },
    });
    expect(response.ok()).toBeTruthy();
    accountIds.push((await response.json()).id as number);
  }
  const [pageId, activeId] = accountIds;
  for (let start = 1; start <= 201; start += 10) {
    const batch = [];
    for (let value = start; value < Math.min(start + 10, 202); value += 1) {
      batch.push(
        request.post("/api/transactions", {
          data: {
            kind: "expense",
            account_id: pageId,
            amount_minor: 100,
            name: `Stale page item ${value}`,
            occurred_at: "2026-11-22T12:00:00",
          },
        }),
      );
    }
    const responses = await Promise.all(batch);
    for (const response of responses) expect(response.ok()).toBeTruthy();
  }
  const activeResponse = await request.post("/api/transactions", {
    data: {
      kind: "expense",
      account_id: activeId,
      amount_minor: 500,
      name: "Active page item",
      occurred_at: "2026-11-22T12:00:00",
    },
  });
  expect(activeResponse.ok()).toBeTruthy();

  await page.goto("/");
  await page.getByTestId("nav-ledger").click();
  await page.getByTestId("filter-account").selectOption(String(pageId));
  await page.getByTestId("apply-filters").click();
  await expect(page.locator(".load-more")).toContainText("Showing 200 of 201");
  const release = deferred();
  const started = deferred();
  const finished = deferred();
  await page.route("**/api/ledger?**", async (route) => {
    const url = new URL(route.request().url());
    if (
      url.searchParams.get("account_id") !== String(pageId) ||
      url.searchParams.get("offset") !== "200"
    ) {
      await route.continue();
      return;
    }
    started.resolve();
    const response = await route.fetch();
    await release.promise;
    await route.fulfill({ response });
    finished.resolve();
  });
  await page.getByTestId("load-more").click();
  await started.promise;
  await page.getByTestId("filter-account").selectOption(String(activeId));
  await page.getByTestId("apply-filters").click();
  await expect(page.locator('[data-testid^="ledger-row-expense-"]').filter({ hasText: "Active page item" })).toHaveCount(1);
  await expect(page.locator(".ledger-day-header")).toContainText("Expense ₹5.00");
  release.resolve();
  await finished.promise;
  await expect(page.locator('[data-testid^="ledger-row-expense-"]').filter({ hasText: "Active page item" })).toHaveCount(1);
  await expect(page.locator('[data-testid^="ledger-row-expense-"]').filter({ hasText: "Stale page item" })).toHaveCount(0);
  await expect(page.locator(".load-more")).toHaveCount(0);
});

test("CSV download honors reopened saved filters and preserves transfers and special text", async ({
  page,
  request,
}) => {
  const sourceResponse = await request.post("/api/accounts", {
    data: { name: '=CSV Source, "Main"', currency: "INR", opening_balance_minor: 100000 },
  });
  expect(sourceResponse.ok()).toBeTruthy();
  const sourceId = (await sourceResponse.json()).id as number;
  const destinationResponse = await request.post("/api/accounts", {
    data: { name: "CSV Destination", currency: "INR", opening_balance_minor: 0 },
  });
  expect(destinationResponse.ok()).toBeTruthy();
  const destinationId = (await destinationResponse.json()).id as number;
  const initialState = await (await request.get("/api/state")).json();
  const groceriesId = initialState.categories.find(
    (category: { name: string }) => category.name === "Groceries",
  ).id as number;

  const entries = [
    {
      kind: "expense",
      account_id: sourceId,
      amount_minor: 1234,
      name: '=SUM(1,2), "lunch"\nsecond line',
      category_id: groceriesId,
      reconciled: true,
      note: '@note\nwith, "quotes"',
      occurred_at: "2026-11-21T10:00:00",
    },
    {
      kind: "expense",
      account_id: sourceId,
      amount_minor: 999,
      name: "CSV excluded distractor",
      reconciled: false,
      occurred_at: "2026-11-21T09:00:00",
    },
    {
      kind: "income",
      account_id: sourceId,
      amount_minor: 500000,
      name: "CSV Salary",
      occurred_at: "2026-11-21T11:00:00",
    },
  ];
  for (const entry of entries) {
    const response = await request.post("/api/transactions", { data: entry });
    expect(response.ok()).toBeTruthy();
  }
  const transferResponse = await request.post("/api/transfers", {
    data: {
      from_account_id: sourceId,
      to_account_id: destinationId,
      amount_minor: 25000,
      name: '+CSV transfer, "move"',
      note: "-transfer note",
      occurred_at: "2026-11-21T12:00:00",
    },
  });
  expect(transferResponse.ok()).toBeTruthy();

  await page.goto("/");
  await page.getByTestId("nav-ledger").click();
  await page.getByTestId("ledger-search").fill("SUM(1,2)");
  await page.getByTestId("filter-status").selectOption("true");
  await page.getByTestId("apply-filters").click();
  const specialRow = page
    .locator('[data-testid^="ledger-row-expense-"]')
    .filter({ hasText: "SUM(1,2)" });
  await expect(specialRow).toHaveCount(1);
  await expect(
    page.locator('[data-testid^="ledger-row-expense-"]').filter({ hasText: "CSV excluded distractor" }),
  ).toHaveCount(0);

  await page.getByTestId("save-filter-name").fill("CSV special reconciled");
  await page.getByTestId("save-filter").click();
  await expect(page.getByText("Named filter saved.")).toBeVisible();
  await page.reload();
  await page.getByTestId("nav-ledger").click();
  await page.getByTestId("saved-filter").selectOption({ label: "CSV special reconciled" });
  await expect(specialRow).toHaveCount(1);

  const savedFilterExport = await downloadLedgerCsv(page);
  expect(savedFilterExport[0]).toEqual(CSV_HEADERS);
  expect(savedFilterExport.slice(1)).toEqual([
    [
      "2026-11-21",
      "Expense",
      '\'=SUM(1,2), "lunch"\nsecond line',
      '\'=CSV Source, "Main"',
      "",
      "",
      "Food & Drink",
      "Groceries",
      "12.34",
      "INR",
      "Reconciled",
      '\'@note\nwith, "quotes"',
    ],
  ]);

  await page.getByRole("button", { name: "Reset", exact: true }).click();
  await page.getByTestId("filter-account").selectOption(String(sourceId));
  await page.getByTestId("apply-filters").click();
  await expect(page.locator('[data-testid^="ledger-row-"]')).toHaveCount(4);

  const accountExport = await downloadLedgerCsv(page);
  expect(accountExport[0]).toEqual(CSV_HEADERS);
  const dataRows = accountExport.slice(1);
  expect(dataRows).toHaveLength(4);
  const transferRows = dataRows.filter((row) => row[1] === "Transfer");
  expect(transferRows).toEqual([
    [
      "2026-11-21",
      "Transfer",
      '\'+CSV transfer, "move"',
      "",
      '\'=CSV Source, "Main"',
      "CSV Destination",
      "",
      "",
      "250.00",
      "INR",
      "Unreconciled",
      "'-transfer note",
    ],
  ]);
  expect(dataRows.find((row) => row[2] === "CSV Salary")).toEqual([
    "2026-11-21",
    "Income",
    "CSV Salary",
    '\'=CSV Source, "Main"',
    "",
    "",
    "Uncategorized",
    "",
    "5000.00",
    "INR",
    "Unreconciled",
    "",
  ]);
});

test("monthly category budgets roll up subcategories and persist independent monthly limits", async ({
  page,
  request,
}) => {
  await page.goto("/");
  await page.getByTestId("nav-budgets").click();
  const defaultMonth = await page.getByTestId("budget-month").inputValue();
  expect(defaultMonth).toMatch(/^\d{4}-\d{2}$/);

  const month = "2027-01";
  const nextMonth = "2027-02";
  const initialState = await (await request.get("/api/state")).json();
  const foodId = initialState.categories.find(
    (category: { name: string; kind: string }) => category.name === "Food & Drink" && category.kind === "expense",
  ).id as number;
  const groceriesId = initialState.categories.find(
    (category: { name: string }) => category.name === "Groceries",
  ).id as number;
  const accountResponse = await request.post("/api/accounts", {
    data: { name: "Budget E2E Checking", currency: "INR", opening_balance_minor: 1000000 },
  });
  expect(accountResponse.ok()).toBeTruthy();
  const accountId = (await accountResponse.json()).id as number;
  const savingsResponse = await request.post("/api/accounts", {
    data: { name: "Budget E2E Savings", currency: "INR", opening_balance_minor: 0 },
  });
  expect(savingsResponse.ok()).toBeTruthy();
  const savingsId = (await savingsResponse.json()).id as number;
  const categoryResponse = await request.post("/api/categories", {
    data: {
      name: "Budget Dining",
      kind: "expense",
      parent_id: foodId,
      color: "#b65d35",
      icon: "🍴",
      sort_order: 999,
    },
  });
  expect(categoryResponse.ok()).toBeTruthy();
  const diningId = (await categoryResponse.json()).id as number;

  const expenses = [
    { name: "Budget E2E groceries", amount_minor: 100000, category_id: groceriesId },
    { name: "Budget E2E dining", amount_minor: 180000, category_id: diningId },
  ];
  for (const expense of expenses) {
    const response = await request.post("/api/transactions", {
      data: {
        kind: "expense",
        account_id: accountId,
        occurred_at: `${month}-10T12:00:00`,
        ...expense,
      },
    });
    expect(response.ok()).toBeTruthy();
  }
  const incomeResponse = await request.post("/api/transactions", {
    data: {
      kind: "income",
      account_id: accountId,
      amount_minor: 500000,
      name: "Budget E2E income",
      occurred_at: `${month}-11T12:00:00`,
    },
  });
  expect(incomeResponse.ok()).toBeTruthy();
  const transferResponse = await request.post("/api/transfers", {
    data: {
      from_account_id: accountId,
      to_account_id: savingsId,
      amount_minor: 100000,
      name: "Budget E2E transfer",
      occurred_at: `${month}-12T12:00:00`,
    },
  });
  expect(transferResponse.ok()).toBeTruthy();

  await page.reload();
  await page.getByTestId("nav-budgets").click();
  await page.getByTestId("budget-month").fill(month);
  const foodRow = page.getByTestId(`budget-row-${foodId}`);
  const diningRow = page.getByTestId(`budget-row-${diningId}`);
  await expect(foodRow).toBeVisible();
  await expect(diningRow).toBeVisible();

  await foodRow.getByRole("button", { name: "Set limit", exact: true }).click();
  await page.getByTestId("budget-limit-amount").fill("5000");
  await page.getByTestId("save-budget-limit").click();
  await expect(foodRow.getByTestId(`budget-limit-${foodId}`)).toContainText("5,000.00");

  await diningRow.getByRole("button", { name: "Set limit", exact: true }).click();
  await page.getByTestId("budget-limit-amount").fill("1500");
  await page.getByTestId("save-budget-limit").click();
  await expect(page.getByTestId("budget-total-spent")).toContainText("2,800.00");
  await expect(foodRow.getByTestId(`budget-spent-${foodId}`)).toContainText("2,800.00");
  await expect(foodRow.getByTestId(`budget-remaining-${foodId}`)).toContainText("2,200.00 left");
  await expect(diningRow.getByTestId(`budget-spent-${diningId}`)).toContainText("1,800.00");
  await expect(diningRow.getByTestId(`budget-remaining-${diningId}`)).toContainText("300.00 over");
  await expect(diningRow.getByRole("progressbar")).toHaveAttribute(
    "aria-valuetext",
    "120% of limit used",
  );

  await foodRow.getByRole("button", { name: "Edit limit", exact: true }).click();
  await page.getByTestId("budget-limit-amount").fill("4000");
  await page.getByTestId("save-budget-limit").click();
  await expect(foodRow.getByTestId(`budget-remaining-${foodId}`)).toContainText("1,200.00 left");

  await page.reload();
  await page.getByTestId("nav-budgets").click();
  await expect(page.getByTestId("budget-month")).toHaveValue(defaultMonth);
  await page.getByTestId("budget-month").fill(month);
  await expect(foodRow.getByTestId(`budget-limit-${foodId}`)).toContainText("4,000.00");
  await expect(diningRow.getByTestId(`budget-limit-${diningId}`)).toContainText("1,500.00");

  await page.getByTestId("budget-month").fill(nextMonth);
  await expect(page.getByTestId("budget-total-spent")).toContainText("0.00");
  await expect(foodRow.getByTestId(`budget-limit-${foodId}`)).toHaveText("No limit");
  await expect(diningRow.getByTestId(`budget-limit-${diningId}`)).toHaveText("No limit");
  await page.getByTestId("budget-month").fill(month);

  const archiveResponse = await request.delete(`/api/categories/${diningId}`);
  expect(archiveResponse.ok()).toBeTruthy();
  expect((await archiveResponse.json()).archived).toBe(true);
  await page.reload();
  await page.getByTestId("nav-budgets").click();
  await page.getByTestId("budget-month").fill(month);
  await expect(diningRow).toContainText("Archived");
  await expect(diningRow.getByTestId(`budget-spent-${diningId}`)).toContainText("1,800.00");
  await expect(diningRow.getByTestId(`budget-limit-${diningId}`)).toContainText("1,500.00");
  await expect(diningRow.getByTestId(`budget-remaining-${diningId}`)).toContainText("300.00 over");

  await diningRow.getByRole("button", { name: "Remove", exact: true }).click();
  await expect(diningRow.getByTestId(`budget-limit-${diningId}`)).toHaveText("No limit");
  await expect(diningRow.getByTestId(`budget-spent-${diningId}`)).toContainText("1,800.00");
  await expect(diningRow.getByRole("button", { name: "Set limit", exact: true })).toBeDisabled();
});

test("budget responses stay tied to their selected month and invalid dates are flagged", async ({
  page,
  request,
}) => {
  const monthA = "2028-01";
  const monthB = "2028-02";
  const initialState = await (await request.get("/api/state")).json();
  const foodId = initialState.categories.find(
    (category: { name: string }) => category.name === "Food & Drink",
  ).id as number;
  const groceriesId = initialState.categories.find(
    (category: { name: string }) => category.name === "Groceries",
  ).id as number;
  const accountResponse = await request.post("/api/accounts", {
    data: { name: "Budget race checking", currency: "INR", opening_balance_minor: 100000 },
  });
  expect(accountResponse.ok()).toBeTruthy();
  const accountId = (await accountResponse.json()).id as number;
  for (const [month, amount] of [[monthA, 12345], [monthB, 67890]] as const) {
    const response = await request.post("/api/transactions", {
      data: {
        kind: "expense",
        account_id: accountId,
        amount_minor: amount,
        name: `Budget race ${month}`,
        category_id: groceriesId,
        occurred_at: `${month}-10T12:00:00`,
      },
    });
    expect(response.ok()).toBeTruthy();
  }
  const budgetResponse = await request.put(`/api/category-budgets/${foodId}?month=${monthB}`, {
    data: { amount_minor: 100000 },
  });
  expect(budgetResponse.ok()).toBeTruthy();

  await page.goto("/");
  await page.getByTestId("nav-budgets").click();
  const oldLoad = deferred();
  const oldLoadStarted = deferred();
  const oldLoadFinished = deferred();
  await page.route(/\/api\/category-budgets\?month=2028-01$/, async (route) => {
    oldLoadStarted.resolve();
    const response = await route.fetch();
    await oldLoad.promise;
    await route.fulfill({ response });
    oldLoadFinished.resolve();
  });
  await page.route(/\/api\/category-budgets\?month=2028-02$/, async (route) => {
    const response = await route.fetch();
    const body = await response.json();
    body.invalid_date_expense_count = 2;
    body.invalid_date_spent_minor = 1000;
    await route.fulfill({
      status: response.status(),
      headers: response.headers(),
      json: body,
    });
  });
  await page.route(/\/api\/category-budgets\?month=2028-03$/, async (route) => {
    await route.fulfill({
      status: 500,
      contentType: "application/json",
      body: JSON.stringify({ error: "Could not load budgets" }),
    });
  });

  await page.getByTestId("budget-month").fill(monthA);
  await oldLoadStarted.promise;
  await expect(page.getByText("Loading budgets…")).toBeVisible();
  await expect(page.getByTestId(`edit-budget-${foodId}`)).toHaveCount(0);
  await expect(page.getByTestId("budget-total-spent")).toHaveText("Pending…");
  await expect(page.getByTestId("budget-uncategorized-spent")).toHaveText("Pending…");
  await expect(page.getByTestId("budget-expense-count")).toHaveText("Expense count pending");
  await page.getByTestId("budget-month").fill("2028-03");
  await expect(page.getByText("Could not load budgets.")).toBeVisible();
  await expect(page.getByTestId("budget-total-spent")).toHaveText("Unavailable");
  await expect(page.getByTestId("budget-uncategorized-spent")).toHaveText("Unavailable");
  await expect(page.getByTestId("budget-expense-count")).toHaveText("Expense count unavailable");
  await page.getByTestId("budget-month").fill(monthB);
  await expect(page.getByTestId("budget-total-spent")).toContainText("678.90");
  await expect(page.getByTestId("budget-invalid-date-warning")).toContainText(
    "2 expenses with invalid dates",
  );
  await expect(page.getByTestId("budget-invalid-date-warning")).toContainText("₹10.00");
  oldLoad.resolve();
  await oldLoadFinished.promise;
  await expect(page.getByTestId("budget-month")).toHaveValue(monthB);
  await expect(page.getByTestId("budget-total-spent")).toContainText("678.90");

  await page.getByTestId("budget-month").fill("2030-01");
  await expect(page.getByTestId("budget-total-spent")).toHaveText("₹0.00");
  await expect(page.getByTestId("budget-uncategorized-spent")).toHaveText("₹0.00");
  await expect(page.getByTestId("budget-expense-count")).toHaveText(
    "0 expenses · income and transfers excluded",
  );

  await page.getByTestId("budget-month").fill(monthA);
  await expect(page.getByTestId("budget-total-spent")).toContainText("123.45");
  const saveResponse = deferred();
  const saveStarted = deferred();
  const saveFinished = deferred();
  await page.route(/\/api\/category-budgets\/\d+\?month=2028-01$/, async (route) => {
    if (route.request().method() !== "PUT") {
      await route.continue();
      return;
    }
    saveStarted.resolve();
    const response = await route.fetch();
    await saveResponse.promise;
    await route.fulfill({ response });
    saveFinished.resolve();
  });
  const foodRow = page.getByTestId(`budget-row-${foodId}`);
  await foodRow.getByRole("button", { name: "Set limit", exact: true }).click();
  await page.getByTestId("budget-limit-amount").fill("500");
  await page.getByTestId("save-budget-limit").click();
  await saveStarted.promise;
  await page.getByTestId("budget-month").fill(monthB);
  await expect(page.getByTestId("budget-total-spent")).toContainText("678.90");
  saveResponse.resolve();
  await saveFinished.promise;
  await expect(page.getByTestId("budget-month")).toHaveValue(monthB);
  await expect(page.getByTestId("budget-total-spent")).toContainText("678.90");
  await expect(foodRow.getByTestId(`budget-limit-${foodId}`)).toContainText("1,000.00");

  const removeResponse = deferred();
  const removeStarted = deferred();
  const removeFinished = deferred();
  await page.route(/\/api\/category-budgets\/\d+\?month=2028-02$/, async (route) => {
    if (route.request().method() !== "DELETE") {
      await route.continue();
      return;
    }
    removeStarted.resolve();
    const response = await route.fetch();
    await removeResponse.promise;
    await route.fulfill({ response });
    removeFinished.resolve();
  });
  await foodRow.getByRole("button", { name: "Remove", exact: true }).click();
  await removeStarted.promise;
  await page.getByTestId("budget-month").fill(monthA);
  await expect(page.getByTestId("budget-total-spent")).toContainText("123.45");
  removeResponse.resolve();
  await removeFinished.promise;
  await expect(page.getByTestId("budget-month")).toHaveValue(monthA);
  await expect(page.getByTestId("budget-total-spent")).toContainText("123.45");
});

test("Net Earnings compares months and keeps report data tied to the active month", async ({
  page,
  request,
}) => {
  const currentMonth = "2029-11";
  const priorMonth = "2029-10";
  const accountIds: number[] = [];
  for (const name of ["F05 visible", "F05 picker hidden", "F05 report hidden"]) {
    const response = await request.post("/api/accounts", {
      data: { name, currency: "INR", opening_balance_minor: 1000000 },
    });
    expect(response.ok()).toBeTruthy();
    accountIds.push((await response.json()).id as number);
  }
  const [visibleId, pickerId, reportHiddenId] = accountIds;
  expect(
    (
      await request.put(`/api/accounts/${pickerId}`, {
        data: { hide_from_selection: true },
      })
    ).ok(),
  ).toBeTruthy();
  expect(
    (
      await request.put(`/api/accounts/${reportHiddenId}`, {
        data: { hide_from_reports: true },
      })
    ).ok(),
  ).toBeTruthy();

  const categoryIds: number[] = [];
  for (const [name, kind, parentId] of [
    ["F05 Expense Parent", "expense", null],
    ["F05 Expense Child", "expense", null],
    ["F05 Expense Archived", "expense", null],
    ["F05 Income Parent", "income", null],
    ["F05 Income Child", "income", null],
  ] as const) {
    const response = await request.post("/api/categories", {
      data: {
        name,
        kind,
        parent_id: parentId,
        color: "#426b8a",
        icon: "•",
        sort_order: 100,
      },
    });
    expect(response.ok()).toBeTruthy();
    categoryIds.push((await response.json()).id as number);
  }
  const [expenseParent, expenseChild, archivedId, incomeParent, incomeChild] = categoryIds;
  expect(
    (
      await request.put(`/api/categories/${expenseChild}`, {
        data: {
          name: "F05 Expense Child",
          kind: "expense",
          parent_id: expenseParent,
          color: "#426b8a",
          icon: "•",
          sort_order: 100,
        },
      })
    ).ok(),
  ).toBeTruthy();
  expect(
    (
      await request.put(`/api/categories/${incomeChild}`, {
        data: {
          name: "F05 Income Child",
          kind: "income",
          parent_id: incomeParent,
          color: "#426b8a",
          icon: "•",
          sort_order: 100,
        },
      })
    ).ok(),
  ).toBeTruthy();

  const entries = [
    [visibleId, "income", 100000, "F05 current income parent", incomeParent, `${currentMonth}-05T12:00:00`],
    [visibleId, "income", 200000, "F05 current income child", incomeChild, `${currentMonth}-06T12:00:00`],
    [visibleId, "expense", 50000, "F05 current expense parent", expenseParent, `${currentMonth}-07T12:00:00`],
    [visibleId, "expense", 150000, "F05 current expense child", expenseChild, `${currentMonth}-08T12:00:00`],
    [visibleId, "expense", 25000, "F05 current archived expense", archivedId, `${currentMonth}-09T12:00:00`],
    [visibleId, "expense", 30000, "F05 current uncategorized expense", null, `${currentMonth}-10T12:00:00`],
    [pickerId, "expense", 40000, "F05 picker-only expense", null, `${currentMonth}-11T12:00:00`],
    [reportHiddenId, "income", 999000, "F05 hidden report income", null, `${currentMonth}-12T12:00:00`],
    [visibleId, "income", 100000, "F05 prior income", incomeChild, `${priorMonth}-15T12:00:00`],
    [visibleId, "expense", 40000, "F05 prior expense", expenseChild, `${priorMonth}-16T12:00:00`],
    [visibleId, "expense", 10000, "F05 prior uncategorized", null, `${priorMonth}-17T12:00:00`],
  ] as const;
  for (const [accountId, kind, amount, name, categoryId, occurredAt] of entries) {
    const response = await request.post("/api/transactions", {
      data: {
        kind,
        account_id: accountId,
        amount_minor: amount,
        name,
        category_id: categoryId,
        occurred_at: occurredAt,
      },
    });
    expect(response.ok()).toBeTruthy();
  }
  expect(
    (
      await request.post("/api/transfers", {
        data: {
          from_account_id: visibleId,
          to_account_id: pickerId,
          amount_minor: 50000,
          name: "F05 excluded transfer",
        },
      })
    ).ok(),
  ).toBeTruthy();
  expect((await request.delete(`/api/categories/${archivedId}`)).ok()).toBeTruthy();

  const releasePrior = deferred();
  const priorStarted = deferred();
  const priorFinished = deferred();
  await page.route("**/api/reports/net-earnings?**", async (route) => {
    const month = new URL(route.request().url()).searchParams.get("month");
    if (month === currentMonth) {
      const response = await route.fetch();
      const body = await response.json();
      body.invalid_date_entry_count = 2;
      await route.fulfill({
        status: response.status(),
        headers: response.headers(),
        json: body,
      });
      return;
    }
    if (month === priorMonth) {
      priorStarted.resolve();
      const response = await route.fetch();
      await releasePrior.promise;
      await route.fulfill({ response });
      priorFinished.resolve();
      return;
    }
    if (month === "2030-02") {
      await route.fulfill({
        status: 500,
        contentType: "application/json",
        body: JSON.stringify({ error: "Selected month is unavailable" }),
      });
      return;
    }
    await route.continue();
  });

  await page.goto("/");
  await page.getByTestId("nav-net-earnings").click();
  const localMonth = await page.evaluate(() => {
    const now = new Date();
    return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}`;
  });
  await expect(page.getByTestId("report-month")).toHaveValue(localMonth);
  await page.getByTestId("report-month").fill(currentMonth);
  await expect(page.getByTestId("report-income-prior")).toHaveText("₹1,000.00");
  await expect(page.getByTestId("report-income-current")).toHaveText("₹3,000.00");
  await expect(page.getByTestId("report-expense-prior")).toHaveText("₹500.00");
  await expect(page.getByTestId("report-expense-current")).toHaveText("₹2,950.00");
  await expect(page.getByTestId("report-net-prior")).toHaveText("₹500.00");
  await expect(page.getByTestId("report-net-current")).toHaveText("₹50.00");
  await expect(page.getByTestId("report-invalid-dates")).toContainText(
    "2 posted entries with invalid legacy dates",
  );
  await expect(page.getByTestId(`report-income-current-category-${incomeParent}-amount`)).toHaveText(
    "₹3,000.00",
  );
  const expenseParentRow = page.getByTestId(
    `report-expense-current-category-${expenseParent}`,
  );
  await expect(expenseParentRow).toContainText("Direct ₹500.00");
  await expect(expenseParentRow).toContainText("68%");
  await expect(
    page.getByTestId(`report-expense-current-category-${expenseChild}-amount`),
  ).toHaveText("₹1,500.00");
  await expect(
    page.getByTestId(`report-expense-current-category-${archivedId}`),
  ).toContainText("Archived");
  await expect(
    page.getByTestId("report-expense-current-category-uncategorized-amount"),
  ).toHaveText("₹700.00");

  await page.getByTestId("report-month").fill(priorMonth);
  await priorStarted.promise;
  await expect(page.getByTestId("report-loading")).toBeVisible();
  await expect(page.getByTestId("report-income-current")).toHaveText("Pending…");
  await page.getByTestId("report-month").fill(currentMonth);
  await expect(page.getByTestId("report-income-current")).toHaveText("₹3,000.00");
  releasePrior.resolve();
  await priorFinished.promise;
  await expect(page.getByTestId("report-income-current")).toHaveText("₹3,000.00");
  await expect(page.getByTestId("report-expense-current")).toHaveText("₹2,950.00");

  await page.getByTestId("report-month").fill("2030-02");
  await expect(page.getByTestId("report-error")).toContainText("Selected month is unavailable");
  await expect(page.getByTestId("report-income-current")).toHaveText("Unavailable");
  await page.getByTestId("report-month").fill("9999-12");
  await expect(page.getByTestId("report-error")).toContainText(
    "representable calendar boundaries",
  );

  await page.getByTestId("report-month").fill("2030-01");
  await expect(page.getByTestId("report-income-current")).toHaveText("₹0.00");
  await expect(page.getByTestId("report-expense-current")).toHaveText("₹0.00");
  await expect(page.getByTestId("report-net-current")).toHaveText("₹0.00");
  await expect(page.getByTestId("report-loading")).toHaveCount(0);
  await expect(page.getByTestId("report-error")).toHaveCount(0);
});

test("account types round-trip assets and liabilities without changing balances or history", async ({
  page,
  request,
}) => {
  const legacyResponse = await request.post("/api/accounts", {
    data: { name: "F01 Legacy", currency: "INR", opening_balance_minor: 0 },
  });
  expect(legacyResponse.ok()).toBeTruthy();
  const legacyId = (await legacyResponse.json()).id as number;

  await page.goto("/");
  const legacyCard = page.locator(`[data-account-name="F01 Legacy"]`);
  await expect(
    legacyCard.getByTestId(`account-classification-${legacyId}`).locator("span"),
  ).toHaveText(["Unclassified", "Unclassified"]);

  await createAccount(page, "F01 Checking", "100000", "checking");
  const checkingCard = page.locator('[data-account-name="F01 Checking"]');
  const checkingClassification = checkingCard.getByTestId(/account-classification-\d+/);
  await expect(checkingClassification).toContainText("Checking");
  await expect(checkingClassification).toContainText("Asset");

  await createAccount(page, "F01 Card", "-5000", "credit_card");
  const card = page.locator('[data-account-name="F01 Card"]');
  await expect(card.getByTestId(/account-classification-\d+/)).toContainText("Credit Card");
  await expect(card.getByTestId(/account-classification-\d+/)).toContainText("Liability");

  const state = await (await request.get("/api/state")).json();
  const checkingId = state.accounts.find(
    (account: { name: string }) => account.name === "F01 Checking",
  ).id as number;
  const expenseResponse = await request.post("/api/transactions", {
    data: {
      kind: "expense",
      account_id: checkingId,
      amount_minor: 2500,
      name: "F01 history expense",
      occurred_at: "2026-11-20T12:00:00",
    },
  });
  expect(expenseResponse.ok()).toBeTruthy();
  await page.reload();
  await expect(balance(page, "F01 Checking")).toContainText("99,975.00");

  await checkingCard.getByTestId(`account-type-edit-${checkingId}`).click();
  await checkingCard.getByTestId(`edit-account-type-${checkingId}`).selectOption("credit_card");
  await expect(checkingCard.getByTestId(`edit-account-type-${checkingId}-group`)).toHaveText(
    "Liability",
  );
  await checkingCard.getByTestId(`save-account-type-${checkingId}`).click();
  await expect(checkingCard.getByTestId(`account-classification-${checkingId}`)).toContainText(
    "Credit Card",
  );
  await expect(checkingCard.getByTestId(`account-classification-${checkingId}`)).toContainText(
    "Liability",
  );
  await expect(balance(page, "F01 Checking")).toContainText("99,975.00");

  await checkingCard.getByTestId(`account-type-edit-${checkingId}`).click();
  await checkingCard.getByTestId(`edit-account-type-${checkingId}`).selectOption("savings");
  await expect(checkingCard.getByTestId(`edit-account-type-${checkingId}-group`)).toHaveText(
    "Asset",
  );
  await checkingCard.getByTestId(`save-account-type-${checkingId}`).click();
  await expect(checkingCard.getByTestId(`account-classification-${checkingId}`)).toContainText(
    "Savings",
  );
  await expect(balance(page, "F01 Checking")).toContainText("99,975.00");

  await page.reload();
  await expect(checkingCard.getByTestId(`account-classification-${checkingId}`)).toContainText(
    "Savings",
  );
  await expect(checkingCard.getByTestId(`account-classification-${checkingId}`)).toContainText(
    "Asset",
  );
  await checkingCard.getByTestId(`account-history-${checkingId}`).click();
  await expect(page.getByRole("heading", { name: "F01 Checking" })).toBeVisible();
  await expect(
    page.locator('[data-testid^="ledger-row-expense-"]').filter({ hasText: "F01 history expense" }),
  ).toHaveCount(1);
  await expect(page.locator(".ledger-day-header")).toContainText("Expense ₹25.00");

  await page.getByTestId("nav-overview").click();
  await page.route(`**/api/accounts/${legacyId}`, async (route) => {
    if (route.request().method() === "PUT") {
      await route.fulfill({
        status: 400,
        contentType: "application/json",
        body: JSON.stringify({ error: "account type is not supported" }),
      });
      return;
    }
    await route.continue();
  });
  await legacyCard.getByTestId(`account-type-edit-${legacyId}`).click();
  await legacyCard.getByTestId(`edit-account-type-${legacyId}`).selectOption("loan");
  await legacyCard.getByTestId(`save-account-type-${legacyId}`).click();
  await expect(legacyCard.getByRole("alert")).toHaveText("account type is not supported");
  await expect(legacyCard.getByTestId(`account-classification-${legacyId}`)).toContainText(
    "Unclassified",
  );
  await page.unroute(`**/api/accounts/${legacyId}`);

  await legacyCard.getByTestId(`save-account-type-${legacyId}`).click();
  await expect(legacyCard.getByTestId(`account-classification-${legacyId}`)).toContainText("Loan");
  await expect(legacyCard.getByTestId(`account-classification-${legacyId}`)).toContainText(
    "Liability",
  );
  await page.reload();
  await expect(legacyCard.getByTestId(`account-classification-${legacyId}`)).toContainText("Loan");
});

test("account metadata persists and hide-selection preserves historical editing", async ({
  page,
  request,
}) => {
  await page.goto("/");
  await page.getByTestId("account-name").fill("F02 Main");
  await page.getByTestId("account-type").selectOption("checking");
  await page.getByTestId("account-note").fill("Main account note");
  await page.getByTestId("account-opening-date").fill("2024-02-29");
  await page.getByTestId("opening-balance").fill("100000");
  await page.getByTestId("create-account").click();

  const initialState = await (await request.get("/api/state")).json();
  const mainId = initialState.accounts.find(
    (account: { name: string }) => account.name === "F02 Main",
  ).id as number;
  let mainCard = page.locator('[data-account-name="F02 Main"]');
  await expect(mainCard.getByTestId(`account-note-${mainId}`)).toHaveText("Main account note");
  await expect(mainCard.getByTestId(`account-opening-date-${mainId}`)).toContainText("2024-02-29");
  await page.reload();
  mainCard = page.locator('[data-account-name="F02 Main"]');
  await expect(mainCard.getByTestId(`account-note-${mainId}`)).toHaveText("Main account note");
  await expect(mainCard.getByTestId(`account-opening-date-${mainId}`)).toContainText("2024-02-29");

  await page.getByTestId("account-name").fill("F02 Spare");
  await page.getByTestId("account-type").selectOption("savings");
  await page.getByTestId("account-hide-from-reports").check();
  await page.getByTestId("opening-balance").fill("50000");
  await page.getByTestId("create-account").click();
  const spareCard = page.locator('[data-account-name="F02 Spare"]');
  await expect(spareCard).toBeVisible();
  const spareState = await (await request.get("/api/state")).json();
  const spareId = spareState.accounts.find(
    (account: { name: string }) => account.name === "F02 Spare",
  ).id as number;
  await expect(spareCard.getByTestId(`account-visibility-${spareId}`)).toContainText(
    "Hidden from new pickers",
  );
  await expect(spareCard.getByTestId(`account-visibility-${spareId}`)).toContainText(
    "Hidden from future reports",
  );
  await expect(
    page.getByTestId("transaction-account").locator("option").filter({ hasText: "F02 Spare" }),
  ).toHaveCount(0);
  await page.getByTestId("transaction-name").fill("F02 historical expense");
  await page.getByTestId("transaction-account").selectOption({ label: "F02 Main · INR" });
  await page.getByTestId("transaction-amount").fill("25");
  await page.getByRole("button", { name: "Record expense", exact: true }).click();
  await expect(balance(page, "F02 Main")).toContainText("99,975.00");

  // An active create draft must be invalidated when its selected account is hidden.
  await page.getByTestId("transaction-account").selectOption({ label: "F02 Main · INR" });
  await mainCard.getByTestId(`account-type-edit-${mainId}`).click();
  await mainCard.getByTestId(`edit-account-name-${mainId}`).fill("F02 Renamed");
  await mainCard.getByTestId(`edit-account-note-${mainId}`).fill("");
  await mainCard.getByTestId(`edit-account-opening-date-${mainId}`).fill("");
  await mainCard.getByTestId(`edit-account-hide-selection-${mainId}`).check();
  await mainCard.getByTestId(`edit-account-hide-reports-${mainId}`).check();
  await mainCard.getByTestId(`save-account-type-${mainId}`).click();

  const renamedCard = page.locator('[data-account-name="F02 Renamed"]');
  await expect(renamedCard.getByTestId(`account-opening-date-${mainId}`)).toContainText("Not set");
  await expect(renamedCard.getByTestId(`account-note-${mainId}`)).toHaveCount(0);
  await expect(renamedCard.getByTestId(`account-visibility-${mainId}`)).toContainText(
    "Hidden from new pickers",
  );
  await expect(renamedCard.getByTestId(`account-visibility-${mainId}`)).toContainText(
    "Hidden from future reports",
  );
  await expect(balance(page, "F02 Renamed")).toContainText("99,975.00");
  await expect(page.getByTestId("transaction-account")).toHaveValue("");
  await expect(
    page.getByTestId("transaction-account").locator("option").filter({ hasText: "F02 Renamed" }),
  ).toHaveCount(0);
  await expect(
    page.getByTestId("transfer-from").locator("option").filter({ hasText: "F02 Renamed" }),
  ).toHaveCount(0);

  await page.reload();
  await expect(renamedCard.getByTestId(`account-visibility-${mainId}`)).toContainText(
    "Hidden from new pickers",
  );

  // Existing-entry editors, histories, and ledger filters still include hidden accounts.
  await page.getByTestId("nav-ledger").click();
  await openEntry(page, "expense", "F02 historical expense");
  await page.getByRole("button", { name: "Edit", exact: true }).click();
  const historicalAccount = page.getByTestId("edit-account");
  await expect(historicalAccount.locator(`option[value="${mainId}"]`)).toContainText(
    "hidden from new entries",
  );
  await historicalAccount.selectOption(String(mainId));
  await page.getByRole("textbox", { name: "Note", exact: true }).fill("Edited historical note");
  await page.getByTestId("save-edit").click();
  await expect(page.getByRole("dialog", { name: "Entry details" })).toBeHidden();
  await expect(page.getByTestId("filter-account").locator(`option[value="${mainId}"]`)).toHaveCount(1);
  await page.getByTestId("filter-account").selectOption(String(mainId));
  await page.getByTestId("apply-filters").click();
  await expect(
    page.locator('[data-testid^="ledger-row-expense-"]').filter({ hasText: "F02 historical expense" }),
  ).toHaveCount(1);

  await page.getByTestId("nav-overview").click();
  await expect(balance(page, "F02 Renamed")).toContainText("99,975.00");
  await renamedCard.getByTestId(`account-history-${mainId}`).click();
  await expect(page.getByRole("heading", { name: "F02 Renamed" })).toBeVisible();
  await expect(
    page.locator('[data-testid^="ledger-row-expense-"]').filter({ hasText: "F02 historical expense" }),
  ).toHaveCount(1);

  // Hiding every remaining selectable account produces an explicit empty state.
  await page.getByTestId("nav-overview").click();
  const state = await (await request.get("/api/state")).json();
  for (
    const account of state.accounts as Array<{
      id: number;
      hide_from_selection: boolean;
      hide_from_reports: boolean;
    }>
  ) {
    if (!account.hide_from_selection && !account.hide_from_reports) {
      const response = await request.put(`/api/accounts/${account.id}`, {
        data: { hide_from_selection: true },
      });
      expect(response.ok()).toBeTruthy();
    }
  }
  await page.reload();
  await expect(page.getByText(/All accounts are hidden from new entries/)).toBeVisible();
  await expect(page.getByTestId("transaction-account")).toBeDisabled();
  await expect(page.getByTestId("transaction-account")).toContainText("No selectable accounts");
  await expect(page.getByTestId("submit-transfer")).toBeDisabled();

  // Metadata and hide settings can be explicitly cleared and restored.
  await renamedCard.getByTestId(`account-type-edit-${mainId}`).click();
  await renamedCard.getByTestId(`edit-account-note-${mainId}`).fill("Restored note");
  await renamedCard.getByTestId(`edit-account-opening-date-${mainId}`).fill("2026-11-27");
  await renamedCard.getByTestId(`edit-account-hide-reports-${mainId}`).uncheck();
  await renamedCard.getByTestId(`edit-account-hide-selection-${mainId}`).uncheck();
  await renamedCard.getByTestId(`save-account-type-${mainId}`).click();
  await expect(renamedCard.getByTestId(`account-note-${mainId}`)).toHaveText("Restored note");
  await expect(renamedCard.getByTestId(`account-opening-date-${mainId}`)).toContainText("2026-11-27");
  await expect(renamedCard.getByTestId(`account-visibility-${mainId}`)).toHaveCount(0);
  await expect(
    page.getByTestId("transaction-account").locator("option").filter({ hasText: "F02 Renamed" }),
  ).toHaveCount(1);
  await page.reload();
  await expect(renamedCard.getByTestId(`account-note-${mainId}`)).toHaveText("Restored note");
});

test("hidden account selections stay cleared across refreshes until explicitly chosen", async ({
  page,
  request,
}) => {
  const ids: number[] = [];
  for (const name of ["Picker A", "Picker B", "Picker C", "Picker D"]) {
    const response = await request.post("/api/accounts", {
      data: { name, currency: "INR", opening_balance_minor: 0 },
    });
    expect(response.ok()).toBeTruthy();
    ids.push((await response.json()).id as number);
  }
  const [aId, bId, cId, dId] = ids;
  const state = await (await request.get("/api/state")).json();
  for (const account of state.accounts as Array<{ id: number; hide_from_selection: boolean }>) {
    if (!ids.includes(account.id) && !account.hide_from_selection) {
      const response = await request.put(`/api/accounts/${account.id}`, {
        data: { hide_from_selection: true },
      });
      expect(response.ok()).toBeTruthy();
    }
  }

  await page.goto("/");
  await expect(page.getByTestId("transaction-account")).toHaveValue(String(aId));
  await expect(page.getByTestId("transfer-from")).toHaveValue(String(aId));
  await expect(page.getByTestId("transfer-to")).toHaveValue(String(bId));
  await page.getByTestId("transfer-from").selectOption(String(bId));
  await expect(page.getByTestId("transfer-to")).toHaveValue(String(bId));

  await page.getByTestId("transaction-name").fill("Picker transaction draft");
  await page.getByTestId("transaction-amount").fill("12.34");
  await page.getByTestId("transfer-name").fill("Picker transfer draft");
  await page.getByTestId("transfer-amount").fill("56.78");
  await page.getByTestId("submit-transfer").click();
  await expect(page.getByRole("alert")).toContainText("Choose two different accounts");
  await page.getByTestId("transaction-account").selectOption(String(bId));
  await page.getByTestId("transfer-from").selectOption(String(bId));
  await page.getByTestId("transfer-to").selectOption(String(cId));

  const hideSourceResponse = await request.put(`/api/accounts/${bId}`, {
    data: { hide_from_selection: true },
  });
  expect(hideSourceResponse.ok()).toBeTruthy();
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(page.getByTestId("transaction-account")).toHaveValue("");
  await expect(page.getByTestId("transfer-from")).toHaveValue("");
  await expect(page.getByTestId("transfer-to")).toHaveValue(String(cId));
  await expect(page.getByTestId("transaction-name")).toHaveValue("Picker transaction draft");
  await expect(page.getByTestId("transaction-amount")).toHaveValue("12.34");
  await expect(page.getByTestId("transfer-name")).toHaveValue("Picker transfer draft");
  await expect(page.getByTestId("transfer-amount")).toHaveValue("56.78");

  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(page.getByTestId("transaction-account")).toHaveValue("");
  await expect(page.getByTestId("transfer-from")).toHaveValue("");
  await expect(page.getByTestId("transfer-to")).toHaveValue(String(cId));

  await page.getByTestId("transaction-account").selectOption(String(aId));
  await page.getByTestId("transfer-from").selectOption(String(aId));
  await expect(page.getByTestId("transaction-account")).toHaveValue(String(aId));
  await expect(page.getByTestId("transfer-from")).toHaveValue(String(aId));
  await expect(page.getByTestId("transfer-to")).toHaveValue(String(cId));

  const hideDestinationResponse = await request.put(`/api/accounts/${cId}`, {
    data: { hide_from_selection: true },
  });
  expect(hideDestinationResponse.ok()).toBeTruthy();
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(page.getByTestId("transfer-to")).toHaveValue("");
  await expect(page.getByTestId("transfer-from")).toHaveValue(String(aId));
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(page.getByTestId("transfer-to")).toHaveValue("");

  await page.getByTestId("transfer-from").selectOption(String(dId));
  await expect(page.getByTestId("transfer-to")).toHaveValue("");
  await page.getByTestId("transfer-to").selectOption(String(aId));
  await expect(page.getByTestId("transfer-to")).toHaveValue(String(aId));

  await page.reload();
  await expect(page.getByTestId("transaction-account")).toHaveValue(String(aId));
  await expect(page.getByTestId("transfer-from")).toHaveValue(String(aId));
  await expect(page.getByTestId("transfer-to")).toHaveValue(String(dId));
});

test("account editor sends only changed fields and preserves concurrent metadata", async ({
  page,
  request,
}) => {
  const createdResponse = await request.post("/api/accounts", {
    data: {
      name: "Diff account",
      currency: "INR",
      opening_balance_minor: 0,
      note: "Original note",
      opening_date: "2024-01-01",
    },
  });
  expect(createdResponse.ok()).toBeTruthy();
  const accountId = (await createdResponse.json()).id as number;
  await page.goto("/");
  const card = page.locator(`[data-account-name="Diff account"]`);
  await card.getByTestId(`account-type-edit-${accountId}`).click();

  const externalResponse = await request.put(`/api/accounts/${accountId}`, {
    data: { note: "External note", opening_date: "2025-05-06" },
  });
  expect(externalResponse.ok()).toBeTruthy();
  await card.getByTestId(`edit-account-name-${accountId}`).fill("Diff renamed");
  await card.getByTestId(`save-account-type-${accountId}`).click();

  const renamedCard = page.locator('[data-account-name="Diff renamed"]');
  await expect(renamedCard.getByTestId(`account-note-${accountId}`)).toHaveText("External note");
  await expect(renamedCard.getByTestId(`account-opening-date-${accountId}`)).toContainText(
    "2025-05-06",
  );

  await renamedCard.getByTestId(`account-type-edit-${accountId}`).click();
  await renamedCard.getByTestId(`edit-account-note-${accountId}`).fill("");
  await renamedCard.getByTestId(`edit-account-opening-date-${accountId}`).fill("");
  await renamedCard.getByTestId(`save-account-type-${accountId}`).click();
  await expect(renamedCard.getByTestId(`account-note-${accountId}`)).toHaveCount(0);
  await expect(renamedCard.getByTestId(`account-opening-date-${accountId}`)).toContainText("Not set");
  await page.reload();
  await expect(renamedCard.getByTestId(`account-note-${accountId}`)).toHaveCount(0);
  await expect(renamedCard.getByTestId(`account-opening-date-${accountId}`)).toContainText("Not set");
});

test("credit card metadata and signed latest balance follow purchases and payments", async ({
  page,
  request,
}) => {
  await page.goto("/");
  await createAccount(page, "F03 Bank", "100000", "checking");
  await page.getByTestId("account-name").fill("F03 Card");
  await page.getByTestId("account-type").selectOption("credit_card");
  await page.getByTestId("account-credit-limit").fill("14000");
  await page.getByTestId("account-cutoff-day").fill("18");
  await page.getByTestId("account-due-day").fill("2");
  await page.getByTestId("opening-balance").fill("0");
  await page.getByTestId("create-account").click();

  const initialState = await (await request.get("/api/state")).json();
  const cardId = initialState.accounts.find(
    (account: { name: string }) => account.name === "F03 Card",
  ).id as number;
  let card = page.locator('[data-account-name="F03 Card"]');
  await expect(card.getByTestId(`account-credit-limit-${cardId}`)).toContainText("14,000.00");
  await expect(card.getByTestId(`account-cutoff-day-${cardId}`)).toContainText("18");
  await expect(card.getByTestId(`account-due-day-${cardId}`)).toContainText("2");
  await expect(card.getByTestId(`account-balance-state-${cardId}`)).toContainText("Settled");
  await page.reload();
  card = page.locator('[data-account-name="F03 Card"]');
  await expect(card.getByTestId(`account-credit-limit-${cardId}`)).toContainText("14,000.00");

  await page.getByTestId("transaction-name").fill("F03 card purchase");
  await page.getByTestId("transaction-account").selectOption({ label: "F03 Card · INR" });
  await page.getByTestId("transaction-amount").fill("166.12");
  await page.getByRole("button", { name: "Record expense", exact: true }).click();
  await expect(balance(page, "F03 Card")).toContainText("-₹166.12");
  await expect(card.getByTestId(`account-balance-state-${cardId}`)).toContainText("Amount owed");
  const expenseBeforePayment = await page.getByTestId("expense-total").textContent();

  await page.getByTestId("transfer-name").fill("F03 card payment");
  await page.getByTestId("transfer-from").selectOption({ label: "F03 Bank" });
  await page.getByTestId("transfer-to").selectOption({ label: "F03 Card" });
  await page.getByTestId("transfer-amount").fill("100");
  await page.getByTestId("submit-transfer").click();
  await expect(balance(page, "F03 Card")).toContainText("-₹66.12");
  await expect(balance(page, "F03 Bank")).toContainText("99,900.00");
  await expect(page.getByTestId("expense-total")).toHaveText(expenseBeforePayment || "");

  await card.getByTestId(`account-type-edit-${cardId}`).click();
  await card.getByTestId(`edit-account-credit-limit-${cardId}`).fill("15000");
  await card.getByTestId(`edit-account-cutoff-day-${cardId}`).fill("20");
  await card.getByTestId(`edit-account-due-day-${cardId}`).fill("3");
  await card.getByTestId(`save-account-type-${cardId}`).click();
  await expect(card.getByTestId(`account-credit-limit-${cardId}`)).toContainText("15,000.00");
  await expect(card.getByTestId(`account-cutoff-day-${cardId}`)).toContainText("20");
  await expect(card.getByTestId(`account-due-day-${cardId}`)).toContainText("3");

  await card.getByTestId(`account-type-edit-${cardId}`).click();
  await card.getByTestId(`edit-account-type-${cardId}`).selectOption("checking");
  await card.getByTestId(`save-account-type-${cardId}`).click();
  await expect(card.getByTestId(`account-credit-limit-${cardId}`)).toHaveCount(0);
  await card.getByTestId(`account-type-edit-${cardId}`).click();
  await card.getByTestId(`edit-account-type-${cardId}`).selectOption("credit_card");
  await card.getByTestId(`save-account-type-${cardId}`).click();
  await expect(card.getByTestId(`account-credit-limit-${cardId}`)).toContainText("15,000.00");
  await expect(card.getByTestId(`account-cutoff-day-${cardId}`)).toContainText("20");
  await expect(card.getByTestId(`account-due-day-${cardId}`)).toContainText("3");

  await card.getByTestId(`account-type-edit-${cardId}`).click();
  await card.getByTestId(`edit-account-credit-limit-${cardId}`).fill("");
  await card.getByTestId(`edit-account-cutoff-day-${cardId}`).fill("");
  await card.getByTestId(`edit-account-due-day-${cardId}`).fill("");
  await card.getByTestId(`save-account-type-${cardId}`).click();
  await expect(card.getByTestId(`account-credit-limit-${cardId}`)).toContainText("Not set");
  await expect(card.getByTestId(`account-cutoff-day-${cardId}`)).toContainText("Not set");
  await expect(card.getByTestId(`account-due-day-${cardId}`)).toContainText("Not set");

  await page.getByTestId("transfer-amount").fill("66.12");
  await page.getByTestId("submit-transfer").click();
  await expect(balance(page, "F03 Card")).toContainText("0.00");
  await expect(card.getByTestId(`account-balance-state-${cardId}`)).toContainText("Settled");
  await page.getByTestId("transfer-amount").fill("1");
  await page.getByTestId("submit-transfer").click();
  await expect(balance(page, "F03 Card")).toContainText("₹1.00");
  await expect(card.getByTestId(`account-balance-state-${cardId}`)).toContainText("Credit balance");
});
