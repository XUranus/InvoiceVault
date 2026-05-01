import React from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import "./styles.css";

type AppHealth = {
  app_data_dir: string;
  database_path: string;
  migration_version: number;
};

type ImportJob = {
  id: number;
  raw_file_id: number | null;
  source_path: string;
  original_name: string | null;
  current_name: string | null;
  status: string;
  sha256: string | null;
  storage_path: string | null;
  error_message: string | null;
  created_at: string;
  updated_at: string;
};

function App() {
  const [health, setHealth] = React.useState<AppHealth | null>(null);
  const [jobs, setJobs] = React.useState<ImportJob[]>([]);
  const [pathsText, setPathsText] = React.useState("");
  const [isImporting, setIsImporting] = React.useState(false);
  const [isDraggingFiles, setIsDraggingFiles] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  const refreshJobs = React.useCallback(() => {
    invoke<ImportJob[]>("list_import_jobs")
      .then(setJobs)
      .catch((err) => setError(String(err)));
  }, []);

  React.useEffect(() => {
    invoke<AppHealth>("app_health")
      .then(setHealth)
      .catch((err) => setError(String(err)));
    refreshJobs();
  }, [refreshJobs]);

  const importPaths = React.useCallback(async (paths: string[]) => {
    setIsImporting(true);
    setError(null);
    try {
      const imported = await invoke<ImportJob[]>("import_files", {
        request: { paths },
      });
      setJobs((current) => [...imported, ...current]);
      return imported;
    } catch (err) {
      setError(String(err));
      return [];
    } finally {
      setIsImporting(false);
    }
  }, []);

  React.useEffect(() => {
    let unlisten: (() => void) | null = null;

    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "enter" || event.payload.type === "over") {
          setIsDraggingFiles(true);
          return;
        }

        if (event.payload.type === "leave") {
          setIsDraggingFiles(false);
          return;
        }

        setIsDraggingFiles(false);
        void importPaths(event.payload.paths);
      })
      .then((handler) => {
        unlisten = handler;
      })
      .catch((err) => setError(String(err)));

    return () => {
      unlisten?.();
    };
  }, [importPaths]);

  const handleImport = async () => {
    const paths = pathsText
      .split("\n")
      .map((line) => line.trim())
      .filter(Boolean);

    if (paths.length === 0) {
      setError("请输入至少一个文件路径。");
      return;
    }

    const imported = await importPaths(paths);
    if (imported.length > 0) {
      setPathsText("");
    }
  };

  const handlePickFiles = async () => {
    setError(null);
    const selected = await open({
      multiple: true,
      directory: false,
      filters: [
        {
          name: "发票文件",
          extensions: ["pdf", "png", "jpg", "jpeg"],
        },
      ],
    });

    if (!selected) {
      return;
    }

    const paths = Array.isArray(selected) ? selected : [selected];
    if (paths.length === 0) {
      return;
    }

    await importPaths(paths);
  };

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
        <div className={`panel import-panel ${isDraggingFiles ? "import-panel-dragging" : ""}`}>
          <h2>导入队列</h2>
          <div className="drop-target">
            {isDraggingFiles ? "松开鼠标导入文件" : "拖入 PDF/PNG/JPG/JPEG 文件"}
          </div>
          <div className="import-form">
            <textarea
              value={pathsText}
              onChange={(event) => setPathsText(event.target.value)}
              placeholder="每行一个 PDF/PNG/JPG/JPEG 文件路径"
              rows={5}
            />
            <div className="import-actions">
              <button type="button" onClick={handlePickFiles} disabled={isImporting}>
                选择文件
              </button>
              <button type="button" onClick={handleImport} disabled={isImporting}>
                {isImporting ? "导入中" : "导入路径"}
              </button>
            </div>
          </div>
          <div className="job-list">
            {jobs.length === 0 ? (
              <p className="muted">暂无导入任务。</p>
            ) : (
              jobs.map((job) => (
                <article className="job-row" key={job.id}>
                  <div className="job-main">
                    <strong>{job.original_name ?? job.source_path}</strong>
                    {job.current_name ? <small>存储为 {job.current_name}</small> : null}
                    <span>{job.source_path}</span>
                    {job.error_message ? <em>{job.error_message}</em> : null}
                  </div>
                  <span className={`job-status job-status-${job.status}`}>
                    {statusLabel(job.status)}
                  </span>
                </article>
              ))
            )}
          </div>
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

function statusLabel(status: string) {
  const labels: Record<string, string> = {
    completed: "已完成",
    duplicate: "重复",
    failed: "失败",
  };

  return labels[status] ?? status;
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
