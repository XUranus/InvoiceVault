import { useEffect } from "react";
import { Outlet, useLocation } from "react-router-dom";
import { Sidebar } from "./Sidebar";
import { ErrorBoundary } from "./ErrorBoundary";
import { useAppInitializer } from "../hooks/useAppInitializer";
import { useAppStore } from "../stores/appStore";

export function Layout() {
  useAppInitializer();
  const location = useLocation();
  const setError = useAppStore((s) => s.setError);

  useEffect(() => {
    useAppStore.getState().clearError();
  }, [location.pathname]);

  return (
    <div className="app-layout">
      <Sidebar />
      <main className="app-main">
        <ErrorBoundary onError={setError}>
          <Outlet />
        </ErrorBoundary>
      </main>
    </div>
  );
}
