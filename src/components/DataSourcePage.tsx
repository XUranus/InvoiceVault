import React from "react";
import { useNavigate } from "react-router-dom";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  WatchDirStatus,
  EmailSource,
  AddEmailSourceRequest,
  UpdateEmailSourceRequest,
  EmailTestResult,
} from "../types";
import {
  listWatchDirs,
  addWatchDir,
  removeWatchDir,
  toggleWatchDir,
  updateWatchDir,
  listEmailSources,
  addEmailSource,
  updateEmailSource,
  removeEmailSource,
  toggleEmailSource,
  syncEmailSource,
  testEmailConnection,
} from "../api";

// --- Watch Dir Edit Modal ---

function WatchDirEditModal({
  dir,
  onClose,
  onSaved,
}: {
  dir: WatchDirStatus | null;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [path, setPath] = React.useState(dir?.path ?? "");
  const [extensions, setExtensions] = React.useState(dir?.extensions ?? "");
  const [nameKeywords, setNameKeywords] = React.useState(dir?.name_keywords ?? "");
  const [maxFileAgeDays, setMaxFileAgeDays] = React.useState(String(dir?.max_file_age_days ?? 0));
  const [recursive, setRecursive] = React.useState(dir?.recursive ?? true);
  const [saving, setSaving] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  const handleBrowse = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (selected) setPath(selected as string);
  };

  const handleSave = async () => {
    if (!path.trim()) {
      setError("请选择目录路径");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      if (dir) {
        await updateWatchDir(dir.id, {
          path: path.trim(),
          extensions: extensions.trim() || undefined,
          name_keywords: nameKeywords.trim() || undefined,
          max_file_age_days: parseInt(maxFileAgeDays) || 0,
          recursive,
        });
      } else {
        await addWatchDir({
          path: path.trim(),
          extensions: extensions.trim() || undefined,
          name_keywords: nameKeywords.trim() || undefined,
          max_file_age_days: parseInt(maxFileAgeDays) || 0,
          recursive,
        });
      }
      onSaved();
      onClose();
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-card" onClick={(e) => e.stopPropagation()}>
        <h3 className="modal-title">{dir ? "编辑监听目录" : "添加监听目录"}</h3>

        <label className="form-label">目录路径</label>
        <div className="form-row">
          <input
            className="form-input"
            value={path}
            onChange={(e) => setPath(e.target.value)}
            placeholder="/path/to/watch"
          />
          <button className="btn-small" onClick={handleBrowse}>
            浏览
          </button>
        </div>

        <label className="form-label">文件扩展名过滤（逗号分隔，留空=所有文件）</label>
        <input
          className="form-input"
          value={extensions}
          onChange={(e) => setExtensions(e.target.value)}
          placeholder="pdf,png,jpg"
        />

        <label className="form-label">文件名关键词过滤（逗号分隔，留空=不过滤）</label>
        <input
          className="form-input"
          value={nameKeywords}
          onChange={(e) => setNameKeywords(e.target.value)}
          placeholder="发票,票据"
        />

        <label className="form-label">文件最大天数（0=不限制）</label>
        <input
          className="form-input"
          type="number"
          min={0}
          value={maxFileAgeDays}
          onChange={(e) => setMaxFileAgeDays(e.target.value)}
        />

        <label className="toggle-label" style={{ marginTop: 8 }}>
          <input
            type="checkbox"
            checked={recursive}
            onChange={(e) => setRecursive(e.target.checked)}
          />
          递归监听子目录
        </label>

        {error ? <div className="alert alert-error">{error}</div> : null}

        <div className="modal-actions">
          <button className="btn-small" onClick={onClose}>
            取消
          </button>
          <button
            className="btn-small btn-primary"
            onClick={handleSave}
            disabled={saving}
          >
            {saving ? "保存中..." : "保存"}
          </button>
        </div>
      </div>
    </div>
  );
}

// --- Email Source Edit Modal ---

function EmailSourceEditModal({
  source,
  onClose,
  onSaved,
}: {
  source: EmailSource | null;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [name, setName] = React.useState(source?.name ?? "");
  const [protocol, setProtocol] = React.useState(source?.protocol ?? "imap");
  const [host, setHost] = React.useState(source?.imap_host ?? "");
  const [port, setPort] = React.useState(
    String(source?.imap_port ?? (source?.protocol === "pop3" ? 995 : 993)),
  );
  const [username, setUsername] = React.useState(source?.username ?? "");
  const [password, setPassword] = React.useState(source?.password ?? "");
  const [useSsl, setUseSsl] = React.useState(source?.use_ssl ?? true);
  const [folder, setFolder] = React.useState(source?.folder ?? "INBOX");
  const [nameKeywords, setNameKeywords] = React.useState(source?.name_keywords ?? "");
  const [maxAgeDays, setMaxAgeDays] = React.useState(String(source?.max_email_age_days ?? 0));
  const [pollInterval, setPollInterval] = React.useState(
    String(source?.poll_interval_seconds ?? 60),
  );
  const [saving, setSaving] = React.useState(false);
  const [testing, setTesting] = React.useState(false);
  const [testResult, setTestResult] = React.useState<EmailTestResult | null>(null);
  const [error, setError] = React.useState<string | null>(null);

  const handleProtocolChange = (p: string) => {
    setProtocol(p);
    if (!source) {
      setPort(p === "pop3" ? "995" : "993");
    }
  };

  const handleTest = async () => {
    if (!host.trim() || !username.trim()) {
      setError("请填写主机和用户名");
      return;
    }
    setTesting(true);
    setTestResult(null);
    setError(null);
    try {
      const result = await testEmailConnection({
        protocol,
        host: host.trim(),
        port: parseInt(port) || (protocol === "pop3" ? 995 : 993),
        username: username.trim(),
        password,
        use_ssl: useSsl,
        folder: folder.trim() || "INBOX",
      });
      setTestResult(result);
    } catch (err) {
      setError(String(err));
    } finally {
      setTesting(false);
    }
  };

  const handleSave = async () => {
    if (!host.trim() || !username.trim()) {
      setError("请填写主机和用户名");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      const data = {
        name: name.trim() || undefined,
        protocol,
        imap_host: host.trim(),
        imap_port: parseInt(port) || (protocol === "pop3" ? 995 : 993),
        username: username.trim(),
        password,
        use_ssl: useSsl,
        folder: protocol === "pop3" ? "" : (folder.trim() || "INBOX"),
        name_keywords: nameKeywords.trim() || undefined,
        max_email_age_days: parseInt(maxAgeDays) || 0,
        poll_interval_seconds: parseInt(pollInterval) || 60,
      };
      if (source) {
        await updateEmailSource(source.id, data);
      } else {
        await addEmailSource(data);
      }
      onSaved();
      onClose();
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-card" onClick={(e) => e.stopPropagation()}>
        <h3 className="modal-title">{source ? "编辑邮件源" : "添加邮件源"}</h3>

        <label className="form-label">名称（可选，显示用）</label>
        <input
          className="form-input"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="财务邮箱"
        />

        <label className="form-label">协议</label>
        <div className="form-row">
          <button
            className={`btn-small ${protocol === "imap" ? "btn-primary" : ""}`}
            onClick={() => handleProtocolChange("imap")}
            type="button"
          >
            IMAP
          </button>
          <button
            className={`btn-small ${protocol === "pop3" ? "btn-primary" : ""}`}
            onClick={() => handleProtocolChange("pop3")}
            type="button"
          >
            POP3
          </button>
        </div>

        <label className="form-label">{protocol === "pop3" ? "POP3" : "IMAP"} 主机</label>
        <input
          className="form-input"
          value={host}
          onChange={(e) => setHost(e.target.value)}
          placeholder={protocol === "pop3" ? "pop.example.com" : "imap.example.com"}
        />

        <label className="form-label">端口</label>
        <input
          className="form-input"
          type="number"
          value={port}
          onChange={(e) => setPort(e.target.value)}
        />

        <label className="form-label">用户名</label>
        <input
          className="form-input"
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          placeholder="user@example.com"
        />

        <label className="form-label">密码</label>
        <input
          className="form-input"
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
        />

        <label className="toggle-label">
          <input
            type="checkbox"
            checked={useSsl}
            onChange={(e) => setUseSsl(e.target.checked)}
          />
          使用 SSL
        </label>

        {protocol !== "pop3" ? (
          <>
            <label className="form-label">邮件文件夹</label>
            <input
              className="form-input"
              value={folder}
              onChange={(e) => setFolder(e.target.value)}
              placeholder="INBOX"
            />
          </>
        ) : null}

        <label className="form-label">附件文件名关键词过滤（逗号分隔，留空=不过滤）</label>
        <input
          className="form-input"
          value={nameKeywords}
          onChange={(e) => setNameKeywords(e.target.value)}
          placeholder="发票,invoice"
        />

        <label className="form-label">邮件最大天数（0=不限制）</label>
        <input
          className="form-input"
          type="number"
          min={0}
          value={maxAgeDays}
          onChange={(e) => setMaxAgeDays(e.target.value)}
        />

        <label className="form-label">轮询间隔（秒）</label>
        <input
          className="form-input"
          type="number"
          min={10}
          value={pollInterval}
          onChange={(e) => setPollInterval(e.target.value)}
        />

        {testResult ? (
          <div
            className={`alert ${testResult.success ? "alert-success" : "alert-error"}`}
          >
            {testResult.message}
          </div>
        ) : null}
        {error ? <div className="alert alert-error">{error}</div> : null}

        <div className="modal-actions">
          <button
            className="btn-small"
            onClick={handleTest}
            disabled={testing}
          >
            {testing ? "测试中..." : "测试连接"}
          </button>
          <div style={{ flex: 1 }} />
          <button className="btn-small" onClick={onClose}>
            取消
          </button>
          <button
            className="btn-small btn-primary"
            onClick={handleSave}
            disabled={saving}
          >
            {saving ? "保存中..." : "保存"}
          </button>
        </div>
      </div>
    </div>
  );
}

// --- Main Page ---

export function DataSourcePage() {
  const navigate = useNavigate();
  const [dirs, setDirs] = React.useState<WatchDirStatus[]>([]);
  const [emailSources, setEmailSources] = React.useState<EmailSource[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [error, setError] = React.useState<string | null>(null);
  const [editingDir, setEditingDir] = React.useState<WatchDirStatus | null | "new">(null);
  const [editingEmail, setEditingEmail] = React.useState<EmailSource | null | "new">(null);
  const [syncingIds, setSyncingIds] = React.useState<Set<number>>(new Set());

  const refresh = React.useCallback(async () => {
    try {
      const [dirResult, emailResult] = await Promise.all([
        listWatchDirs(),
        listEmailSources(),
      ]);
      setDirs(dirResult);
      setEmailSources(emailResult);
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  React.useEffect(() => {
    refresh();
  }, [refresh]);

  const handleDirToggle = async (id: number, enabled: boolean) => {
    try {
      await toggleWatchDir(id, enabled);
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const handleDirRemove = async (id: number) => {
    try {
      await removeWatchDir(id);
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const handleEmailToggle = async (id: number, enabled: boolean) => {
    try {
      await toggleEmailSource(id, enabled);
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const handleEmailRemove = async (id: number) => {
    try {
      await removeEmailSource(id);
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const handleEmailSync = async (id: number) => {
    setSyncingIds((prev) => new Set(prev).add(id));
    try {
      await syncEmailSource(id);
      await refresh();
    } catch (err) {
      setError(String(err));
    } finally {
      setSyncingIds((prev) => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    }
  };

  const statusLabel = (status: string) => {
    switch (status) {
      case "syncing":
        return "同步中...";
      case "error":
        return "错误";
      default:
        return "空闲";
    }
  };

  if (loading) return null;

  return (
    <div className="page">
      <div className="page-header">
        <button className="btn-back" onClick={() => navigate("/import")}>
          &larr; 返回
        </button>
        <h2>数据源配置</h2>
      </div>

      {error ? (
        <div className="alert alert-error" style={{ marginBottom: 12 }}>
          {error}
        </div>
      ) : null}

      {/* Watch Directories Section */}
      <div className="section">
        <div className="section-header">
          <h3>监听目录</h3>
          <button
            className="btn-small"
            onClick={() => setEditingDir("new")}
            disabled={dirs.length >= 5}
          >
            添加目录
          </button>
        </div>
        <p className="section-desc">
          配置后台自动监听目录，文件变化时自动导入。最多 5 个目录。
        </p>

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
                </div>
                <div className="watch-dir-meta-row">
                  <span>
                    {d.recursive ? "递归" : "顶层"}
                    {d.extensions ? ` · 过滤: ${d.extensions}` : " · 所有文件"}
                  </span>
                  {d.name_keywords ? (
                    <span> · 关键词: {d.name_keywords}</span>
                  ) : null}
                  {d.max_file_age_days > 0 ? (
                    <span> · {d.max_file_age_days}天内</span>
                  ) : null}
                </div>
                <div className="watch-dir-controls">
                  <label className="toggle-label">
                    <input
                      type="checkbox"
                      checked={d.enabled}
                      onChange={(e) => handleDirToggle(d.id, e.target.checked)}
                    />
                    启用
                  </label>
                  <button
                    className="btn-small"
                    onClick={() => setEditingDir(d)}
                  >
                    编辑
                  </button>
                  <button
                    className="btn-small btn-danger"
                    onClick={() => handleDirRemove(d.id)}
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

      {/* Email Sources Section */}
      <div className="section" style={{ marginTop: 24 }}>
        <div className="section-header">
          <h3>邮件导入</h3>
          <button
            className="btn-small"
            onClick={() => setEditingEmail("new")}
          >
            添加邮件源
          </button>
        </div>
        <p className="section-desc">
          通过 IMAP 协议定期拉取邮件附件，自动导入发票。
        </p>

        {emailSources.length === 0 ? (
          <p className="section-desc">暂无邮件数据源。</p>
        ) : (
          <div className="watch-dir-list">
            {emailSources.map((s) => (
              <div key={s.id} className="watch-dir-card">
                <div className="watch-dir-info">
                  <span
                    className={`watch-dir-status-dot ${
                      s.status === "syncing"
                        ? "running"
                        : s.status === "error"
                          ? "error"
                          : s.enabled
                            ? "running"
                            : "stopped"
                    }`}
                    title={statusLabel(s.status)}
                  />
                  <span className="watch-dir-path">
                    {s.name || s.username}
                  </span>
                </div>
                <div className="watch-dir-meta-row">
                  <span>
                    {s.protocol.toUpperCase()} {s.imap_host}:{s.imap_port}
                    {s.protocol !== "pop3" && s.folder ? ` · ${s.folder}` : ""}
                    {" · "}{s.poll_interval_seconds}s轮询
                  </span>
                  {s.name_keywords ? (
                    <span> · 关键词: {s.name_keywords}</span>
                  ) : null}
                  {s.max_email_age_days > 0 ? (
                    <span> · {s.max_email_age_days}天内</span>
                  ) : null}
                </div>
                {s.last_sync_at ? (
                  <div className="watch-dir-meta-row">
                    <span>上次同步: {s.last_sync_at}</span>
                  </div>
                ) : null}
                {s.error_message ? (
                  <div className="watch-dir-error">{s.error_message}</div>
                ) : null}
                <div className="watch-dir-controls">
                  <label className="toggle-label">
                    <input
                      type="checkbox"
                      checked={s.enabled}
                      onChange={(e) =>
                        handleEmailToggle(s.id, e.target.checked)
                      }
                    />
                    启用
                  </label>
                  <button
                    className="btn-small"
                    onClick={() => setEditingEmail(s)}
                  >
                    编辑
                  </button>
                  <button
                    className="btn-small"
                    onClick={() => handleEmailSync(s.id)}
                    disabled={syncingIds.has(s.id)}
                  >
                    {syncingIds.has(s.id) ? "同步中..." : "立即同步"}
                  </button>
                  <button
                    className="btn-small btn-danger"
                    onClick={() => handleEmailRemove(s.id)}
                  >
                    删除
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Modals */}
      {editingDir !== null ? (
        <WatchDirEditModal
          dir={editingDir === "new" ? null : editingDir}
          onClose={() => setEditingDir(null)}
          onSaved={refresh}
        />
      ) : null}

      {editingEmail !== null ? (
        <EmailSourceEditModal
          source={editingEmail === "new" ? null : editingEmail}
          onClose={() => setEditingEmail(null)}
          onSaved={refresh}
        />
      ) : null}
    </div>
  );
}

export default DataSourcePage;
