import { useLocation, useNavigate } from "react-router-dom";
import {
  Activity,
  Bot,
  LayoutDashboard,
  ReceiptText,
  Settings,
  Upload,
  type LucideIcon,
} from "lucide-react";
import { useAppStore } from "../stores/appStore";

const appIcon = new URL("../../icons/icon.png", import.meta.url).href;

const TOP_NAV_ITEMS: { path: string; label: string; icon: LucideIcon }[] = [
  { path: "/dashboard", label: "仪表盘", icon: LayoutDashboard },
  { path: "/import", label: "导入", icon: Upload },
  { path: "/invoices", label: "发票库", icon: ReceiptText },
  { path: "/agent", label: "Agent", icon: Bot },
  { path: "/events", label: "事件", icon: Activity },
];

const BOTTOM_NAV_ITEM = { path: "/settings", label: "设置", icon: Settings };

export function Sidebar() {
  const location = useLocation();
  const navigate = useNavigate();

  const health = useAppStore((s) => s.health);
  const error = useAppStore((s) => s.error);
  const unreadEventCount = useAppStore(
    (s) => s.unreadEventCount,
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
        {TOP_NAV_ITEMS.map((item) => {
          const Icon = item.icon;
          return (
            <button
              key={item.path}
              className={`nav-item ${(location.pathname === item.path || (item.path !== "/" && location.pathname.startsWith(item.path + "/"))) ? "nav-item-active" : ""}`}
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
              {item.path === "/events" && unreadEventCount ? (
                <span className="nav-badge">{unreadEventCount}</span>
              ) : null}
            </button>
          );
        })}
        <div className="sidebar-nav-spacer" />
        <button
          className={`nav-item ${(location.pathname === BOTTOM_NAV_ITEM.path || location.pathname.startsWith(BOTTOM_NAV_ITEM.path + "/")) ? "nav-item-active" : ""}`}
          onClick={() => navigate(BOTTOM_NAV_ITEM.path)}
        >
          <BOTTOM_NAV_ITEM.icon className="nav-icon" size={18} strokeWidth={2} />
          <span className="nav-label">{BOTTOM_NAV_ITEM.label}</span>
        </button>
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
