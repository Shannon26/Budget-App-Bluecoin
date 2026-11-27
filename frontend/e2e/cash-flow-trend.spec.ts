import { expect, test } from "@playwright/test";

test("cash flow trend reconciles with the existing reports", async ({ page, request }) => {
  const accountResponse = await request.post("/api/accounts", {
    data: { name: "Trend browser account", currency: "INR", opening_balance_minor: 10000 },
  });
  expect(accountResponse.ok()).toBeTruthy();
  const account = await accountResponse.json();
  const entryResponse = await request.post("/api/transactions", {
    data: { kind: "expense", account_id: account.id, amount_minor: 1250, name: "Trend expense", occurred_at: "2026-11-10" },
  });
  expect(entryResponse.ok()).toBeTruthy();
  const earnings = await (await request.get("/api/reports/net-earnings?month=2026-11")).json();
  const balance = await (await request.get("/api/reports/balance-sheet?month=2026-11")).json();
  const trendResponse = await request.get("/api/reports/cash-flow-trend?month=2026-11&months=2");
  expect(trendResponse.ok()).toBeTruthy();
  const trend = await trendResponse.json();
  expect(trend.points[1].cash_flow).toEqual(earnings.current);
  expect(trend.points[1].net_worth_minor).toBe(balance.current_net_worth_minor);
  await page.goto("/");
  await page.getByTestId("nav-overview").click();
  await page.getByTestId("nav-cash-flow").click();
  await page.getByLabel("Ending month").fill("2026-11");
  await page.getByLabel("Months").selectOption("2");
  const row = page.getByRole("row", { name: /2026-11-30/ });
  await expect(row).toContainText("12.50");
  await expect(row).toContainText("87.50");
});

test("cash flow report shows errors and recovers on retry", async ({ page }) => {
  let fail = true;
  await page.route("**/api/reports/cash-flow-trend?*", async (route) => {
    if (fail) {
      await route.fulfill({ status: 500, json: { error: "Report unavailable" } });
    } else {
      await route.continue();
    }
  });
  await page.goto("/");
  await page.getByTestId("nav-overview").click();
  await page.getByTestId("nav-cash-flow").click();
  await expect(page.getByRole("alert")).toContainText("Report unavailable");
  fail = false;
  await page.getByRole("button", { name: "Try again" }).click();
  await expect(page.getByRole("table", { name: "Cash flow comparison" })).toBeVisible();
});
