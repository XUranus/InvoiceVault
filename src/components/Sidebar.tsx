import { useLocation, useNavigate } from "react-router-dom";
import {
  Activity,
  Bell,
  Bot,
  LayoutDashboard,
  ReceiptText,
  Settings,
  Upload,
  type LucideIcon,
} from "lucide-react";
import { useAppStore } from "../stores/appStore";

const appIcon = new URL("../../icons/icon.png", import.meta.url).href;

const NAV_ITEMS: { path: string; label: string; icon: LucideIcon }[] = [
  { path: "/dashboard", label: "仪表盘", icon: LayoutDashboard },
  { path: "/import", label: "导入", icon: Upload },
  { path: "/invoices", label: "发票库", icon: ReceiptText },
  { path: "/agent", label: "Agent", icon: Bot },
  { path: "/events", label: "事件", icon: Activity },
  { path: "/notifications", label: "通知", icon: Bell },
  { path: "/settings", label: "设置", icon: Settings },
];

export function Sidebar() {
  const location = useLocation();
  const navigate = useNavigate();

  const health = useAppStore((s) => s.health);
  const error = useAppStore((s) => s.error);
  const unreadNotificationCount = useAppStore(
    (s) => s.unreadNotificationCount,
  );
  const importBadgeCount = useAppStore((s) => s.importBadgeCount);
  const invoiceBadgeCount = useAppStore((s) => s.invoiceBadgeCount);

  const healthReady = health !== null;
  const hasError = error !== null;

  return (
    <aside className="sidebar">
      <div className="sidebar-brand">
        <img
          className="sidebar-app-icon"
          src={appIcon}
          alt=""
          aria-hidden="true"
        />
        <h1 className="sidebar-logo">InvoiceVault</h1>
        <p className="sidebar-subtitle">发票处理工作台</p>
      </div>

      <nav className="sidebar-nav">
        {NAV_ITEMS.map((item) => {
          const Icon = item.icon;
          return (
            <button
              key={item.path}
              className={`nav-item ${location.pathname === item.path ? "nav-item-active" : ""}`}
              onClick={() => navigate(item.path)}
            >
              <Icon className="nav-icon" size={18} strokeWidth={2} />
              <span className="nav-label">{item.label}</span>
              {item.path === "/import" && importBadgeCount ? (
                <span className="nav-badge nav-badge-import">
                  <span className="nav-spinner" />
                  {importBadgeCount}
                </span>
              ) : null}
              {item.path === "/invoices" && invoiceBadgeCount ? (
                <span className="nav-badge nav-badge-invoice">
                  {invoiceBadgeCount}
                </span>
              ) : null}
              {item.path === "/notifications" && unreadNotificationCount ? (
                <span className="nav-badge">{unreadNotificationCount}</span>
              ) : null}
            </button>
          );
        })}
      </nav>

      <div className="sidebar-footer">
        <span
          className={`status-dot ${healthReady ? (hasError ? "status-dot-error" : "status-dot-ok") : "status-dot-wait"}`}
        />
        <span className="status-text">
          {healthReady ? (hasError ? "后端异常" : "系统就绪") : "连接中"}
        </span>
      </div>
    </aside>
  );
}
