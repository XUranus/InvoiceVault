import React, { Suspense } from "react";
import ReactDOM from "react-dom/client";
import { HashRouter, Routes, Route, Navigate } from "react-router-dom";
import { Layout } from "./components/Layout";
import "./styles.css";

const DashboardPage = React.lazy(
  () => import("./components/DashboardPage"),
);
const ImportPage = React.lazy(() => import("./components/ImportPage"));
const InvoicesPage = React.lazy(
  () => import("./components/InvoicesPage"),
);
const AgentPage = React.lazy(() => import("./components/AgentPage"));
const EventsPage = React.lazy(() => import("./components/EventsPage"));
const NotificationsPage = React.lazy(
  () => import("./components/NotificationsPage"),
);
const SettingsPage = React.lazy(
  () => import("./components/SettingsPage"),
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
            path="/notifications"
            element={
              <Suspense fallback={<PageFallback />}>
                <NotificationsPage />
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
          />
        </Route>
      </Routes>
    </HashRouter>
  </React.StrictMode>,
);
