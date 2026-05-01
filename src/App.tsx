import React from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { ImportJob, Invoice } from "./types";
import { getAppHealth, listImportJobs, searchInvoices, importFiles } from "./api";
import { Sidebar } from "./components/Sidebar";
import { DashboardPage } from "./components/DashboardPage";
import { ImportPage } from "./components/ImportPage";
import { InvoicesPage } from "./components/InvoicesPage";
import { SettingsPage } from "./components/SettingsPage";

type Page = "dashboard" | "import" | "invoices" | "settings";

export default function App() {
  const [page, setPage] = React.useState<Page>("dashboard");
  const [health, setHealth] = React.useState<import("./types").AppHealth | null>(null);
  const [jobs, setJobs] = React.useState<ImportJob[]>([]);
  const [invoices, setInvoices] = React.useState<Invoice[]>([]);
  const [llmBaseUrl, setLlmBaseUrl] = React.useState(
    "https://dashscope.aliyuncs.com/compatible-mode/v1",
  );
  const [llmModel, setLlmModel] = React.useState("qwen3.6-plus");
  const [llmApiKey, setLlmApiKey] = React.useState(
    "sk-0bfe76db71b74da59ef1fa085586e6ba",
  );
  const [isDraggingFiles, setIsDraggingFiles] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  const clearError = () => setError(null);

  const refreshJobs = React.useCallback(() => {
    listImportJobs()
      .then(setJobs)
      .catch((err) => setError(String(err)));
  }, []);

  const refreshInvoices = React.useCallback(() => {
    searchInvoices({ page: 1, page_size: 100 })
      .then((result) => setInvoices(result.invoices))
      .catch((err) => setError(String(err)));
  }, []);

  React.useEffect(() => {
    getAppHealth()
      .then(setHealth)
      .catch((err) => setError(String(err)));
    refreshJobs();
    refreshInvoices();
  }, [refreshJobs, refreshInvoices]);

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
          .then((imported) => {
            setJobs((current) => [...imported, ...current]);
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

  const handleJobsChange = (imported: ImportJob[]) => {
    setJobs((current) => [...imported, ...current]);
  };

  return (
    <div className="app-layout">
      <Sidebar
        activePage={page}
        onNavigate={(p) => {
          setPage(p);
          clearError();
          if (p === "invoices") refreshInvoices();
          if (p === "import") refreshJobs();
        }}
        healthReady={health !== null}
        hasError={error !== null}
      />

      <main className="app-main">
        {page === "dashboard" ? (
          <DashboardPage
            health={health}
            error={error}
            invoiceCount={invoices.length}
            jobCount={jobs.length}
          />
        ) : page === "import" ? (
          <ImportPage
            jobs={jobs}
            isDraggingFiles={isDraggingFiles}
            llmApiKey={llmApiKey}
            llmBaseUrl={llmBaseUrl}
            llmModel={llmModel}
            onJobsChange={handleJobsChange}
            onInvoicesAdded={refreshInvoices}
            onError={setError}
          />
        ) : page === "invoices" ? (
          <InvoicesPage
            invoices={invoices}
            onInvoicesChanged={refreshInvoices}
            onError={setError}
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
          />
        )}
      </main>
    </div>
  );
}
