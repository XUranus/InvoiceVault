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

const appIcon = new URL("../../icons/icon.png", import.meta.url).href;

type Page = "dashboard" | "import" | "invoices" | "agent" | "events" | "notifications" | "settings";

type Props = {
  activePage: Page;
  onNavigate: (page: Page) => void;
  healthReady: boolean;
  hasError: boolean;
  unreadNotificationCount?: number;
  importBadgeCount?: number;
  invoiceBadgeCount?: number;
};

const NAV_ITEMS: { page: Page; label: string; icon: LucideIcon }[] = [
  { page: "dashboard", label: "仪表盘", icon: LayoutDashboard },
  { page: "import", label: "导入", icon: Upload },
  { page: "invoices", label: "发票库", icon: ReceiptText },
  { page: "agent", label: "Agent", icon: Bot },
  { page: "events", label: "事件", icon: Activity },
  { page: "notifications", label: "通知", icon: Bell },
  { page: "settings", label: "设置", icon: Settings },
];

export function Sidebar({ activePage, onNavigate, healthReady, hasError, unreadNotificationCount, importBadgeCount, invoiceBadgeCount }: Props) {
  return (
    <aside className="sidebar">
      <div className="sidebar-brand">
        <img className="sidebar-app-icon" src={appIcon} alt="" aria-hidden="true" />
        <h1 className="sidebar-logo">InvoiceVault</h1>
        <p className="sidebar-subtitle">发票处理工作台</p>
      </div>

      <nav className="sidebar-nav">
        {NAV_ITEMS.map((item) => {
          const Icon = item.icon;
          return (
            <button
              key={item.page}
              className={`nav-item ${activePage === item.page ? "nav-item-active" : ""}`}
              onClick={() => onNavigate(item.page)}
            >
            <Icon className="nav-icon" size={18} strokeWidth={2} />
            <span className="nav-label">{item.label}</span>
            {item.page === "import" && importBadgeCount ? (
              <span className="nav-badge nav-badge-import">
                <span className="nav-spinner" />
                {importBadgeCount}
              </span>
            ) : null}
            {item.page === "invoices" && invoiceBadgeCount ? (
              <span className="nav-badge nav-badge-invoice">{invoiceBadgeCount}</span>
            ) : null}
            {item.page === "notifications" && unreadNotificationCount ? (
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
