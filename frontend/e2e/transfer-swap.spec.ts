import { readFile } from "node:fs/promises";
import { expect, test, type APIRequestContext, type Page, type Request } from "@playwright/test";
import type { BudgetState, NetEarningsReport } from "../src/shared/types";

const SOURCE_NAME = "F24 swap checking";
const DESTINATION_NAME = "F24 swap savings";
const HIDDEN_NAME = "F24 swap hidden";
const TRANSFER_NAME = "F24 swap transfer";
const TRANSFER_NOTE = "Keep this note, status, date, and labels";
const TRANSFER_DATE = "2026-11-20";
const REPORT_MONTH = "2026-11";
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

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

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

async function getState(request: APIRequestContext): Promise<BudgetState> {
  const response = await request.get("/api/state");
  expect(response.ok()).toBeTruthy();
  return (await response.json()) as BudgetState;
}

async function getReport(request: APIRequestContext): Promise<NetEarningsReport> {
  const response = await request.get(`/api/reports/net-earnings?month=${REPORT_MONTH}`);
  expect(response.ok()).toBeTruthy();
  return (await response.json()) as NetEarningsReport;
}

async function draftSnapshot(page: Page, kind: "create" | "edit") {
  const scope =
    kind === "create"
      ? page.locator(".transfer-panel")
      : page.getByRole("dialog", { name: "Edit entry" });
  const nameId = kind === "create" ? "transfer-name" : "edit-name";
  const fromId = kind === "create" ? "transfer-from" : "edit-account";
  const toId = kind === "create" ? "transfer-to" : "edit-counterpart";
  const amountId = kind === "create" ? "transfer-amount" : "edit-amount";

  return {
    name: await scope.getByTestId(nameId).inputValue(),
    from: await scope.getByTestId(fromId).inputValue(),
    to: await scope.getByTestId(toId).inputValue(),
    amount: await scope.getByTestId(amountId).inputValue(),
    date: await scope.locator('input[type="date"]').inputValue(),
    note:
      kind === "create"
        ? await scope.getByTestId("transfer-note").inputValue()
        : await scope.locator("textarea").inputValue(),
    reconciled: await scope.locator(".check-label input").isChecked(),
    labels: await scope.locator(".label-option input:checked").evaluateAll((inputs) =>
      inputs.map((input) => input.getAttribute("data-testid")).sort(),
    ),
  };
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

test("transfer endpoint swaps are explicit, keyboard accessible, and persisted only by submit/save", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);

  const sourceId = await createAccount(request, SOURCE_NAME, 100_000);
  const destinationId = await createAccount(request, DESTINATION_NAME, 50_000);
  const hiddenId = await createAccount(request, HIDDEN_NAME, 25_000, true);
  const labelResponse = await request.post("/api/labels", {
    data: { name: "F24 preserved label" },
  });
  expect(labelResponse.ok()).toBeTruthy();
  const labelId = (await labelResponse.json()).id as number;

  for (const transaction of [
    {
      kind: "income",
      account_id: sourceId,
      amount_minor: 12_000,
      name: "F24 unchanged income",
      occurred_at: "2026-11-18T12:00:00",
    },
    {
      kind: "expense",
      account_id: sourceId,
      amount_minor: 3_000,
      name: "F24 unchanged expense",
      occurred_at: "2026-11-19T12:00:00",
    },
  ]) {
    const response = await request.post("/api/transactions", { data: transaction });
    expect(response.ok()).toBeTruthy();
  }

  const baselineState = await getState(request);
  const baselineReport = await getReport(request);
  expect(baselineState.totals).toEqual({ income_minor: 12_000, expense_minor: 3_000 });

  await page.goto("/");
  await expect(page.locator(`[data-account-name="${SOURCE_NAME}"]`)).toBeVisible();
  await expect(page.locator(`[data-account-name="${DESTINATION_NAME}"]`)).toBeVisible();
  await expect(page.getByTestId("transfer-from").locator(`option[value="${hiddenId}"]`)).toHaveCount(0);
  await expect(page.getByTestId("transfer-to").locator(`option[value="${hiddenId}"]`)).toHaveCount(0);

  const createSwap = page.getByRole("button", { name: "Swap accounts", exact: true });
  await page.getByTestId("transfer-from").selectOption(String(sourceId));
  await page.getByTestId("transfer-to").selectOption(String(sourceId));
  await expect(createSwap).toBeDisabled();
  await page.getByTestId("transfer-to").selectOption("");
  await expect(createSwap).toBeDisabled();
  await page.getByTestId("transfer-to").selectOption(String(destinationId));
  await expect(createSwap).toBeEnabled();

  await page.getByTestId("transfer-name").fill(TRANSFER_NAME);
  await page.getByTestId("transfer-amount").fill("250.00");
  await page.locator('.transfer-panel input[type="date"]').fill(TRANSFER_DATE);
  await page.getByTestId("transfer-note").fill(TRANSFER_NOTE);
  await page.getByTestId(`transfer-label-${labelId}`).check();
  await page.locator(".transfer-panel .check-label input").check();

  const beforeCreateSwap = await draftSnapshot(page, "create");
  const createWrites = watchApiWrites(page);
  await createSwap.focus();
  await page.keyboard.press("Enter");
  await expect(page.getByTestId("transfer-from")).toHaveValue(String(destinationId));
  await expect(page.getByTestId("transfer-to")).toHaveValue(String(sourceId));
  expect(await draftSnapshot(page, "create")).toEqual({
    ...beforeCreateSwap,
    from: beforeCreateSwap.to,
    to: beforeCreateSwap.from,
  });
  expect(createWrites.writes).toEqual([]);

  const createStarted = deferred();
  const releaseCreate = deferred();
  await page.route("**/api/transfers", async (route) => {
    if (route.request().method() !== "POST") {
      await route.continue();
      return;
    }
    createStarted.resolve();
    const response = await route.fetch();
    await releaseCreate.promise;
    await route.fulfill({ response });
  });
  await page.getByTestId("submit-transfer").click();
  await createStarted.promise;
  await expect(page.getByTestId("submit-transfer")).toContainText("Moving…");
  await expect(createSwap).toBeDisabled();
  releaseCreate.resolve();
  await expect(page.getByTestId("transfer-name")).toHaveValue("");
  await expect(page.getByTestId("submit-transfer")).toBeEnabled();
  await page.unroute("**/api/transfers");
  expect(createWrites.writes).toEqual(["POST /api/transfers"]);
  createWrites.stop();

  const createdState = await getState(request);
  const createdTransfer = createdState.entries.find((entry) => entry.name === TRANSFER_NAME)!;
  expect(createdTransfer).toMatchObject({
    kind: "transfer",
    account_id: destinationId,
    counterpart_account_id: sourceId,
    amount_minor: 25_000,
    note: TRANSFER_NOTE,
    occurred_at: `${TRANSFER_DATE}T12:00:00`,
    reconciled: true,
    labels: [{ id: labelId, name: "F24 preserved label" }],
  });
  expect(createdState.accounts.find((account) => account.id === sourceId)?.balance_minor).toBe(134_000);
  expect(createdState.accounts.find((account) => account.id === destinationId)?.balance_minor).toBe(25_000);
  expect(createdState.totals).toEqual(baselineState.totals);
  expect((await getReport(request)).current).toEqual(baselineReport.current);

  await page.getByTestId("nav-ledger").click();
  await page.getByTestId("ledger-search").fill(TRANSFER_NAME);
  await page.getByTestId("apply-filters").click();
  const transferRow = page
    .locator('[data-testid^="ledger-row-transfer-"]')
    .filter({ hasText: TRANSFER_NAME });
  await expect(transferRow).toHaveCount(1);
  await expect(transferRow).toContainText(`${DESTINATION_NAME} → ${SOURCE_NAME}`);
  const transferRowId = await transferRow.getAttribute("data-testid");
  const transferId = Number(transferRowId!.split("-").at(-1));
  expect(transferId).toBe(createdTransfer.id);

  await transferRow.click();
  await page.getByRole("dialog", { name: "Entry details" }).getByRole("button", { name: "Edit", exact: true }).click();
  const editor = page.getByRole("dialog", { name: "Edit entry" });
  const editSwap = page.getByRole("button", { name: "Swap accounts", exact: true });
  await expect(editor.getByTestId("edit-account").locator(`option[value="${hiddenId}"]`)).toHaveCount(1);
  await expect(editor.getByTestId("edit-account").locator(`option[value="${hiddenId}"]`)).toContainText(
    "hidden from new entries",
  );

  await editor.getByTestId("edit-account").selectOption(String(sourceId));
  await expect(editor.getByRole("alert")).toContainText("Choose two different accounts");
  await expect(editor.getByTestId("save-edit")).toBeDisabled();
  await expect(editSwap).toBeDisabled();
  await editor.getByTestId("edit-account").selectOption(String(destinationId));
  await expect(editor.getByTestId("save-edit")).toBeEnabled();
  await expect(editSwap).toBeEnabled();

  const beforeEditSwap = await draftSnapshot(page, "edit");
  const editWrites = watchApiWrites(page);
  await editSwap.focus();
  await page.keyboard.press("Space");
  await expect(editor.getByTestId("edit-account")).toHaveValue(String(sourceId));
  await expect(editor.getByTestId("edit-counterpart")).toHaveValue(String(destinationId));
  expect(await draftSnapshot(page, "edit")).toEqual({
    ...beforeEditSwap,
    from: beforeEditSwap.to,
    to: beforeEditSwap.from,
  });
  expect(editWrites.writes).toEqual([]);

  const saveStarted = deferred();
  const releaseSave = deferred();
  await page.route(`**/api/transfers/${transferId}`, async (route) => {
    if (route.request().method() !== "PUT") {
      await route.continue();
      return;
    }
    saveStarted.resolve();
    const response = await route.fetch();
    await releaseSave.promise;
    await route.fulfill({ response });
  });
  await editor.getByTestId("save-edit").click();
  await saveStarted.promise;
  await expect(editor.getByTestId("save-edit")).toContainText("Saving…");
  await expect(editSwap).toBeDisabled();
  releaseSave.resolve();
  await expect(editor).toBeHidden();
  await page.unroute(`**/api/transfers/${transferId}`);
  expect(editWrites.writes).toEqual([`PUT /api/transfers/${transferId}`]);
  editWrites.stop();

  await expect(transferRow).toHaveCount(1);
  await expect(transferRow).toContainText(`${SOURCE_NAME} → ${DESTINATION_NAME}`);
  await expect(transferRow).toContainText("F24 preserved label");
  await expect(transferRow).toContainText("Reconciled");

  const finalState = await getState(request);
  const finalTransfer = finalState.entries.find((entry) => entry.name === TRANSFER_NAME)!;
  expect(finalTransfer).toEqual({
    ...createdTransfer,
    account_id: sourceId,
    account_name: SOURCE_NAME,
    counterpart_account_id: destinationId,
    counterpart_account_name: DESTINATION_NAME,
  });
  expect(finalState.accounts.find((account) => account.id === sourceId)?.balance_minor).toBe(84_000);
  expect(finalState.accounts.find((account) => account.id === destinationId)?.balance_minor).toBe(75_000);
  expect(finalState.accounts.find((account) => account.id === hiddenId)?.balance_minor).toBe(25_000);
  expect(finalState.totals).toEqual(baselineState.totals);
  expect(finalState.entries.filter((entry) => entry.kind === "transfer" && entry.name === TRANSFER_NAME)).toHaveLength(1);
  const finalReport = await getReport(request);
  expect(finalReport.current).toEqual(baselineReport.current);
  expect(finalReport.prior).toEqual(baselineReport.prior);

  await page.getByTestId("nav-overview").click();
  await expect(page.locator(`[data-account-name="${SOURCE_NAME}"]`).getByTestId("account-balance")).toContainText("840.00");
  await expect(page.locator(`[data-account-name="${DESTINATION_NAME}"]`).getByTestId("account-balance")).toContainText("750.00");
  await expect(page.getByTestId("income-total")).toContainText("120.00");
  await expect(page.getByTestId("expense-total")).toContainText("30.00");

  for (const history of [
    {
      accountId: sourceId,
      kind: "transfer_out",
      absentKind: "transfer_in",
      sign: "−",
      fromId: sourceId,
      toId: destinationId,
    },
    {
      accountId: destinationId,
      kind: "transfer_in",
      absentKind: "transfer_out",
      sign: "+",
      fromId: sourceId,
      toId: destinationId,
    },
  ]) {
    await page.getByTestId("nav-overview").click();
    await page.getByTestId(`account-history-${history.accountId}`).click();
    const historyRow = page
      .locator(`[data-testid^="ledger-row-${history.kind}-"]`)
      .filter({ hasText: TRANSFER_NAME });
    const absentRow = page
      .locator(`[data-testid^="ledger-row-${history.absentKind}-"]`)
      .filter({ hasText: TRANSFER_NAME });
    await expect(historyRow).toHaveCount(1);
    await expect(absentRow).toHaveCount(0);
    await expect(historyRow).toContainText(`${history.sign}₹250.00`);
    await historyRow.click();
    await page.getByRole("dialog", { name: "Entry details" }).getByRole("button", { name: "Edit", exact: true }).click();
    await expect(editor.getByTestId("edit-account")).toHaveValue(String(history.fromId));
    await expect(editor.getByTestId("edit-counterpart")).toHaveValue(String(history.toId));
    await expect(editSwap).toBeEnabled();
    await editor.getByRole("button", { name: "Close editor" }).click();
    await page.getByRole("dialog", { name: "Entry details" }).getByRole("button", { name: "Close details" }).click();
  }

  await page.getByTestId("nav-overview").click();
  await page.getByTestId("nav-ledger").click();
  await page.getByTestId("ledger-search").fill(TRANSFER_NAME);
  await page.getByTestId("apply-filters").click();
  const exported = await downloadLedgerCsv(page);
  expect(exported[0]).toEqual(CSV_HEADERS);
  expect(exported.slice(1)).toEqual([
    [
      TRANSFER_DATE,
      "Transfer",
      TRANSFER_NAME,
      "",
      SOURCE_NAME,
      DESTINATION_NAME,
      "",
      "",
      "250.00",
      "INR",
      "Reconciled",
      TRANSFER_NOTE,
    ],
  ]);

  await page.getByTestId("nav-net-earnings").click();
  await page.getByTestId("report-month").fill(REPORT_MONTH);
  await expect(page.getByTestId("report-income-current")).toHaveText("₹120.00");
  await expect(page.getByTestId("report-expense-current")).toHaveText("₹30.00");
  await expect(page.getByTestId("report-net-current")).toHaveText("₹90.00");
});
