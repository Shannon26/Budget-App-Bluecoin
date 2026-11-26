import { useState, type FormEvent } from "react";
import { api } from "./api";
import { formatMinor } from "./money";
import type { BudgetState, Category, TransactionKind } from "./types";

interface CategoryManagerProps {
  data: BudgetState;
  refresh: () => Promise<void>;
  onMessage: (message: string, error?: boolean) => void;
}

interface CategoryFormState {
  id: number | null;
  name: string;
  kind: TransactionKind;
  parent_id: string;
  color: string;
  icon: string;
  sort_order: string;
}

const emptyForm: CategoryFormState = {
  id: null,
  name: "",
  kind: "expense",
  parent_id: "",
  color: "#426b8a",
  icon: "•",
  sort_order: "0",
};

function orderedCategories(categories: Category[], kind: TransactionKind) {
  const matching = categories.filter((category) => category.kind === kind);
  const parents = matching
    .filter((category) => category.parent_id === null)
    .sort((left, right) => left.sort_order - right.sort_order || left.name.localeCompare(right.name));
  const children = matching
    .filter((category) => category.parent_id !== null)
    .sort((left, right) => left.sort_order - right.sort_order || left.name.localeCompare(right.name));
  return [
    ...parents,
    ...children.sort((left, right) => (left.parent_id ?? 0) - (right.parent_id ?? 0)),
  ];
}

export default function CategoryManager({ data, refresh, onMessage }: CategoryManagerProps) {
  const [form, setForm] = useState<CategoryFormState | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function startEdit(category: Category) {
    setForm({
      id: category.id,
      name: category.name,
      kind: category.kind,
      parent_id: category.parent_id ? String(category.parent_id) : "",
      color: category.color,
      icon: category.icon,
      sort_order: String(category.sort_order),
    });
    setError(null);
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!form) return;
    setSaving(true);
    setError(null);
    try {
      const input = {
        name: form.name,
        kind: form.kind,
        parent_id: form.parent_id ? Number(form.parent_id) : null,
        color: form.color,
        icon: form.icon,
        sort_order: Number(form.sort_order) || 0,
      };
      if (form.id) {
        await api.updateCategory(form.id, input);
      } else {
        await api.createCategory(input);
      }
      setForm(null);
      await refresh();
      onMessage(form.id ? "Category updated." : "Category created.");
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Could not save category");
    } finally {
      setSaving(false);
    }
  }

  async function archive(category: Category) {
    try {
      const result = await api.deleteCategory(category.id);
      await refresh();
      onMessage(
        result.archived
          ? `${category.name} archived. Historical entries and totals were preserved.`
          : `${category.name} deleted.`,
      );
    } catch (reason) {
      onMessage(reason instanceof Error ? reason.message : "Could not remove category", true);
    }
  }

  const totals = new Map(data.category_totals.map((total) => [total.category_id, total]));

  return (
    <section className="category-section">
      <div className="panel-heading">
        <div>
          <p className="section-kicker">Catalog</p>
          <h2>Categories</h2>
        </div>
        <button className="primary-button compact-button" data-testid="new-category" type="button" onClick={() => setForm(emptyForm)}>
          New category
        </button>
      </div>
      <p className="section-copy">
        Categories are optional. Existing and new unassigned income or expenses appear as
        Uncategorized. Archiving a referenced category keeps history intact and removes it from
        new-entry pickers.
      </p>

      <div className="category-columns">
        {(["expense", "income"] as TransactionKind[]).map((kind) => (
          <div className="category-group" key={kind}>
            <h3>{kind === "expense" ? "Expense categories" : "Income categories"}</h3>
            <div className="category-list">
              {orderedCategories(data.categories, kind).map((category) => {
                const total = totals.get(category.id);
                return (
                  <article
                    className={`category-row ${category.parent_id ? "category-child" : ""} ${category.archived_at ? "category-archived" : ""}`}
                    data-testid={`category-row-${category.id}`}
                    key={category.id}
                  >
                    <span className="category-icon" style={{ background: category.color }}>
                      {category.icon}
                    </span>
                    <div className="category-row-main">
                      <strong>{category.name}</strong>
                      <span>
                        {category.parent_name ? `${category.parent_name} / ` : ""}
                        {total ? `${formatMinor(total.total_minor)} · ${total.entry_count} entries` : "No entries yet"}
                      </span>
                    </div>
                    {category.archived_at ? (
                      <span className="archived-badge">Archived</span>
                    ) : (
                      <div className="category-row-actions">
                        <button className="text-button" type="button" onClick={() => startEdit(category)}>
                          Edit
                        </button>
                        <button
                          className="text-button danger-text"
                          data-testid={`archive-category-${category.id}`}
                          type="button"
                          onClick={() => void archive(category)}
                        >
                          {total ? "Archive" : "Delete"}
                        </button>
                      </div>
                    )}
                  </article>
                );
              })}
            </div>
          </div>
        ))}
      </div>

      <div className="uncategorized-summary">
        <h3>Uncategorized</h3>
        {data.category_totals
          .filter((total) => total.category_id === null)
          .map((total) => (
            <p key={total.kind}>
              {total.kind === "income" ? "Income" : "Expense"}: {formatMinor(total.total_minor)} · {total.entry_count} entries
            </p>
          ))}
        {!data.category_totals.some((total) => total.category_id === null) && <p>No uncategorized entries.</p>}
      </div>

      {form && (
        <div className="modal-backdrop" role="presentation">
          <section className="modal-card" role="dialog" aria-modal="true" aria-label="Category editor">
            <div className="panel-heading">
              <div>
                <p className="section-kicker">Catalog</p>
                <h2>{form.id ? "Edit category" : "New category"}</h2>
              </div>
              <button className="icon-button" type="button" onClick={() => setForm(null)} aria-label="Close category editor">
                ×
              </button>
            </div>
            {error && <div className="message message-error">{error}</div>}
            <form className="stack-form" onSubmit={submit}>
              <label>
                Name
                <input
                  data-testid="category-name"
                  value={form.name}
                  onChange={(event) => setForm({ ...form, name: event.target.value })}
                  required
                />
              </label>
              <div className="form-row">
                <label>
                  Type
                  <select
                    data-testid="category-kind"
                    value={form.kind}
                    onChange={(event) =>
                      setForm({ ...form, kind: event.target.value as TransactionKind, parent_id: "" })
                    }
                  >
                    <option value="expense">Expense</option>
                    <option value="income">Income</option>
                  </select>
                </label>
                <label>
                  Parent
                  <select
                    data-testid="category-parent"
                    value={form.parent_id}
                    onChange={(event) => setForm({ ...form, parent_id: event.target.value })}
                  >
                    <option value="">No parent</option>
                    {data.categories
                      .filter(
                        (category) =>
                          category.kind === form.kind &&
                          category.parent_id === null &&
                          category.archived_at === null &&
                          category.id !== form.id,
                      )
                      .map((category) => (
                        <option key={category.id} value={category.id}>
                          {category.name}
                        </option>
                      ))}
                  </select>
                </label>
              </div>
              <div className="form-row">
                <label>
                  Icon
                  <input
                    data-testid="category-icon"
                    value={form.icon}
                    onChange={(event) => setForm({ ...form, icon: event.target.value })}
                    maxLength={8}
                  />
                </label>
                <label>
                  Color
                  <input
                    data-testid="category-color"
                    type="color"
                    value={form.color}
                    onChange={(event) => setForm({ ...form, color: event.target.value })}
                  />
                </label>
                <label>
                  Sort order
                  <input
                    type="number"
                    value={form.sort_order}
                    onChange={(event) => setForm({ ...form, sort_order: event.target.value })}
                  />
                </label>
              </div>
              <button className="primary-button" data-testid="save-category" disabled={saving}>
                {saving ? "Saving…" : form.id ? "Save category" : "Create category"}
              </button>
            </form>
          </section>
        </div>
      )}
    </section>
  );
}
