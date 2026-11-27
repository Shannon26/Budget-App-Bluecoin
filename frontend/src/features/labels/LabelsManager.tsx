import { useState, type FormEvent } from "react";
import { api } from "../../shared/api";
import Modal from "../../shared/Modal";
import type { BudgetState } from "../../shared/types";

export default function LabelsManager({
  data,
  refresh,
  onMessage,
}: {
  data: BudgetState;
  refresh: () => Promise<void>;
  onMessage: (message: string, error?: boolean) => void;
}) {
  const [form, setForm] = useState<{ id: number | null; name: string } | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!form) return;
    setSaving(true);
    setError(null);
    try {
      if (form.id === null) await api.createLabel(form.name);
      else await api.updateLabel(form.id, form.name);
      setForm(null);
      await refresh();
      onMessage(form.id === null ? "Label created." : "Label renamed.");
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Could not save label");
    } finally {
      setSaving(false);
    }
  }

  return (
    <section className="labels-section">
      <div className="panel-heading">
        <div>
          <p className="section-kicker">Cross-category organization</p>
          <h2>Labels</h2>
        </div>
        <button
          className="primary-button compact-button"
          data-testid="new-label"
          type="button"
          onClick={() => {
            setError(null);
            setForm({ id: null, name: "" });
          }}
        >
          New label
        </button>
      </div>
      <p className="section-copy">
        Labels are reusable names you can attach to transactions and transfers. They do not change
        amounts, categories, CSV columns, or report totals.
      </p>
      {data.labels.length === 0 ? (
        <div className="empty-state">No labels yet. Create one to start organizing entries.</div>
      ) : (
        <div className="label-list">
          {data.labels.map((label) => (
            <article className="label-row" data-testid={`label-row-${label.id}`} key={label.id}>
              <strong>{label.name}</strong>
              <div className="label-row-actions">
                <button
                  className="text-button"
                  data-testid={`rename-label-${label.id}`}
                  type="button"
                  onClick={() => {
                    setError(null);
                    setForm({ id: label.id, name: label.name });
                  }}
                >
                  Rename
                </button>
              </div>
            </article>
          ))}
        </div>
      )}
      {form && (
        <Modal
          label="Label editor"
          dismissible={!saving}
          onClose={() => {
            if (!saving) setForm(null);
          }}
        >
          <div className="panel-heading">
            <div>
              <p className="section-kicker">Reusable label</p>
              <h2>{form.id === null ? "New label" : "Rename label"}</h2>
            </div>
            <button
              className="icon-button"
              type="button"
              disabled={saving}
              aria-label="Close label editor"
              onClick={() => setForm(null)}
            >
              ×
            </button>
          </div>
          {error && <div className="message message-error" role="alert">{error}</div>}
          <form className="stack-form" onSubmit={submit}>
            <label>
              Name
              <input
                data-testid="label-name"
                data-autofocus
                value={form.name}
                onChange={(event) => setForm({ ...form, name: event.target.value })}
                maxLength={40}
                required
              />
            </label>
            <button className="primary-button" data-testid="save-label" disabled={saving}>
              {saving ? "Saving…" : form.id === null ? "Create label" : "Save label"}
            </button>
          </form>
        </Modal>
      )}
    </section>
  );
}
