import { readFile } from "node:fs/promises";
import { expect, test, type Page } from "@playwright/test";

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

async function createAccount(page: import("@playwright/test").Page, name: string, opening: string) {
  await page.getByTestId("account-name").fill(name);
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
