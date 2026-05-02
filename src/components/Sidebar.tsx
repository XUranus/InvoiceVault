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

const NAV_ITEMS: { page: Page; label: string; emoji: string }[] = [
  { page: "dashboard", label: "仪表盘", emoji: "📊" },
  { page: "import", label: "导入", emoji: "📥" },
  { page: "invoices", label: "发票库", emoji: "🧾" },
  { page: "agent", label: "Agent", emoji: "🤖" },
  { page: "events", label: "事件", emoji: "📋" },
  { page: "notifications", label: "通知", emoji: "🔔" },
  { page: "settings", label: "设置", emoji: "⚙️" },
];

export function Sidebar({ activePage, onNavigate, healthReady, hasError, unreadNotificationCount, importBadgeCount, invoiceBadgeCount }: Props) {
  return (
    <aside className="sidebar">
      <div className="sidebar-brand">
        <h1 className="sidebar-logo">Receiptier</h1>
        <p className="sidebar-subtitle">发票处理工作台</p>
      </div>

      <nav className="sidebar-nav">
        {NAV_ITEMS.map((item) => (
          <button
            key={item.page}
            className={`nav-item ${activePage === item.page ? "nav-item-active" : ""}`}
            onClick={() => onNavigate(item.page)}
          >
            <span className="nav-emoji">{item.emoji}</span>
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
        ))}
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
