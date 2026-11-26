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
