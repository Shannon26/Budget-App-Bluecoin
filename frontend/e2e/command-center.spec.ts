import { expect, test, type APIRequestContext } from "@playwright/test";

function localDateValue(): string {
  const date = new Date();
  const month = `${date.getMonth() + 1}`.padStart(2, "0");
  const day = `${date.getDate()}`.padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

interface SeedRow {
  row_id: string;
  ordinal: number;
  kind: "expense" | "income" | "transfer" | "new_account";
  name: string;
  amount_minor: number | null;
  date: string;
}

/** Full snake_case row: the backend validates every field on write. */
function draftRow(seed: SeedRow) {
  return {
    row_id: seed.row_id,
    ordinal: seed.ordinal,
    kind: seed.kind,
    name: seed.name,
    amount_minor: seed.amount_minor,
    date: seed.date,
    account_id: null,
    account_name: null,
    to_account_id: null,
    to_account_name: null,
    category_id: null,
    note: "",
    status: "needs_input",
    issues: ["Choose an account"],
    manual: false,
    source: "assistant",
    proposed_row_id: null,
    account_type: null,
    opening_balance_minor: null,
    current_balance_minor: null,
    transfer_request: false,
  };
}

async function freshDraft(request: APIRequestContext) {
  const response = await request.post("/api/draft/discard");
  expect(response.ok()).toBeTruthy();
}

async function createAccount(request: APIRequestContext, name: string): Promise<number> {
  const response = await request.post("/api/accounts", {
    data: {
      name,
      currency: "INR",
      opening_balance_minor: 100000,
      account_type: "checking",
    },
  });
  expect(response.ok()).toBeTruthy();
  return (await response.json()).id as number;
}

test.beforeEach(async ({ request }) => {
  await freshDraft(request);
});

test("prompt batch resolves accounts, saves, and appears in the ledger", async ({
  page,
  request,
}) => {
  const accountId = await createAccount(request, `CC batch account ${Date.now()}`);
  const date = localDateValue();
  const lunch = draftRow({
    row_id: "cc-e2e-lunch",
    ordinal: 1,
    kind: "expense",
    name: "Lunch",
    amount_minor: 24000,
    date,
  });
  const coffee = draftRow({
    row_id: "cc-e2e-coffee",
    ordinal: 2,
    kind: "expense",
    name: "Coffee",
    amount_minor: 8000,
    date,
  });
  for (const row of [lunch, coffee]) {
    const response = await request.post("/api/draft/rows", { data: row });
    expect(response.ok()).toBeTruthy();
  }

  // The offline assistant would add no rows. Stub only the prompt response with
  // the rows already held by the reviewed server draft.
  await page.route("**/api/draft/prompt", async (route) => {
    const response = await request.get("/api/draft");
    const draft = await response.json();
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        draft: {
          ...draft,
          prompts: [
            {
              submission_id: "cc-e2e-prompt",
              seq: 1,
              role: "member",
              text: "Yesterday I paid ₹240 for lunch and ₹80 for coffee",
              created_at: new Date().toISOString(),
            },
          ],
        },
        message: "Prepared 2 rows. Choose accounts before saving.",
      }),
    });
  });

  await page.goto("/");
  await expect(page.getByTestId("nav-command-center")).toHaveAttribute("aria-current", "page");
  await expect(page.getByTestId("cc-composer")).toBeVisible();
  await page
    .getByTestId("cc-composer")
    .fill("Yesterday I paid ₹240 for lunch and ₹80 for coffee in cash.");
  await page.getByTestId("cc-prepare").click();

  const rows = page.getByTestId("cc-review-row");
  await expect(rows).toHaveCount(2);
  await expect(page.getByTestId("cc-row-issue").first()).toContainText("Choose an account");
  await expect(page.getByTestId("cc-prompt-history")).toBeVisible();

  await rows.nth(0).getByTestId("cc-row-select").check();
  await rows.nth(1).getByTestId("cc-row-select").check();
  await page.getByTestId("cc-assign-account").selectOption(String(accountId));
  await page.getByRole("button", { name: "Assign to account" }).click();
  await expect(page.getByTestId("cc-row-issue")).toHaveCount(0);

  await expect(page.getByTestId("cc-save")).toBeEnabled();
  await page.getByTestId("cc-save").click();
  await expect(page.getByTestId("cc-receipt")).toBeVisible();
  await expect(page.getByTestId("cc-receipt-saved")).toContainText("2 entries");
  await expect(page.getByTestId("cc-receipt-pending")).toContainText("No transfers");

  await page.getByTestId("nav-ledger").click();
  await expect(
    page.locator('[data-testid^="ledger-row-expense-"]').filter({ hasText: "Lunch" }),
  ).toHaveCount(1);
  await expect(
    page.locator('[data-testid^="ledger-row-expense-"]').filter({ hasText: "Coffee" }),
  ).toHaveCount(1);
});

test("manual rows save a new account and an expense against it", async ({ page }) => {
  const accountName = `CC new wallet ${Date.now()}`;
  await page.goto("/");
  await page.getByTestId("cc-add-accounts").click();
  await expect(page.getByTestId("cc-review-row")).toHaveCount(1);
  await page.getByTestId("cc-row-edit").click();
  await page.getByTestId("cc-edit-name").fill(accountName);
  await page.getByTestId("cc-edit-account-type").selectOption("checking");
  await page.getByTestId("cc-edit-opening-balance").fill("1000");
  await page.getByTestId("cc-edit-save").click();
  await expect(page.getByTestId("cc-review-row")).toContainText(accountName);

  await page.locator(".cc-back").click();
  await page.getByTestId("cc-add-manually").click();
  await expect(page.getByTestId("cc-review-row")).toHaveCount(2);
  await page.getByTestId("cc-review-row").nth(1).getByTestId("cc-row-edit").click();
  await page.getByTestId("cc-edit-name").fill("Coffee from new wallet");
  await page.getByTestId("cc-edit-amount").fill("80");
  await page.getByTestId("cc-edit-account").selectOption({ label: `New: ${accountName}` });
  await page.getByTestId("cc-edit-save").click();

  await expect(page.getByTestId("cc-save")).toBeEnabled();
  await page.getByTestId("cc-save").click();
  await expect(page.getByTestId("cc-receipt-saved")).toContainText("1 account");
  await expect(page.getByTestId("cc-receipt-saved")).toContainText("1 entry");
  await expect(page.getByTestId("cc-receipt")).toContainText(accountName);
});

test("a draft survives reload and continues from the composer", async ({ page, request }) => {
  const response = await request.post("/api/draft/rows", {
    data: draftRow({
      row_id: "cc-e2e-resume",
      ordinal: 1,
      kind: "expense",
      name: "Resumed lunch",
      amount_minor: 1200,
      date: localDateValue(),
    }),
  });
  expect(response.ok()).toBeTruthy();

  await page.goto("/");
  await expect(page.getByTestId("cc-continue-draft")).toContainText("1 row");
  await page.reload();
  await expect(page.getByTestId("cc-continue-draft")).toBeVisible();
  await page.getByTestId("cc-continue-draft").click();
  await expect(page.getByTestId("cc-review-row")).toHaveCount(1);
  await expect(page.getByTestId("cc-review-row")).toContainText("Resumed lunch");
});

test("a failed prompt keeps the text and manual save still works", async ({
  page,
  request,
}) => {
  const accountId = await createAccount(request, `CC recovery account ${Date.now()}`);
  await page.goto("/");
  await page.route("**/api/draft/prompt", async (route) => {
    await route.fulfill({
      status: 500,
      contentType: "application/json",
      body: JSON.stringify({ error: "assistant offline" }),
    });
  });

  const prompt = "Yesterday I paid ₹240 for lunch from HDFC";
  await page.getByTestId("cc-composer").fill(prompt);
  await page.getByTestId("cc-prepare").click();
  await expect(page.locator(".cc-inline-error")).toContainText("assistant offline");
  await expect(page.getByTestId("cc-composer")).toHaveValue(prompt);

  await page.getByTestId("cc-add-manually").click();
  await expect(page.getByTestId("cc-review-row")).toHaveCount(1);
  await page.getByTestId("cc-row-edit").click();
  await page.getByTestId("cc-edit-name").fill("Recovered entry");
  await page.getByTestId("cc-edit-amount").fill("240");
  await page.getByTestId("cc-edit-account").selectOption(String(accountId));
  await page.getByTestId("cc-edit-save").click();

  await expect(page.getByTestId("cc-save")).toBeEnabled();
  await page.getByTestId("cc-save").click();
  await expect(page.getByTestId("cc-receipt-saved")).toContainText("1 entry");
  await page.getByTestId("nav-ledger").click();
  await expect(
    page.locator('[data-testid^="ledger-row-expense-"]').filter({ hasText: "Recovered entry" }),
  ).toHaveCount(1);
});
