import { useCallback, useEffect, useMemo, useState, type FormEvent } from "react";
import { api } from "./api";
import { formatMinor, localDateValue, parseMinorUnits } from "./money";
import type { CategoryBudget, MonthlyCategoryBudgets } from "./types";

const MONTH_NAMES = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];

function shiftMonth(month: string, amount: number): string {
  const year = Number(month.slice(0, 4));
  const monthNumber = Number(month.slice(5, 7));
  const absoluteMonth = year * 12 + monthNumber - 1 + amount;
  const nextYear = Math.floor(absoluteMonth / 12);
  const nextMonth = (absoluteMonth % 12) + 1;
  if (nextYear < 1 || nextYear > 9999) return month;
  return `${String(nextYear).padStart(4, "0")}-${String(nextMonth).padStart(2, "0")}`;
}

function monthLabel(month: string): string {
  const year = month.slice(0, 4);
  const monthIndex = Number(month.slice(5, 7)) - 1;
  return `${MONTH_NAMES[monthIndex] ?? month} ${year}`;
}

function progressPercent(category: CategoryBudget): number {
  if (!category.limit_minor) return 0;
  return Math.round((category.spent_minor / category.limit_minor) * 100);
}

export default function Budgets({ onMessage }: { onMessage: (message: string, error?: boolean) => void }) {
  const [month, setMonth] = useState(() => localDateValue().slice(0, 7));
  const [result, setResult] = useState<MonthlyCategoryBudgets | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [editor, setEditor] = useState<{ category: CategoryBudget; amount: string } | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setResult(await api.categoryBudgets(month));
    } catch (reason) {
      onMessage(reason instanceof Error ? reason.message : "Could not load budgets", true);
    } finally {
      setLoading(false);
    }
  }, [month, onMessage]);

  useEffect(() => {
    setEditor(null);
    void load();
  }, [load]);

  const parentIds = useMemo(
    () =>
      new Set(
        (result?.categories ?? [])
          .map((category) => category.parent_id)
          .filter((parentId): parentId is number => parentId !== null),
      ),
    [result],
  );

  function openEditor(category: CategoryBudget) {
    setEditor({
      category,
      amount: category.limit_minor === null ? "" : (category.limit_minor / 100).toFixed(2),
    });
  }

  async function saveLimit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!editor) return;
    const amountMinor = parseMinorUnits(editor.amount);
    if (amountMinor === null || amountMinor <= 0) {
      onMessage("Enter a valid monthly limit greater than zero", true);
      return;
    }
    setSaving(true);
    try {
      setResult(
        await api.setCategoryBudget(editor.category.category_id, month, amountMinor),
      );
      setEditor(null);
      onMessage("Monthly category budget saved.");
    } catch (reason) {
      onMessage(reason instanceof Error ? reason.message : "Could not save budget", true);
    } finally {
      setSaving(false);
    }
  }

  async function removeLimit(category: CategoryBudget) {
    setSaving(true);
    try {
      setResult(await api.removeCategoryBudget(category.category_id, month));
      setEditor(null);
      onMessage("Monthly category budget removed.");
    } catch (reason) {
      onMessage(reason instanceof Error ? reason.message : "Could not remove budget", true);
    } finally {
      setSaving(false);
    }
  }

  return (
    <section className="budgets-section">
      <div className="panel-heading ledger-heading">
        <div>
          <p className="section-kicker">Monthly planning</p>
          <h2>Budgets</h2>
        </div>
        <div className="budget-month-controls">
          <button
            className="secondary-button"
            type="button"
            aria-label="Previous month"
            onClick={() => setMonth((current) => shiftMonth(current, -1))}
          >
            ←
          </button>
          <label>
            <span className="sr-only">Budget month</span>
            <input
              data-testid="budget-month"
              type="month"
              value={month}
              onChange={(event) => setMonth(event.target.value)}
            />
          </label>
          <button
            className="secondary-button"
            type="button"
            aria-label="Next month"
            onClick={() => setMonth((current) => shiftMonth(current, 1))}
          >
            →
          </button>
        </div>
      </div>

      <div className="budget-summary-grid">
        <article className="budget-summary-card">
          <p>Monthly spending</p>
          <strong data-testid="budget-total-spent">
            {formatMinor(result?.total_spent_minor ?? 0)}
          </strong>
          <span>{result?.expense_count ?? 0} expenses · income and transfers excluded</span>
        </article>
        <article className="budget-summary-card">
          <p>Uncategorized spending</p>
          <strong data-testid="budget-uncategorized-spent">
            {formatMinor(result?.uncategorized_spent_minor ?? 0)}
          </strong>
          <span>Included in monthly spending, not in category rollups</span>
        </article>
      </div>

      {editor && (
        <form className="budget-editor" onSubmit={saveLimit}>
          <div>
            <p className="section-kicker">{monthLabel(month)}</p>
            <h3>{editor.category.limit_minor === null ? "Set" : "Edit"} monthly limit</h3>
            <p>
              {editor.category.parent_name
                ? `${editor.category.parent_name} / ${editor.category.name}`
                : editor.category.name}
            </p>
          </div>
          <label>
            Limit in INR
            <input
              data-testid="budget-limit-amount"
              value={editor.amount}
              onChange={(event) =>
                setEditor({ ...editor, amount: event.target.value })
              }
              inputMode="decimal"
              placeholder="5000.00"
              autoFocus
              required
            />
          </label>
          <button
            className="primary-button compact-button"
            data-testid="save-budget-limit"
            type="submit"
            disabled={saving}
          >
            {saving ? "Saving…" : "Save limit"}
          </button>
          <button
            className="secondary-button"
            data-testid="cancel-budget-limit"
            type="button"
            disabled={saving}
            onClick={() => setEditor(null)}
          >
            Cancel
          </button>
        </form>
      )}

      {loading && !result ? (
        <div className="empty-state">Loading budgets…</div>
      ) : !result || result.categories.length === 0 ? (
        <div className="empty-state">No expense categories yet.</div>
      ) : (
        <div className="budget-list">
          {result.categories.map((category) => {
            const hasLimit = category.limit_minor !== null;
            const over = hasLimit && category.spent_minor > category.limit_minor!;
            const percent = progressPercent(category);
            const remaining = category.remaining_minor ?? 0;
            return (
              <article
                className={`budget-row ${category.parent_id !== null ? "budget-child" : ""} ${category.archived_at ? "budget-archived" : ""}`}
                data-testid={`budget-row-${category.category_id}`}
                key={category.category_id}
              >
                <div className="budget-category">
                  <span className="budget-category-icon" style={{ background: category.color }}>
                    {category.icon}
                  </span>
                  <div>
                    <strong>{category.name}</strong>
                    <span>
                      {category.parent_name ? `${category.parent_name} / ` : ""}
                      {category.archived_at ? "Archived" : parentIds.has(category.category_id) ? "Includes subcategories" : "Expense category"}
                    </span>
                  </div>
                </div>
                <div className="budget-numbers">
                  <div>
                    <span>Limit</span>
                    <strong data-testid={`budget-limit-${category.category_id}`}>
                      {hasLimit ? formatMinor(category.limit_minor!) : "No limit"}
                    </strong>
                  </div>
                  <div>
                    <span>Spent</span>
                    <strong data-testid={`budget-spent-${category.category_id}`}>
                      {formatMinor(category.spent_minor)}
                    </strong>
                  </div>
                  <div>
                    <span>{over ? "Over" : "Remaining"}</span>
                    <strong
                      className={over ? "negative-text" : "positive-text"}
                      data-testid={`budget-remaining-${category.category_id}`}
                    >
                      {hasLimit
                        ? over
                          ? `${formatMinor(-remaining)} over`
                          : `${formatMinor(remaining)} left`
                        : "—"}
                    </strong>
                  </div>
                </div>
                {hasLimit && (
                  <div className="budget-progress-block">
                    <div
                      className={`budget-progress ${over ? "over" : ""}`}
                      role="progressbar"
                      aria-label={`${category.name} budget used`}
                      aria-valuemin={0}
                      aria-valuemax={100}
                      aria-valuenow={Math.min(percent, 100)}
                      aria-valuetext={`${percent}% of limit used`}
                    >
                      <span style={{ width: `${Math.min(percent, 100)}%` }} />
                    </div>
                    <small>{percent}% used</small>
                  </div>
                )}
                <div className="budget-row-actions">
                  <button
                    className="secondary-button"
                    data-testid={`edit-budget-${category.category_id}`}
                    type="button"
                    disabled={saving || (!hasLimit && category.archived_at !== null)}
                    onClick={() => openEditor(category)}
                  >
                    {hasLimit ? "Edit limit" : "Set limit"}
                  </button>
                  {hasLimit && (
                    <button
                      className="text-button danger-text"
                      data-testid={`remove-budget-${category.category_id}`}
                      type="button"
                      disabled={saving}
                      onClick={() => void removeLimit(category)}
                    >
                      Remove
                    </button>
                  )}
                </div>
              </article>
            );
          })}
        </div>
      )}
    </section>
  );
}
