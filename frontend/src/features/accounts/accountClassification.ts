import type { AccountGroup, AccountMetadata, AccountTypeDefinition } from "../../shared/types";

export const ACCOUNT_GROUP_ORDER: AccountGroup[] = ["asset", "liability", "unclassified"];

export function accountGroupLabel(group: AccountGroup): string {
  if (group === "asset") return "Asset";
  if (group === "liability") return "Liability";
  return "Unclassified";
}

export function accountTypeDefinition(
  definitions: AccountTypeDefinition[],
  value: string | undefined,
): AccountTypeDefinition {
  return (
    definitions.find((definition) => definition.value === value) ?? {
      value: "unclassified",
      label: "Unclassified",
      group: "unclassified",
    }
  );
}

export function isAccountSelectable(
  account: Pick<AccountMetadata, "hide_from_selection" | "hide_from_reports">,
): boolean {
  return !account.hide_from_selection && !account.hide_from_reports;
}
