type Page = "dashboard" | "import" | "invoices" | "agent" | "settings";

type Props = {
  activePage: Page;
  onNavigate: (page: Page) => void;
  healthReady: boolean;
  hasError: boolean;
};

const NAV_ITEMS: { page: Page; label: string; emoji: string }[] = [
  { page: "dashboard", label: "仪表盘", emoji: "📊" },
  { page: "import", label: "导入", emoji: "📥" },
  { page: "invoices", label: "发票库", emoji: "🧾" },
  { page: "agent", label: "Agent", emoji: "🤖" },
  { page: "settings", label: "设置", emoji: "⚙️" },
];

export function Sidebar({ activePage, onNavigate, healthReady, hasError }: Props) {
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
