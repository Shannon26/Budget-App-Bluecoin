import type { Label } from "../../shared/types";

export default function LabelChips({ labels }: { labels: Label[] }) {
  if (labels.length === 0) return null;
  return (
    <span className="entry-labels">
      {labels.map((label) => (
        <span key={label.id}>{label.name}</span>
      ))}
    </span>
  );
}
