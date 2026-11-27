import { describe, expect, it } from "vitest";
import type { AccountTypeDefinition } from "../../shared/types";
import {
  accountGroupLabel,
  accountTypeDefinition,
  isAccountSelectable,
} from "./accountClassification";

const definitions: AccountTypeDefinition[] = [
  { value: "unclassified", label: "Unclassified", group: "unclassified" },
  { value: "cash", label: "Cash", group: "asset" },
  { value: "credit_card", label: "Credit Card", group: "liability" },
];

describe("account classification presentation", () => {
  it("uses the server-provided type definition", () => {
    expect(accountTypeDefinition(definitions, "credit_card")).toEqual({
      value: "credit_card",
      label: "Credit Card",
      group: "liability",
    });
  });

  it("falls back to Unclassified for legacy or unknown values", () => {
    expect(accountTypeDefinition(definitions, undefined)).toEqual({
      value: "unclassified",
      label: "Unclassified",
      group: "unclassified",
    });
    expect(accountTypeDefinition(definitions, "legacy")).toEqual({
      value: "unclassified",
      label: "Unclassified",
      group: "unclassified",
    });
  });

  it("labels logical groups without reclassifying account types", () => {
    expect(accountGroupLabel("asset")).toBe("Asset");
    expect(accountGroupLabel("liability")).toBe("Liability");
    expect(accountGroupLabel("unclassified")).toBe("Unclassified");
  });

  it("treats report hiding as selection plus future-report hiding", () => {
    expect(
      isAccountSelectable({ hide_from_selection: false, hide_from_reports: false }),
    ).toBe(true);
    expect(
      isAccountSelectable({ hide_from_selection: true, hide_from_reports: false }),
    ).toBe(false);
    expect(
      isAccountSelectable({ hide_from_selection: false, hide_from_reports: true }),
    ).toBe(false);
  });
});
