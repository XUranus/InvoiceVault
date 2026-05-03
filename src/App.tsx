import React from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import type { Invoice, WatcherImportEvent } from "./types";
import { getAppHealth, searchInvoices, importFiles, getLlmConfig, getRecognitionQueueStatus } from "./api";
import { Sidebar } from "./components/Sidebar";
import { DashboardPage } from "./components/DashboardPage";
import { ImportPage } from "./components/ImportPage";
import { InvoicesPage } from "./components/InvoicesPage";
import { SettingsPage } from "./components/SettingsPage";
import { AgentPage } from "./components/AgentPage";
import { EventsPage } from "./components/EventsPage";
import { NotificationsPage } from "./components/NotificationsPage";
import { ErrorBoundary } from "./components/ErrorBoundary";

type Page = "dashboard" | "import" | "invoices" | "agent" | "events" | "notifications" | "settings";

export default function App() {
  const [page, setPage] = React.useState<Page>("dashboard");
  const [health, setHealth] = React.useState<import("./types").AppHealth | null>(null);
  const [invoices, setInvoices] = React.useState<Invoice[]>([]);
  const [llmBaseUrl, setLlmBaseUrl] = React.useState(
    "https://dashscope.aliyuncs.com/compatible-mode/v1",
  );
  const [llmModel, setLlmModel] = React.useState("qwen3.6-plus");
  const [llmApiKey, setLlmApiKey] = React.useState("");
  const [isDraggingFiles, setIsDraggingFiles] = React.useState(false);
  const [dashboardKey, setDashboardKey] = React.useState(0);
  const [importKey, setImportKey] = React.useState(0);
  const [error, setError] = React.useState<string | null>(null);
  const [theme, setTheme] = React.useState<"light" | "dark">(() => {
    const stored = localStorage.getItem("theme");
    return stored === "dark" ? "dark" : "light";
  });
  const [unreadNotificationCount, setUnreadNotificationCount] = React.useState(0);
  const [invoicesKey, setInvoicesKey] = React.useState(0);
  const [importBadgeCount, setImportBadgeCount] = React.useState(0);
  const [invoiceBadgeCount, setInvoiceBadgeCount] = React.useState(0);

  const navigateToInvoice = (id: number) => {
    sessionStorage.setItem("focusInvoiceId", String(id));
    setPage("invoices");
    setInvoicesKey((k) => k + 1);
  };

  const toggleTheme = () => {
    setTheme((prev) => {
      const next = prev === "dark" ? "light" : "dark";
      localStorage.setItem("theme", next);
      return next;
    });
  };

  React.useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
  }, [theme]);

  const clearError = () => setError(null);

  const refreshInvoices = React.useCallback(() => {
    searchInvoices({ page: 1, page_size: 100 })
      .then((result) => setInvoices(result.invoices))
      .catch((err) => setError(String(err)));
  }, []);

  React.useEffect(() => {
    getAppHealth()
      .then(setHealth)
      .catch((err) => setError(String(err)));
    refreshInvoices();
    // Restore saved LLM config
    getLlmConfig()
      .then((cfg) => {
        if (cfg) {
          setLlmBaseUrl(cfg.base_url);
          setLlmModel(cfg.model);
          setLlmApiKey(cfg.api_key);
        }
      })
      .catch(() => {});
  }, [refreshInvoices]);

  // Global drag-drop handler — navigate to import page on drop
  React.useEffect(() => {
    let unlisten: (() => void) | null = null;

    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "enter" || event.payload.type === "over") {
          setIsDraggingFiles(true);
          return;
        }

        if (event.payload.type === "leave") {
          setIsDraggingFiles(false);
          return;
        }

        setIsDraggingFiles(false);
        const paths: string[] = (event.payload as { paths: string[] }).paths;
        importFiles(paths)
          .then(() => {
            setImportKey((k) => k + 1);
            setPage("import");
          })
          .catch((err) => setError(String(err)));
      })
      .then((handler) => {
        unlisten = handler;
      })
      .catch((err) => setError(String(err)));

    return () => {
      unlisten?.();
    };
  }, []);

  // Watcher auto-import notifications
  React.useEffect(() => {
    const unlisten = listen<WatcherImportEvent>("watcher-import", (event) => {
      setImportKey((k) => k + 1);
      refreshInvoices();
      setDashboardKey((k) => k + 1);
      // Increment invoice badge by number of imported files
      const count = event.payload.imported_count ?? event.payload.jobs?.length ?? 0;
      setInvoiceBadgeCount((c) => c + count);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [refreshInvoices]);

  // Poll recognition queue status for import tab badge
  React.useEffect(() => {
    const poll = () => {
      getRecognitionQueueStatus()
        .then((status) => {
          const total = status.pending + status.running;
          setImportBadgeCount(total);
        })
        .catch(() => {});
    };
    poll();
    const interval = setInterval(poll, 2000);
    return () => clearInterval(interval);
  }, []);

  const handleInvoiceDetailOpened = () => {
    setInvoiceBadgeCount((c) => Math.max(0, c - 1));
  };

  return (
    <div className="app-layout">
      <Sidebar
        activePage={page}
        onNavigate={(p) => {
          setPage(p);
          clearError();
          if (p === "dashboard") setDashboardKey((k) => k + 1);
          if (p === "invoices") { refreshInvoices(); setInvoicesKey((k) => k + 1); }
          if (p === "import") setImportKey((k) => k + 1);
        }}
        healthReady={health !== null}
        hasError={error !== null}
        unreadNotificationCount={unreadNotificationCount}
        importBadgeCount={importBadgeCount}
        invoiceBadgeCount={invoiceBadgeCount}
      />

      <main className="app-main">
        <ErrorBoundary onError={setError}>
        {page === "dashboard" ? (
          <DashboardPage
            error={error}
            refreshKey={dashboardKey}
          />
        ) : page === "import" ? (
          <ImportPage
            isDraggingFiles={isDraggingFiles}
            llmApiKey={llmApiKey}
            llmBaseUrl={llmBaseUrl}
            llmModel={llmModel}
            refreshKey={importKey}
            onInvoicesAdded={refreshInvoices}
            onNavigateToInvoice={navigateToInvoice}
            onError={setError}
          />
        ) : page === "invoices" ? (
          <InvoicesPage
            invoices={invoices}
            onInvoicesChanged={refreshInvoices}
            onError={setError}
            refreshKey={invoicesKey}
            onInvoiceDetailOpened={handleInvoiceDetailOpened}
          />
        ) : page === "agent" ? (
          <AgentPage
            llmBaseUrl={llmBaseUrl}
            llmModel={llmModel}
            llmApiKey={llmApiKey}
            onError={setError}
          />
        ) : page === "events" ? (
          <EventsPage onNavigateToInvoice={navigateToInvoice} />
        ) : page === "notifications" ? (
          <NotificationsPage
            onNavigateToInvoice={navigateToInvoice}
            onUnreadCountChange={setUnreadNotificationCount}
          />
        ) : (
          <SettingsPage
            health={health}
            error={error}
            llmBaseUrl={llmBaseUrl}
            llmModel={llmModel}
            llmApiKey={llmApiKey}
            onBaseUrlChange={setLlmBaseUrl}
            onModelChange={setLlmModel}
            onApiKeyChange={setLlmApiKey}
            theme={theme}
            onToggleTheme={toggleTheme}
          />
        )}
        </ErrorBoundary>
      </main>
    </div>
  );
}
