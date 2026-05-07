import React from "react";
import { Outlet, useLocation, useNavigate, Navigate } from "react-router-dom";
import { useAppStore } from "../stores/appStore";

const TABS = [
  { path: "/settings/ai", label: "AI Provider" },
  { path: "/settings/general", label: "通用" },
  { path: "/settings/advanced", label: "高级" },
];

export function SettingsPage() {
  const navigate = useNavigate();
  const location = useLocation();
  const error = useAppStore((s) => s.error);

  if (location.pathname === "/settings") {
    return <Navigate to="/settings/ai" replace />;
  }

  return (
    <div className="page settings-page">
      <h2 className="page-title">设置</h2>
      {error ? <div className="alert alert-error">{error}</div> : null}
      <div className="settings-tabs">
        {TABS.map((tab) => (
          <button
            key={tab.path}
            className={`settings-tab ${location.pathname === tab.path ? "settings-tab-active" : ""}`}
            onClick={() => navigate(tab.path)}
          >
            {tab.label}
          </button>
        ))}
      </div>
      <Outlet />
    </div>
  );
}

export default SettingsPage;
