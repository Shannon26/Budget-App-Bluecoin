import { expect, test } from "@playwright/test";

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
