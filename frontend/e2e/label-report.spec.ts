import { expect, test, type APIRequestContext, type Page } from "@playwright/test";

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

async function createLabel(request: APIRequestContext, name: string): Promise<number> {
  const response = await request.post("/api/labels", { data: { name } });
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
    occurred_at: string;
    labels?: number[];
  },
): Promise<void> {
  const response = await request.post("/api/transactions", { data });
  expect(response.ok()).toBeTruthy();
}

test("Label Report allocates ordinary posted entries once and distinguishes No Labels", async ({
  page,
  request,
}) => {
  const month = "2037-11";
  const priorMonth = "2037-10";
  const visible = await createAccount(request, "F14 visible", 900000);
  const pickerHidden = await createAccount(request, "F14 picker hidden");
  const reportHidden = await createAccount(request, "F14 report hidden");
  const destination = await createAccount(request, "F14 transfer destination");
  expect(
    (
      await request.put(`/api/accounts/${pickerHidden}`, {
        data: { hide_from_selection: true },
      })
    ).ok(),
  ).toBeTruthy();
  expect(
    (
      await request.put(`/api/accounts/${reportHidden}`, {
        data: { hide_from_reports: true },
      })
    ).ok(),
  ).toBeTruthy();

  const zulu = await createLabel(request, "Zulu");
  const alpha = await createLabel(request, "Alpha");
  const userNoLabels = await createLabel(request, "No Labels");
  await createTransaction(request, {
    kind: "expense",
    account_id: visible,
    amount_minor: 101,
    name: "F14 unlabelled",
    occurred_at: `${month}-01`,
  });
  await createTransaction(request, {
    kind: "income",
    account_id: visible,
    amount_minor: 1000,
    name: "F14 single label",
    occurred_at: `${month}-02`,
    labels: [userNoLabels],
  });
  await createTransaction(request, {
    kind: "expense",
    account_id: visible,
    amount_minor: 100,
    name: "F14 three labels",
    occurred_at: `${month}-03`,
    labels: [zulu, alpha, userNoLabels],
  });
  await createTransaction(request, {
    kind: "expense",
    account_id: pickerHidden,
    amount_minor: 50,
    name: "F14 picker-only expense",
    occurred_at: `${month}-10`,
    labels: [alpha],
  });
  await createTransaction(request, {
    kind: "income",
    account_id: reportHidden,
    amount_minor: 9999,
    name: "F14 hidden income",
    occurred_at: `${month}-10`,
    labels: [zulu],
  });
  await createTransaction(request, {
    kind: "expense",
    account_id: visible,
    amount_minor: 7,
    name: "F14 prior unlabelled",
    occurred_at: `${priorMonth}-31`,
  });
  await createTransaction(request, {
    kind: "expense",
    account_id: visible,
    amount_minor: 999,
    name: "F14 next month",
    occurred_at: "2037-12-01",
  });
  expect(
    (
      await request.post("/api/transfers", {
        data: {
          from_account_id: visible,
          to_account_id: destination,
          amount_minor: 500,
          name: "F14 excluded transfer",
          occurred_at: `${month}-12`,
          labels: [zulu, alpha, userNoLabels],
        },
      })
    ).ok(),
  ).toBeTruthy();

  await page.goto("/");

  await page.getByTestId("nav-overview").click();
  await page.getByTestId("nav-label-report").click();
  await expect(page.getByTestId("nav-label-report")).toHaveAttribute("aria-current", "page");
  const localMonth = await page.evaluate(() => {
    const now = new Date();
    return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}`;
  });
  await expect(page.getByTestId("label-report-month")).toHaveValue(localMonth);
  await page.getByTestId("label-report-month").fill(month);

  await expect(page.getByTestId("label-report-income-total")).toHaveText("₹10.00");
  await expect(page.getByTestId("label-report-expense-total")).toHaveText("₹2.51");
  await expect(page.getByTestId("label-report-net-total")).toHaveText("₹7.49");
  await expect(page.getByTestId("label-report-income-count")).toHaveText("1 income entries this month");
  await expect(page.getByTestId("label-report-expense-count")).toHaveText("3 expense entries this month");
  await expect(page.getByTestId(`label-report-label-${zulu}-expense`)).toHaveText("₹0.34");
  await expect(page.getByTestId(`label-report-label-${alpha}-expense`)).toHaveText("₹0.83");
  await expect(page.getByTestId(`label-report-label-${userNoLabels}-income`)).toHaveText("₹10.00");
  await expect(page.getByTestId(`label-report-label-${userNoLabels}-expense`)).toHaveText("₹0.33");
  await expect(page.getByTestId("label-report-no-labels-expense")).toHaveText("₹1.01");
  await expect(page.getByTestId("label-report-no-labels")).toHaveAttribute("data-no-labels", "true");
  await expect(page.getByTestId(`label-report-label-${userNoLabels}`)).toHaveAttribute(
    "data-no-labels",
    "false",
  );
  await expect(page.getByTestId("label-report-no-labels")).toContainText("Synthetic");
  await expect(page.getByTestId(`label-report-label-${userNoLabels}`)).not.toContainText("Synthetic");
  await expect(page.getByTestId("label-report-allocation-rules")).toContainText(
    "leftover paise",
  );
  await expect(page.getByTestId("label-report-allocation-rules")).toContainText(
    "ascending stable label ID",
  );
  await expect(page.getByTestId("label-report-allocated-income")).toHaveText("₹10.00");
  await expect(page.getByTestId("label-report-allocated-expense")).toHaveText("₹2.51");
  await expect(page.getByTestId("label-report-allocated-net")).toHaveText("₹7.49");

  await page.getByTestId("nav-net-earnings").click();
  await page.getByTestId("report-month").fill(month);
  await expect(page.getByTestId("report-income-current")).toHaveText("₹10.00");
  await expect(page.getByTestId("report-expense-current")).toHaveText("₹2.51");
  await expect(page.getByTestId("report-net-current")).toHaveText("₹7.49");

  await page.getByTestId("nav-label-report").click();
  await page.getByTestId("label-report-month").fill(priorMonth);
  await expect(page.getByTestId("label-report-expense-total")).toHaveText("₹0.07");
  await expect(page.getByTestId("label-report-no-labels-expense")).toHaveText("₹0.07");
  await expect(page.getByTestId(`label-report-label-${zulu}-expense`)).toHaveText("₹0.00");

  await page.getByTestId("label-report-month").fill("2039-01");
  await expect(page.getByTestId("label-report-empty")).toContainText(
    "No report-visible posted income or expenses",
  );
  await expect(page.getByTestId("label-report-income-total")).toHaveText("₹0.00");
  await expect(page.getByTestId("label-report-expense-total")).toHaveText("₹0.00");
  await expect(page.getByTestId("label-report-no-labels-income")).toHaveText("₹0.00");
  await expect(page.getByTestId("label-report-loading")).toHaveCount(0);
  await expect(page.getByTestId("label-report-error")).toHaveCount(0);
});

test("Label Report keeps loading stale error and retry states tied to the selected month", async ({
  page,
  request,
}) => {
  const monthA = "2038-01";
  const monthB = "2038-02";
  const monthC = "2038-03";
  const account = await createAccount(request, "F14 state account");
  const label = await createLabel(request, "F14 State");
  await createTransaction(request, {
    kind: "expense",
    account_id: account,
    amount_minor: 12345,
    name: "F14 state January",
    occurred_at: `${monthA}-10`,
    labels: [label],
  });
  await createTransaction(request, {
    kind: "expense",
    account_id: account,
    amount_minor: 67890,
    name: "F14 state February",
    occurred_at: `${monthB}-10`,
    labels: [label],
  });

  const januaryStarted = deferred();
  const releaseJanuary = deferred();
  const januaryFinished = deferred();
  let januaryCalls = 0;
  let marchCalls = 0;
  await page.route("**/api/reports/labels?**", async (route) => {
    const month = new URL(route.request().url()).searchParams.get("month");
    if (month === monthA && januaryCalls++ === 0) {
      januaryStarted.resolve();
      await route.fetch();
      await releaseJanuary.promise;
      await route.fulfill({
        status: 500,
        contentType: "application/json",
        body: JSON.stringify({ error: "Stale January failure" }),
      });
      januaryFinished.resolve();
      return;
    }
    if (month === monthB) {
      const response = await route.fetch();
      const body = await response.json();
      body.invalid_date_entry_count = 1;
      await route.fulfill({ status: response.status(), headers: response.headers(), json: body });
      return;
    }
    if (month === monthC && marchCalls++ === 0) {
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

  await page.getByTestId("nav-overview").click();
  await page.getByTestId("nav-label-report").click();
  await page.getByTestId("label-report-month").fill(monthA);
  await januaryStarted.promise;
  await expect(page.getByTestId("label-report-loading")).toBeVisible();
  await expect(page.getByTestId("label-report-expense-total")).toHaveText("Pending…");
  await expect(page.getByTestId(`label-report-label-${label}`)).toHaveCount(0);

  await page.getByTestId("label-report-month").fill(monthB);
  await expect(page.getByTestId("label-report-expense-total")).toHaveText("₹678.90");
  await expect(page.getByTestId(`label-report-label-${label}-expense`)).toHaveText("₹678.90");
  await expect(page.getByTestId("label-report-invalid-dates")).toContainText("1 posted entry");
  releaseJanuary.resolve();
  await januaryFinished.promise;
  await expect(page.getByTestId("label-report-month")).toHaveValue(monthB);
  await expect(page.getByTestId("label-report-expense-total")).toHaveText("₹678.90");
  await expect(page.getByTestId("label-report-error")).toHaveCount(0);
  await expect(page.getByText("Stale January failure")).toHaveCount(0);

  await page.getByTestId("label-report-month").fill(monthC);
  await expect(page.getByTestId("label-report-error")).toContainText("Selected month is unavailable");
  await expect(page.getByTestId("label-report-expense-total")).toHaveText("Unavailable");
  await page.getByRole("button", { name: "Try again" }).click();
  await expect(page.getByTestId("label-report-empty")).toBeVisible();
  await expect(page.getByTestId("label-report-expense-total")).toHaveText("₹0.00");
  await expect(page.getByTestId("label-report-error")).toHaveCount(0);
});
