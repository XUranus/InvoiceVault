import React from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import "./styles.css";

type AppHealth = {
  app_data_dir: string;
  database_path: string;
  migration_version: number;
};

function App() {
  const [health, setHealth] = React.useState<AppHealth | null>(null);
  const [error, setError] = React.useState<string | null>(null);

  React.useEffect(() => {
    invoke<AppHealth>("app_health")
      .then(setHealth)
      .catch((err) => setError(String(err)));
  }, []);

  return (
    <main className="app-shell">
      <section className="topbar">
        <div>
          <h1>Receiptier</h1>
          <p>本地优先的发票处理工作台</p>
        </div>
        <span className={error ? "status status-error" : "status"}>
          {error ? "后端异常" : health ? "基础设施就绪" : "连接中"}
        </span>
      </section>

      <section className="workspace">
        <div className="panel">
          <h2>导入队列</h2>
          <p className="muted">下一步将接入 RAW 内容寻址存储、文件校验和导入任务状态机。</p>
        </div>
        <div className="panel">
          <h2>系统状态</h2>
          {error ? (
            <pre className="error-box">{error}</pre>
          ) : health ? (
            <dl className="health-grid">
              <dt>数据目录</dt>
              <dd>{health.app_data_dir}</dd>
              <dt>SQLite</dt>
              <dd>{health.database_path}</dd>
              <dt>迁移版本</dt>
              <dd>{health.migration_version}</dd>
            </dl>
          ) : (
            <p className="muted">正在读取 Tauri 后端状态。</p>
          )}
        </div>
      </section>
    </main>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

