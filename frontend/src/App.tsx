import { useCallback, useEffect, useState } from "react";
import { api } from "./shared/api";
import type { BudgetState } from "./shared/types";
import Budgets from "./features/budgets/Budgets";
import CategoryManager from "./features/categories/CategoryManager";
import Ledger from "./features/ledger/Ledger";
import Overview from "./features/overview/Overview";
import { useOverviewForms } from "./features/overview/useOverviewForms";

const emptyState: BudgetState = {
  accounts: [],
  account_types: [],
  totals: { income_minor: 0, expense_minor: 0 },
  categories: [],
  category_totals: [],
  saved_filters: [],
  entries: [],
};

type Tab = "overview" | "ledger" | "categories" | "budgets";

export default function App() {
  const [data, setData] = useState<BudgetState>(emptyState);
  const [activeTab, setActiveTab] = useState<Tab>("overview");
  const [historyAccountId, setHistoryAccountId] = useState<number | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const onMessage = useCallback((message: string, isError = false) => {
    if (isError) {
      setError(message);
      setNotice(null);
    } else {
      setNotice(message);
      setError(null);
    }
  }, []);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      setData(await api.state());
      setError(null);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Could not load your ledger");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const overviewForms = useOverviewForms(data, refresh, onMessage);

  return (
    <div className="app-shell">
      <header className="topbar">
        <a className="brand" href="/" aria-label="Paisa home">
          <span className="brand-mark">₹</span>
          <span>
            <strong>Paisa</strong>
            <small>a calmer money ledger</small>
          </span>
        </a>
        <div className="local-badge">
          <span className="status-dot" />
          Local-first · SQLite
        </div>
      </header>

      <nav className="main-nav" aria-label="Main navigation">
        <button
          data-testid="nav-overview"
          className={activeTab === "overview" ? "active" : ""}
          type="button"
          onClick={() => {
            setActiveTab("overview");
            setHistoryAccountId(null);
          }}
        >
          Overview
        </button>
        <button
          data-testid="nav-ledger"
          className={activeTab === "ledger" ? "active" : ""}
          type="button"
          onClick={() => setActiveTab("ledger")}
        >
          Ledger
        </button>
        <button
          data-testid="nav-categories"
          className={activeTab === "categories" ? "active" : ""}
          type="button"
          onClick={() => setActiveTab("categories")}
        >
          Categories
        </button>
        <button
          data-testid="nav-budgets"
          className={activeTab === "budgets" ? "active" : ""}
          type="button"
          onClick={() => setActiveTab("budgets")}
        >
          Budgets
        </button>
      </nav>

      <main>
        {(error || notice) && (
          <div
            className={`message ${error ? "message-error" : "message-success"}`}
            role={error ? "alert" : "status"}
          >
            <span>{error ? "!" : "✓"}</span>
            {error ?? notice}
          </div>
        )}

        {activeTab === "overview" && (
          <Overview
            data={data}
            loading={loading}
            refresh={refresh}
            forms={overviewForms}
            onMessage={onMessage}
            onAccountHistory={(accountId) => {
              setHistoryAccountId(accountId);
              setActiveTab("ledger");
            }}
            onOpenLedger={() => setActiveTab("ledger")}
            onOpenCategories={() => setActiveTab("categories")}
          />
        )}

        {activeTab === "ledger" && (
          <section className="panel ledger-panel">
            <Ledger
              data={data}
              refresh={refresh}
              onMessage={onMessage}
              historyAccountId={historyAccountId}
              onClearHistory={() => setHistoryAccountId(null)}
            />
          </section>
        )}

        {activeTab === "categories" && (
          <section className="panel ledger-panel">
            <CategoryManager data={data} refresh={refresh} onMessage={onMessage} />
          </section>
        )}

        {activeTab === "budgets" && (
          <section className="panel ledger-panel">
            <Budgets onMessage={onMessage} />
          </section>
        )}
      </main>

      <footer>
        <span>Paisa · transactions, categories, and monthly budgets</span>
        <span>Amounts are stored as integer minor units (paise).</span>
      </footer>
    </div>
  );
}
