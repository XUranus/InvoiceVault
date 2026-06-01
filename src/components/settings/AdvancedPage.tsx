import React from "react";
import type { ExternalDependencyStatus } from "../../types";
import {
  exportLogs,
  exportBackup,
  cleanupStorage,
  checkExternalDependencies,
  getLogLevel,
  setLogLevel,
} from "../../api";
import type { ExportLogsResult, CleanupStorageResult } from "../../types";
import { ConfirmDialog } from "../ConfirmDialog";
import { useAppStore } from "../../stores/appStore";

export function AdvancedPage() {
  const health = useAppStore((s) => s.health);
  const appVersion = useAppStore((s) => s.appVersion);

  // --- External Dependencies ---
  const [dependencyStatuses, setDependencyStatuses] = React.useState<ExternalDependencyStatus[]>([]);
  const [checkingDependencies, setCheckingDependencies] = React.useState(false);
  const [depsExpanded, setDepsExpanded] = React.useState(false);
  const depsCheckedRef = React.useRef(false);

  // --- Log Level ---
  const [logLevel, setLogLevelState] = React.useState<string>("info");
  const [logLevelSaving, setLogLevelSaving] = React.useState(false);
  const [logLevelMessage, setLogLevelMessage] = React.useState<string | null>(null);

  React.useEffect(() => {
    getLogLevel().then(setLogLevelState).catch(() => {});
  }, []);

  const handleLogLevelChange = React.useCallback(async (newLevel: string) => {
    setLogLevelState(newLevel);
    setLogLevelSaving(true);
    setLogLevelMessage(null);
    try {
      await setLogLevel(newLevel);
      setLogLevelMessage("日志级别已更新，重启后仍然生效");
    } catch (e) {
      setLogLevelMessage(`设置失败: ${e}`);
    } finally {
      setLogLevelSaving(false);
    }
  }, []);

  // --- Data Management ---
  const [exporting, setExporting] = React.useState(false);
  const [exportResult, setExportResult] = React.useState<ExportLogsResult | null>(null);
  const [exportError, setExportError] = React.useState<string | null>(null);
  const [backingUp, setBackingUp] = React.useState(false);
  const [backupResult, setBackupResult] = React.useState<ExportLogsResult | null>(null);
  const [backupError, setBackupError] = React.useState<string | null>(null);
  const [cleanupDialogOpen, setCleanupDialogOpen] = React.useState(false);
  const [cleaning, setCleaning] = React.useState(false);
  const [cleanupResult, setCleanupResult] = React.useState<CleanupStorageResult | null>(null);
  const [cleanupError, setCleanupError] = React.useState<string | null>(null);

  // --- System ---
  const [developerCopied, setDeveloperCopied] = React.useState(false);
  const [dataDirCopied, setDataDirCopied] = React.useState(false);
  const [dbPathCopied, setDbPathCopied] = React.useState(false);

  // Load on mount

  const refreshExternalDependencies = React.useCallback(async () => {
    setCheckingDependencies(true);
    try {
      const result = await checkExternalDependencies();
      setDependencyStatuses(result);
      depsCheckedRef.current = true;
    } catch {
      setDependencyStatuses([]);
    } finally {
      setCheckingDependencies(false);
    }
  }, []);

  // Check on first expand only
  React.useEffect(() => {
    if (depsExpanded && !depsCheckedRef.current) {
      refreshExternalDependencies();
    }
  }, [depsExpanded, refreshExternalDependencies]);

  // --- Data Management handlers ---
  const handleExportLogs = async () => {
    try {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const filePath = await save({
        title: "选择导出位置",
        defaultPath: "invoicevault-logs.zip",
        filters: [{ name: "ZIP 压缩包", extensions: ["zip"] }],
      });
      if (!filePath) return;

      setExporting(true);
      setExportResult(null);
      setExportError(null);
      const result = await exportLogs(filePath);
      setExportResult(result);
    } catch (err) {
      setExportError(String(err));
    } finally {
      setExporting(false);
    }
  };

  const handleBackup = async () => {
    try {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const filePath = await save({
        title: "选择备份保存位置",
        defaultPath: "invoicevault-backup.zip",
        filters: [{ name: "ZIP 压缩包", extensions: ["zip"] }],
      });
      if (!filePath) return;

      setBackingUp(true);
      setBackupResult(null);
      setBackupError(null);
      const result = await exportBackup(filePath);
      setBackupResult(result);
    } catch (err) {
      setBackupError(String(err));
    } finally {
      setBackingUp(false);
    }
  };

  const handleCleanupStorage = async () => {
    setCleanupDialogOpen(false);
    setCleaning(true);
    setCleanupResult(null);
    setCleanupError(null);
    try {
      const result = await cleanupStorage();
      setCleanupResult(result);
    } catch (err) {
      setCleanupError(String(err));
    } finally {
      setCleaning(false);
    }
  };

  // --- Utility ---
  const openFolder = async (path: string) => {
    try {
      const { open } = await import("@tauri-apps/plugin-shell");
      await open(path);
    } catch {
      // ignore if shell plugin not available
    }
  };

  const openParentFolder = (path: string) => {
    const normalized = path.replace(/\\/g, "/");
    const parent = normalized.includes("/")
      ? normalized.slice(0, normalized.lastIndexOf("/"))
      : path;
    openFolder(parent || path);
  };

  const openExternalLink = (url: string) => {
    window.open(url, "_blank", "noopener,noreferrer");
  };

  const copyDeveloperEmail = async () => {
    try {
      await navigator.clipboard.writeText("xuranus42@qq.com");
      setDeveloperCopied(true);
      window.setTimeout(() => setDeveloperCopied(false), 1600);
    } catch {
      // ignore if clipboard is unavailable
    }
  };

  const copyToClipboard = async (
    text: string,
    setCopied: (v: boolean) => void,
  ) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1600);
    } catch {
      // ignore if clipboard is unavailable
    }
  };

  return (
    <>
      {/* Log Level */}
      <div className="section">
        <h3>日志级别</h3>
        <label className="form-field" style={{ maxWidth: 360 }}>
          <span>详细程度</span>
          <select
            value={logLevel}
            onChange={(e) => handleLogLevelChange(e.target.value)}
            disabled={logLevelSaving}
            style={{
              textTransform: "uppercase",
            }}
          >
            <option value="error">ERROR — 仅错误</option>
            <option value="warn">WARN — 警告和错误</option>
            <option value="info">INFO — 常规信息（默认）</option>
            <option value="debug">DEBUG — 调试信息</option>
            <option value="trace">TRACE — 全部追踪</option>
          </select>
          {logLevelSaving || logLevelMessage ? (
            <span style={{ fontSize: 12, color: logLevelSaving ? "var(--color-text-secondary)" : "var(--color-success)", fontWeight: 400 }}>
              {logLevelSaving ? "保存中..." : logLevelMessage}
            </span>
          ) : null}
        </label>
      </div>

      {/* External Dependencies */}
      <div className="section">
        <div className="section-header">
          <h3
            style={{ cursor: "pointer", userSelect: "none" }}
            onClick={() => setDepsExpanded((v) => !v)}
          >
            外部依赖 {depsExpanded ? "▾" : "▸"}
          </h3>
          {depsExpanded ? (
            <button
              className="btn-small"
              type="button"
              onClick={refreshExternalDependencies}
              disabled={checkingDependencies}
            >
              {checkingDependencies ? "检测中..." : "重新检测"}
            </button>
          ) : null}
        </div>
        {depsExpanded ? (
          <>
            <p className="section-desc">
              PDF 渲染依赖本机 Poppler 命令。Windows 下需要确保 Poppler 已安装并加入 PATH。
            </p>
            <div className="dependency-list">
              {dependencyStatuses.map((dependency) => (
                <div className="dependency-card" key={dependency.command}>
                  <div className="dependency-main">
                    <span
                      className={`dependency-dot ${dependency.available ? "dependency-dot-ok" : "dependency-dot-error"}`}
                    />
                    <div>
                      <strong>{dependency.name}</strong>
                      <span className="dependency-command mono">{dependency.command}</span>
                    </div>
                  </div>
                  <div className="dependency-detail">
                    <span className={`mini-tag ${dependency.available ? "tag-recognized" : "tag-flagged"}`}>
                      {dependency.available ? "可用" : "未找到"}
                    </span>
                    <span className="dependency-version">
                      {dependency.version ?? dependency.error ?? "未返回版本信息"}
                    </span>
                  </div>
                  {!dependency.available ? (
                    <ExternalDependencyHelp command={dependency.command} />
                  ) : null}
                </div>
              ))}
              {!checkingDependencies && dependencyStatuses.length === 0 ? (
                <p className="muted">暂未获取依赖状态。</p>
              ) : null}
            </div>
          </>
        ) : null}
      </div>

      {/* Data Management */}
      <div className="section">
        <h3>数据管理</h3>

        <div className="section-sub">
          <h4>导出日志</h4>
          <p className="section-desc">
            将数据库、配置文件及系统信息打包为 ZIP 压缩包用于问题诊断。
          </p>
          <button className="btn-primary" onClick={handleExportLogs} disabled={exporting}>
            {exporting && <span className="inline-spinner" style={{ marginRight: 6 }} />}
            {exporting ? "导出中..." : "导出日志"}
          </button>
          {exportResult ? (
            <div className="test-result" style={{ marginTop: 12 }}>
              <div className="test-result-row">
                <span>文件路径</span>
                <strong className="mono" style={{ wordBreak: "break-all" }}>{exportResult.file_path}</strong>
              </div>
              <div className="test-result-row">
                <span>文件大小</span>
                <strong>{formatFileSize(exportResult.byte_size)}</strong>
              </div>
            </div>
          ) : null}
          {exportError ? (
            <div className="alert alert-error" style={{ marginTop: 12 }}>{exportError}</div>
          ) : null}
        </div>

        <div className="section-sub" style={{ marginTop: 20 }}>
          <h4>基础备份</h4>
          <p className="section-desc">
            将全部用户数据（数据库、配置、日志、文件归档）打包压缩为 ZIP 文件。
          </p>
          <button className="btn-primary" onClick={handleBackup} disabled={backingUp}>
            {backingUp && <span className="inline-spinner" style={{ marginRight: 6 }} />}
            {backingUp ? "备份中..." : "基础备份"}
          </button>
          {backupResult ? (
            <div className="test-result" style={{ marginTop: 12 }}>
              <div className="test-result-row">
                <span>文件路径</span>
                <strong className="mono" style={{ wordBreak: "break-all" }}>{backupResult.file_path}</strong>
              </div>
              <div className="test-result-row">
                <span>文件大小</span>
                <strong>{formatFileSize(backupResult.byte_size)}</strong>
              </div>
            </div>
          ) : null}
          {backupError ? (
            <div className="alert alert-error" style={{ marginTop: 12 }}>{backupError}</div>
          ) : null}
        </div>

        <div className="section-sub" style={{ marginTop: 20 }}>
          <h4>存储清理</h4>
          <p className="section-desc">
            扫描文件归档目录，清理未被数据库引用的失效文件及无效数据库记录。
          </p>
          <button
            className="btn-danger"
            onClick={() => setCleanupDialogOpen(true)}
            disabled={cleaning}
          >
            {cleaning && <span className="inline-spinner" style={{ marginRight: 6 }} />}
            {cleaning ? "清理中..." : "存储清理"}
          </button>
          {cleanupResult ? (
            <div className="test-result" style={{ marginTop: 12 }}>
              <div className="test-result-row">
                <span>清理文件数</span>
                <strong>{cleanupResult.files_removed}</strong>
              </div>
              <div className="test-result-row">
                <span>清理记录数</span>
                <strong>{cleanupResult.db_records_removed}</strong>
              </div>
              <div className="test-result-row">
                <span>释放空间</span>
                <strong>{formatFileSize(cleanupResult.bytes_freed)}</strong>
              </div>
            </div>
          ) : null}
          {cleanupError ? (
            <div className="alert alert-error" style={{ marginTop: 12 }}>{cleanupError}</div>
          ) : null}
        </div>
      </div>

      <ConfirmDialog
        open={cleanupDialogOpen}
        title="存储清理"
        message="将扫描文件归档目录并清理失效文件及无效数据库记录。清理操作不可撤销，是否继续？"
        confirmLabel="开始清理"
        danger
        loading={cleaning}
        onConfirm={handleCleanupStorage}
        onCancel={() => setCleanupDialogOpen(false)}
      />

      {/* System Info */}
      {health ? (
        <div className="section">
          <h3>系统信息</h3>
          <dl className="info-grid">
            <dt>应用版本</dt>
            <dd>{appVersion || "—"}</dd>
            <dt>数据目录</dt>
            <dd>
              <button
                className="path-link path-button"
                type="button"
                onClick={() => copyToClipboard(health.app_data_dir, setDataDirCopied)}
                title="点击复制路径"
              >
                {health.app_data_dir}
              </button>
              {dataDirCopied ? <span className="copy-hint">已复制</span> : null}
            </dd>
            <dt>数据库</dt>
            <dd>
              <button
                className="path-link path-button"
                type="button"
                onClick={() => copyToClipboard(health.database_path, setDbPathCopied)}
                title="点击复制路径"
              >
                {health.database_path}
              </button>
              {dbPathCopied ? <span className="copy-hint">已复制</span> : null}
            </dd>
            <dt>迁移版本</dt>
            <dd>{health.migration_version}</dd>
            <dt>开发者</dt>
            <dd>
              <button
                className="path-link path-button"
                type="button"
                onClick={copyDeveloperEmail}
                title="点击复制邮箱"
              >
                xuranus42@qq.com
              </button>
              {developerCopied ? <span className="copy-hint">已复制</span> : null}
            </dd>
            <dt>GitHub</dt>
            <dd>
              <button
                className="path-link path-button"
                type="button"
                onClick={() => openExternalLink("https://github.com/XUranus/InvoiceVault")}
                title="打开 GitHub 仓库"
              >
                https://github.com/XUranus/InvoiceVault
              </button>
            </dd>
          </dl>
        </div>
      ) : null}
    </>
  );
}

function ExternalDependencyHelp({ command }: { command: string }) {
  const isWindows = navigator.platform.startsWith("Win");
  const isMac = navigator.platform.startsWith("Mac");
  const isLinux = !isWindows && !isMac;

  if (command === "pdftoppm") {
    if (isLinux) {
      return (
        <div className="dependency-help">
          <strong>Linux 安装 Poppler</strong>
          <span>Poppler 包含 pdftoppm 工具，用于 PDF 渲染。</span>
          <code>sudo apt install poppler-utils</code>
        </div>
      );
    }
    if (isMac) {
      return (
        <div className="dependency-help">
          <strong>macOS 安装 Poppler</strong>
          <span>使用 Homebrew 安装 Poppler。</span>
          <code>brew install poppler</code>
        </div>
      );
    }
    return (
      <div className="dependency-help">
        <strong>Windows 安装 Poppler</strong>
        <span>下载 Poppler for Windows，解压后把包含 pdftoppm.exe 的 bin 目录加入 PATH。</span>
        <code>pdftoppm -h</code>
      </div>
    );
  }

  return null;
}

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export default AdvancedPage;
