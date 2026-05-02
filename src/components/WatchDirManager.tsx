import React from "react";
import type { WatchDirStatus } from "../types";
import { addWatchDir, removeWatchDir, listWatchDirs, toggleWatchDir } from "../api";
import { open } from "@tauri-apps/plugin-dialog";

export function WatchDirManager() {
  const [dirs, setDirs] = React.useState<WatchDirStatus[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [error, setError] = React.useState<string | null>(null);

  const refresh = async () => {
    try {
      const result = await listWatchDirs();
      setDirs(result);
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  React.useEffect(() => {
    refresh();
  }, []);

  const handleAdd = async () => {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (!selected) return;
      const path = selected as string;
      await addWatchDir({ path });
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const handleToggle = async (id: number, enabled: boolean) => {
    try {
      await toggleWatchDir(id, enabled);
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const handleRemove = async (id: number) => {
    try {
      await removeWatchDir(id);
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  if (loading) return null;

  return (
    <div className="section">
      <div className="section-header">
        <h3>监听目录</h3>
        <button className="btn-small" onClick={handleAdd}>
          添加目录
        </button>
      </div>
      <p className="section-desc">
        配置后台自动监听目录，文件变化时自动导入。新文件稳定 2 秒后触发导入。
      </p>

      {error ? (
        <div className="alert alert-error" style={{ marginBottom: 12 }}>
          {error}
        </div>
      ) : null}

      {dirs.length === 0 ? (
        <p className="section-desc">暂无监听目录。</p>
      ) : (
        <div className="watch-dir-list">
          {dirs.map((d) => (
            <div key={d.id} className="watch-dir-card">
              <div className="watch-dir-info">
                <span
                  className={`watch-dir-status-dot ${d.running ? "running" : d.error ? "error" : "stopped"}`}
                  title={d.running ? "运行中" : d.error ? "错误" : "已停止"}
                />
                <span className="watch-dir-path" title={d.path}>
                  {d.path}
                </span>
                <span className="watch-dir-meta">
                  {d.recursive ? "递归" : "顶层"}
                  {d.extensions ? ` · ${d.extensions}` : " · 所有文件"}
                </span>
              </div>
              <div className="watch-dir-controls">
                <label className="toggle-label">
                  <input
                    type="checkbox"
                    checked={d.enabled}
                    onChange={(e) => handleToggle(d.id, e.target.checked)}
                  />
                  启用
                </label>
                <button
                  className="btn-small btn-danger"
                  onClick={() => handleRemove(d.id)}
                >
                  删除
                </button>
              </div>
              {d.error ? (
                <div className="watch-dir-error">{d.error}</div>
              ) : null}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
