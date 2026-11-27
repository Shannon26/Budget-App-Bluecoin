import type { Label } from "../../shared/types";

export default function LabelMultiSelect({
  labels,
  selected,
  onChange,
  testIdPrefix,
  disabled = false,
}: {
  labels: Label[];
  selected: number[];
  onChange: (ids: number[]) => void;
  testIdPrefix: string;
  disabled?: boolean;
}) {
  return (
    <fieldset className="label-picker" disabled={disabled}>
      <legend>Labels</legend>
      {labels.length === 0 ? (
        <p className="field-hint">No labels yet. Create reusable labels in the Labels screen.</p>
      ) : (
        <div className="label-picker-options">
          {labels.map((label) => (
            <label className="label-option" key={label.id}>
              <input
                data-testid={`${testIdPrefix}-${label.id}`}
                type="checkbox"
                checked={selected.includes(label.id)}
                onChange={(event) =>
                  onChange(
                    event.target.checked
                      ? [...selected, label.id]
                      : selected.filter((id) => id !== label.id),
                  )
                }
              />
              <span>{label.name}</span>
            </label>
          ))}
        </div>
      )}
    </fieldset>
  );
}
