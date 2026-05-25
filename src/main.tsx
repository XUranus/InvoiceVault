import React, { Suspense } from "react";
import ReactDOM from "react-dom/client";
import { HashRouter, Routes, Route, Navigate } from "react-router-dom";
import { Layout } from "./components/Layout";
import "./styles.css";

const DashboardPage = React.lazy(
  () => import("./components/DashboardPage"),
);
const ImportPage = React.lazy(() => import("./components/ImportPage"));
const DataSourcePage = React.lazy(
  () => import("./components/DataSourcePage"),
);
const InvoicesPage = React.lazy(
  () => import("./components/InvoicesPage"),
);
const AgentPage = React.lazy(() => import("./components/agent/AgentPage"));
const EventsPage = React.lazy(() => import("./components/EventsPage"));
const SettingsPage = React.lazy(
  () => import("./components/SettingsPage"),
);
const AiProviderPage = React.lazy(
  () => import("./components/settings/AiProviderPage"),
);
const GeneralPage = React.lazy(
  () => import("./components/settings/GeneralPage"),
);
const AdvancedPage = React.lazy(
  () => import("./components/settings/AdvancedPage"),
);

function PageFallback() {
  return (
    <div className="page">
      <p className="muted">加载中...</p>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <HashRouter>
      <Routes>
        <Route element={<Layout />}>
          <Route
            path="/"
            element={<Navigate to="/dashboard" replace />}
          />
          <Route
            path="/dashboard"
            element={
              <Suspense fallback={<PageFallback />}>
                <DashboardPage />
              </Suspense>
            }
          />
          <Route
            path="/import"
            element={
              <Suspense fallback={<PageFallback />}>
                <ImportPage />
              </Suspense>
            }
          />
          <Route
            path="/import/sources"
            element={
              <Suspense fallback={<PageFallback />}>
                <DataSourcePage />
              </Suspense>
            }
          />
          <Route
            path="/invoices"
            element={
              <Suspense fallback={<PageFallback />}>
                <InvoicesPage />
              </Suspense>
            }
          />
          <Route
            path="/agent"
            element={
              <Suspense fallback={<PageFallback />}>
                <AgentPage />
              </Suspense>
            }
          />
          <Route
            path="/events"
            element={
              <Suspense fallback={<PageFallback />}>
                <EventsPage />
              </Suspense>
            }
          />
          <Route
            path="/settings"
            element={
              <Suspense fallback={<PageFallback />}>
                <SettingsPage />
              </Suspense>
            }
          >
            <Route index element={<Navigate to="ai" replace />} />
            <Route
              path="ai"
              element={
                <Suspense fallback={<PageFallback />}>
                  <AiProviderPage />
                </Suspense>
              }
            />
            <Route
              path="general"
              element={
                <Suspense fallback={<PageFallback />}>
                  <GeneralPage />
                </Suspense>
              }
            />
            <Route
              path="advanced"
              element={
                <Suspense fallback={<PageFallback />}>
                  <AdvancedPage />
                </Suspense>
              }
            />
          </Route>
        </Route>
      </Routes>
    </HashRouter>
  </React.StrictMode>,
);
