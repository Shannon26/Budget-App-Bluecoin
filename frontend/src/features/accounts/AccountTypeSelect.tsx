import type { AccountTypeDefinition } from "../../shared/types";
import {
  ACCOUNT_GROUP_ORDER,
  accountGroupLabel,
  accountTypeDefinition,
} from "./accountClassification";

interface AccountTypeSelectProps {
  definitions: AccountTypeDefinition[];
  value: string;
  onChange: (value: string) => void;
  testId: string;
  disabled?: boolean;
}

export default function AccountTypeSelect({
  definitions,
  value,
  onChange,
  testId,
  disabled = false,
}: AccountTypeSelectProps) {
  const selected = accountTypeDefinition(definitions, value);

  return (
    <label>
      Account type
      <select
        data-testid={testId}
        value={selected.value}
        onChange={(event) => onChange(event.target.value)}
        disabled={disabled || definitions.length === 0}
        required
      >
        {definitions.length === 0 && <option value="unclassified">Unclassified</option>}
        {ACCOUNT_GROUP_ORDER.map((group) => {
          const options = definitions.filter((definition) => definition.group === group);
          if (options.length === 0) return null;
          return (
            <optgroup key={group} label={accountGroupLabel(group)}>
              {options.map((definition) => (
                <option key={definition.value} value={definition.value}>
                  {definition.label}
                </option>
              ))}
            </optgroup>
          );
        })}
      </select>
      <small className="field-hint">
        Logical group: <strong data-testid={`${testId}-group`}>{accountGroupLabel(selected.group)}</strong>
      </small>
    </label>
  );
}
