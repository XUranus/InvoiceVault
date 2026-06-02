import { useEffect } from "react";
import { Outlet, useLocation, useNavigate } from "react-router-dom";
import { TitleBar } from "./TitleBar";
import { Sidebar } from "./Sidebar";
import { ErrorBoundary } from "./ErrorBoundary";
import { OnboardingDialog } from "./OnboardingDialog";
import { useAppInitializer } from "../hooks/useAppInitializer";
import { useAppStore } from "../stores/appStore";

const NAV_SHORTCUTS: Record<string, string> = {
  "1": "/dashboard",
  "2": "/import",
  "3": "/invoices",
  "4": "/agent",
  "5": "/events",
};

export function Layout() {
  useAppInitializer();
  const location = useLocation();
  const navigate = useNavigate();
  const setError = useAppStore((s) => s.setError);
  const showOnboarding = useAppStore((s) => s.showOnboarding);

  useEffect(() => {
    useAppStore.getState().clearError();
  }, [location.pathname]);

  // Global keyboard shortcuts
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // Don't trigger shortcuts when typing in inputs
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;

      // Ctrl+1-5: navigate to sidebar items
      if (e.ctrlKey && NAV_SHORTCUTS[e.key]) {
        e.preventDefault();
        navigate(NAV_SHORTCUTS[e.key]);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [navigate]);

  return (
    <div className="app-layout">
      <TitleBar />
      <div className="app-body">
        <Sidebar />
        <main className="app-main">
          <ErrorBoundary onError={setError}>
            <Outlet />
          </ErrorBoundary>
        </main>
      </div>
      {showOnboarding ? <OnboardingDialog /> : null}
    </div>
  );
}
