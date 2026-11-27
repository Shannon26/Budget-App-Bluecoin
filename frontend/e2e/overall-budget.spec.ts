import { expect, test, type APIRequestContext } from "@playwright/test";

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

async function createAccount(request: APIRequestContext, name: string, opening = 0): Promise<number> {
  const response = await request.post("/api/accounts", {
    data: { name, currency: "INR", opening_balance_minor: opening },
  });
  expect(response.ok()).toBeTruthy();
  return (await response.json()).id as number;
}

async function createTransaction(
  request: APIRequestContext,
  data: {
    kind: "income" | "expense";
    account_id: number;
    amount_minor: number;
    name: string;
    category_id?: number | null;
    occurred_at: string;
  },
): Promise<number> {
  const response = await request.post("/api/transactions", { data });
  expect(response.ok()).toBeTruthy();
  return (await response.json()).id as number;
}

test("overall monthly cap tracks raw spending and persists independently of category budgets", async ({
  page,
  request,
}) => {
  const month = "2039-11";
  const nextMonth = "2039-12";
  const checking = await createAccount(request, "F17 checking", 1000000);
  const savings = await createAccount(request, "F17 savings");
  const reportHidden = await createAccount(request, "F17 report hidden");
  expect(
    (
      await request.put(`/api/accounts/${reportHidden}`, {
        data: { hide_from_reports: true },
      })
    ).ok(),
  ).toBeTruthy();
  const initialState = await (await request.get("/api/state")).json();
  const foodId = initialState.categories.find(
    (category: { name: string; kind: string }) => category.name === "Food & Drink" && category.kind === "expense",
  ).id as number;
  const groceriesId = initialState.categories.find(
    (category: { name: string }) => category.name === "Groceries",
  ).id as number;
  const diningId = initialState.categories.find(
    (category: { name: string }) => category.name === "Dining Out",
  ).id as number;

  const groceriesEntry = await createTransaction(request, {
    kind: "expense",
    account_id: checking,
    amount_minor: 100000,
    name: "F17 groceries",
    category_id: groceriesId,
    occurred_at: `${month}-05`,
  });
  await createTransaction(request, {
    kind: "expense",
    account_id: checking,
    amount_minor: 180000,
    name: "F17 dining",
    category_id: diningId,
    occurred_at: `${month}-06`,
  });
  await createTransaction(request, {
    kind: "expense",
    account_id: checking,
    amount_minor: 30000,
    name: "F17 uncategorized",
    occurred_at: `${month}-07`,
  });
  await createTransaction(request, {
    kind: "expense",
    account_id: reportHidden,
    amount_minor: 5000,
    name: "F17 report-hidden expense",
    category_id: groceriesId,
    occurred_at: `${month}-08`,
  });
  await createTransaction(request, {
    kind: "income",
    account_id: checking,
    amount_minor: 500000,
    name: "F17 income excluded",
    occurred_at: `${month}-09`,
  });
  expect(
    (
      await request.post("/api/transfers", {
        data: {
          from_account_id: checking,
          to_account_id: savings,
          amount_minor: 100000,
          name: "F17 transfer excluded",
          occurred_at: `${month}-10`,
        },
      })
    ).ok(),
  ).toBeTruthy();
  expect((await request.delete(`/api/categories/${diningId}`)).ok()).toBeTruthy();

  await page.goto("/");
  await page.getByTestId("nav-budgets").click();
  await page.getByTestId("budget-month").fill(month);
  await expect(page.getByTestId("budget-total-spent")).toHaveText("₹3,150.00");
  await expect(page.getByTestId("overall-budget-spent")).toHaveText("₹3,150.00");
  await expect(page.getByTestId("overall-budget-limit")).toHaveText("No cap");
  await expect(page.getByTestId("overall-budget-remaining")).toHaveText("—");

  await page.getByTestId("set-overall-budget").click();
  await page.getByTestId("overall-budget-amount").fill("2500");
  await page.getByTestId("save-overall-budget").click();
  await expect(page.getByTestId("overall-budget-limit")).toHaveText("₹2,500.00");
  await expect(page.getByTestId("overall-budget-remaining")).toHaveText("₹650.00 over");
  await expect(page.getByTestId("overall-budget-progress")).toHaveAttribute(
    "aria-valuetext",
    "126% of cap used",
  );

  const foodRow = page.getByTestId(`budget-row-${foodId}`);
  await foodRow.getByRole("button", { name: "Set limit", exact: true }).click();
  await page.getByTestId("budget-limit-amount").fill("5000");
  await page.getByTestId("save-budget-limit").click();
  await expect(foodRow.getByTestId(`budget-limit-${foodId}`)).toHaveText("₹5,000.00");
  await expect(page.getByTestId("overall-budget-limit")).toHaveText("₹2,500.00");
  await expect(page.getByTestId(`budget-row-${diningId}`)).toContainText("Archived");

  await page.getByTestId("edit-overall-budget").click();
  await page.getByTestId("overall-budget-amount").fill("4000");
  await page.getByTestId("save-overall-budget").click();
  await expect(page.getByTestId("overall-budget-remaining")).toHaveText("₹850.00 left");

  await page.reload();
  await page.getByTestId("nav-budgets").click();
  await page.getByTestId("budget-month").fill(month);
  await expect(page.getByTestId("overall-budget-limit")).toHaveText("₹4,000.00");
  await expect(foodRow.getByTestId(`budget-limit-${foodId}`)).toHaveText("₹5,000.00");

  expect(
    (
      await request.put(`/api/transactions/${groceriesEntry}`, {
        data: {
          kind: "expense",
          account_id: checking,
          amount_minor: 120000,
          name: "F17 groceries edited",
          category_id: groceriesId,
          occurred_at: `${month}-05`,
        },
      })
    ).ok(),
  ).toBeTruthy();
  await page.reload();
  await page.getByTestId("nav-budgets").click();
  await page.getByTestId("budget-month").fill(month);
  await expect(page.getByTestId("overall-budget-spent")).toHaveText("₹3,350.00");
  await expect(page.getByTestId("overall-budget-remaining")).toHaveText("₹650.00 left");

  await page.getByTestId("budget-month").fill(nextMonth);
  await expect(page.getByTestId("overall-budget-spent")).toHaveText("₹0.00");
  await expect(page.getByTestId("overall-budget-limit")).toHaveText("No cap");
  await page.getByTestId("set-overall-budget").click();
  await page.getByTestId("overall-budget-amount").fill("1500");
  await page.getByTestId("save-overall-budget").click();
  await expect(page.getByTestId("overall-budget-remaining")).toHaveText("₹1,500.00 left");

  await page.getByTestId("budget-month").fill(month);
  await expect(page.getByTestId("overall-budget-limit")).toHaveText("₹4,000.00");
  await page.getByTestId("budget-month").fill(nextMonth);
  await expect(page.getByTestId("overall-budget-limit")).toHaveText("₹1,500.00");
  await page.getByTestId("remove-overall-budget").click();
  await expect(page.getByTestId("overall-budget-limit")).toHaveText("No cap");
  await expect(page.getByTestId("overall-budget-remaining")).toHaveText("—");
  await page.getByTestId("budget-month").fill(month);
  await expect(page.getByTestId("overall-budget-limit")).toHaveText("₹4,000.00");
});

test("overall cap keeps loading error and stale month responses honest", async ({ page, request }) => {
  const monthA = "2040-01";
  const monthB = "2040-02";
  const monthC = "2040-03";
  const account = await createAccount(request, "F17 state checking");
  await createTransaction(request, {
    kind: "expense",
    account_id: account,
    amount_minor: 12345,
    name: "F17 January expense",
    occurred_at: `${monthA}-10`,
  });
  await createTransaction(request, {
    kind: "expense",
    account_id: account,
    amount_minor: 67890,
    name: "F17 February expense",
    occurred_at: `${monthB}-10`,
  });
  expect((await request.put(`/api/overall-budget?month=${monthA}`, { data: { amount_minor: 100000 } })).ok()).toBeTruthy();
  expect((await request.put(`/api/overall-budget?month=${monthB}`, { data: { amount_minor: 50000 } })).ok()).toBeTruthy();

  const januaryStarted = deferred();
  const releaseJanuary = deferred();
  const januaryFinished = deferred();
  let januaryLoads = 0;
  let februarySaveAttempts = 0;
  await page.route(/\/api\/category-budgets\?month=2040-01$/, async (route) => {
    if (januaryLoads++ > 0) {
      await route.continue();
      return;
    }
    januaryStarted.resolve();
    await route.fetch();
    await releaseJanuary.promise;
    await route.fulfill({
      status: 500,
      contentType: "application/json",
      body: JSON.stringify({ error: "Stale January failure" }),
    });
    januaryFinished.resolve();
  });
  await page.route(/\/api\/category-budgets\?month=2040-03$/, async (route) => {
    await route.fulfill({
      status: 500,
      contentType: "application/json",
      body: JSON.stringify({ error: "Could not load budgets" }),
    });
  });
  const staleSaveStarted = deferred();
  const releaseStaleSave = deferred();
  const staleSaveFinished = deferred();
  await page.route(/\/api\/overall-budget\?month=2040-01$/, async (route) => {
    if (route.request().method() !== "PUT") {
      await route.continue();
      return;
    }
    staleSaveStarted.resolve();
    const response = await route.fetch();
    await releaseStaleSave.promise;
    await route.fulfill({ response });
    staleSaveFinished.resolve();
  });
  await page.route(/\/api\/overall-budget\?month=2040-02$/, async (route) => {
    if (route.request().method() !== "PUT" || februarySaveAttempts++ > 0) {
      await route.continue();
      return;
    }
    await route.fulfill({
      status: 500,
      contentType: "application/json",
      body: JSON.stringify({ error: "Could not save overall budget" }),
    });
  });

  await page.goto("/");
  await page.getByTestId("nav-budgets").click();
  await page.getByTestId("budget-month").fill(monthA);
  await januaryStarted.promise;
  await expect(page.getByText("Loading budgets…")).toBeVisible();
  await expect(page.getByTestId("overall-budget-spent")).toHaveText("Pending…");
  await expect(page.getByTestId("edit-overall-budget")).toHaveCount(0);

  await page.getByTestId("budget-month").fill(monthB);
  await expect(page.getByTestId("overall-budget-spent")).toHaveText("₹678.90");
  await expect(page.getByTestId("overall-budget-limit")).toHaveText("₹500.00");
  await expect(page.getByTestId("overall-budget-remaining")).toHaveText("₹178.90 over");
  releaseJanuary.resolve();
  await januaryFinished.promise;
  await expect(page.getByTestId("budget-month")).toHaveValue(monthB);
  await expect(page.getByTestId("overall-budget-spent")).toHaveText("₹678.90");
  await expect(page.getByText("Stale January failure")).toHaveCount(0);

  await page.getByTestId("budget-month").fill(monthC);
  await expect(page.getByText("Could not load budgets.")).toBeVisible();
  await expect(page.getByTestId("overall-budget-spent")).toHaveText("Unavailable");
  await page.getByTestId("budget-month").fill(monthB);
  await expect(page.getByTestId("overall-budget-spent")).toHaveText("₹678.90");

  await page.getByTestId("budget-month").fill(monthA);
  await expect(page.getByTestId("overall-budget-spent")).toHaveText("₹123.45");
  await page.getByTestId("edit-overall-budget").click();
  await page.getByTestId("overall-budget-amount").fill("2000");
  await page.getByTestId("save-overall-budget").click();
  await staleSaveStarted.promise;
  await page.getByTestId("budget-month").fill(monthB);
  await expect(page.getByTestId("overall-budget-limit")).toHaveText("₹500.00");
  releaseStaleSave.resolve();
  await staleSaveFinished.promise;
  await expect(page.getByTestId("budget-month")).toHaveValue(monthB);
  await expect(page.getByTestId("overall-budget-limit")).toHaveText("₹500.00");

  await page.getByTestId("edit-overall-budget").click();
  await page.getByTestId("overall-budget-amount").fill("600");
  await page.getByTestId("save-overall-budget").click();
  await expect(page.getByRole("alert")).toContainText("Could not save overall budget");
  await expect(page.getByTestId("overall-budget-limit")).toHaveText("₹500.00");
  await page.getByTestId("save-overall-budget").click();
  await expect(page.getByTestId("overall-budget-limit")).toHaveText("₹600.00");
  await expect(page.getByTestId("overall-budget-remaining")).toHaveText("₹78.90 over");
});
