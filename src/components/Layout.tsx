import { useEffect } from "react";
import { Outlet, useLocation } from "react-router-dom";
import { TitleBar } from "./TitleBar";
import { Sidebar } from "./Sidebar";
import { ErrorBoundary } from "./ErrorBoundary";
import { OnboardingDialog } from "./OnboardingDialog";
import { useAppInitializer } from "../hooks/useAppInitializer";
import { useAppStore } from "../stores/appStore";

export function Layout() {
  useAppInitializer();
  const location = useLocation();
  const setError = useAppStore((s) => s.setError);
  const showOnboarding = useAppStore((s) => s.showOnboarding);

  useEffect(() => {
    useAppStore.getState().clearError();
  }, [location.pathname]);

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
