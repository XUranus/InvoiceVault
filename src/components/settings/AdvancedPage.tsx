import React from "react";
import type {
  ExternalDependencyStatus,
  BadgeConfig,
} from "../../types";
import {
  exportLogs,
  exportBackup,
  cleanupStorage,
  checkExternalDependencies,
  getBadgeConfig,
  setBadgeConfig,
} from "../../api";
import type { ExportLogsResult, CleanupStorageResult } from "../../types";
import { ConfirmDialog } from "../ConfirmDialog";
import { useAppStore } from "../../stores/appStore";
import { APP_CONFIG } from "../../appConfig";

export function AdvancedPage() {
  const health = useAppStore((s) => s.health);

  // --- External Dependencies ---
  const [dependencyStatuses, setDependencyStatuses] = React.useState<ExternalDependencyStatus[]>([]);
  const [checkingDependencies, setCheckingDependencies] = React.useState(false);

  // --- Badge Config ---
  const [badgeConfig, setBadgeConfigState] = React.useState<BadgeConfig>({ groups: [] });
  const [badgeOptionDrafts, setBadgeOptionDrafts] = React.useState<Record<number, string>>({});
  const [savingBadgeConfig, setSavingBadgeConfig] = React.useState(false);
  const [badgeConfigMessage, setBadgeConfigMessage] = React.useState<string | null>(null);

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

  // Load on mount
  React.useEffect(() => {
    let cancelled = false;
    Promise.all([
      getBadgeConfig().catch(() => null),
    ]).then(([badge]) => {
      if (cancelled) return;
      if (badge) setBadgeConfigState(badge);
    });
    return () => { cancelled = true; };
  }, []);

  const refreshExternalDependencies = React.useCallback(async () => {
    setCheckingDependencies(true);
    try {
      const result = await checkExternalDependencies();
      setDependencyStatuses(result);
    } catch {
      setDependencyStatuses([]);
    } finally {
      setCheckingDependencies(false);
    }
  }, []);

  React.useEffect(() => {
    refreshExternalDependencies();
  }, [refreshExternalDependencies]);

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

  // --- Badge handlers ---
  const updateBadgeGroupName = (index: number, name: string) => {
    setBadgeConfigState((prev) => ({
      groups: prev.groups.map((group, idx) =>
        idx === index ? { ...group, name } : group,
      ),
    }));
    setBadgeConfigMessage(null);
  };

  const updateBadgeOptionDraft = (groupIndex: number, value: string) => {
    setBadgeOptionDrafts((prev) => ({ ...prev, [groupIndex]: value }));
    setBadgeConfigMessage(null);
  };

  const addBadgeGroup = () => {
    setBadgeConfigState((prev) => ({
      groups: [...prev.groups, { name: "", options: [] }],
    }));
    setBadgeConfigMessage(null);
  };

  const removeBadgeGroup = (index: number) => {
    setBadgeConfigState((prev) => ({
      groups: prev.groups.filter((_, idx) => idx !== index),
    }));
    setBadgeOptionDrafts((prev) => {
      const next: Record<number, string> = {};
      Object.entries(prev).forEach(([key, value]) => {
        const draftIndex = Number(key);
        if (draftIndex < index) {
          next[draftIndex] = value;
        } else if (draftIndex > index) {
          next[draftIndex - 1] = value;
        }
      });
      return next;
    });
    setBadgeConfigMessage(null);
  };

  const addBadgeOption = (groupIndex: number, rawValue: string) => {
    const value = rawValue.trim();
    if (!value) return;
    const group = badgeConfig.groups[groupIndex];
    if (group?.options.some((option) => option.trim() === value)) {
      setBadgeConfigMessage("Badge 已存在");
      return;
    }
    setBadgeConfigState((prev) => ({
      groups: prev.groups.map((group, idx) =>
        idx === groupIndex ? { ...group, options: [...group.options, value] } : group,
      ),
    }));
    setBadgeOptionDrafts((prev) => ({ ...prev, [groupIndex]: "" }));
    setBadgeConfigMessage(null);
  };

  const removeBadgeOption = (groupIndex: number, optionIndex: number) => {
    setBadgeConfigState((prev) => ({
      groups: prev.groups.map((group, idx) =>
        idx === groupIndex
          ? { ...group, options: group.options.filter((_, optIdx) => optIdx !== optionIndex) }
          : group,
      ),
    }));
    setBadgeConfigMessage(null);
  };

  const handleBadgeOptionKeyDown = (
    event: React.KeyboardEvent<HTMLInputElement>,
    groupIndex: number,
  ) => {
    if (event.key !== "Enter" || event.nativeEvent.isComposing) return;
    event.preventDefault();
    addBadgeOption(groupIndex, badgeOptionDrafts[groupIndex] ?? "");
  };

  const saveBadgeConfig = async () => {
    setSavingBadgeConfig(true);
    setBadgeConfigMessage(null);
    const normalizedBadgeConfig: BadgeConfig = {
      groups: badgeConfig.groups.map((group) => ({
        name: group.name.trim(),
        options: group.options.map((option) => option.trim()).filter(Boolean),
      })),
    };
    try {
      await setBadgeConfig(normalizedBadgeConfig);
      const latest = await getBadgeConfig();
      setBadgeConfigState(latest);
      setBadgeConfigMessage("已保存 Badge 配置");
    } catch (err) {
      setBadgeConfigMessage(String(err));
    } finally {
      setSavingBadgeConfig(false);
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

  const openExternalLink = async (url: string) => {
    try {
      const { open } = await import("@tauri-apps/plugin-shell");
      await open(url);
    } catch {
      // ignore if shell plugin not available
    }
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

  return (
    <>
      {/* Badge Config */}
      <div className="section badge-config-section">
        <div className="section-header">
          <h3>自定义 Badge</h3>
          <button
            className="btn-small"
            type="button"
            onClick={addBadgeGroup}
          >
            添加分组
          </button>
        </div>
        <p className="section-desc">
          配置后可在发票详情页为单张发票选择标签。每个分组一张发票只能选择一个值。
        </p>

        <div className="badge-config-list">
          {badgeConfig.groups.map((group, groupIndex) => (
            <div className="badge-config-card" key={groupIndex}>
              <div className="badge-config-card-header">
                <label className="form-field">
                  <span>分组名称</span>
                  <input
                    value={group.name}
                    onChange={(e) => updateBadgeGroupName(groupIndex, e.target.value)}
                    placeholder="例如：电商"
                  />
                </label>
                <button
                  className="btn-danger btn-small badge-group-remove"
                  type="button"
                  onClick={() => removeBadgeGroup(groupIndex)}
                  aria-label={`删除分组 ${group.name || groupIndex + 1}`}
                  title="删除分组"
                >
                  删除
                </button>
              </div>
              <div className="badge-option-editor">
                <div className="badge-option-input-row">
                  <input
                    className="badge-option-input"
                    value={badgeOptionDrafts[groupIndex] ?? ""}
                    onChange={(e) => updateBadgeOptionDraft(groupIndex, e.target.value)}
                    onKeyDown={(e) => handleBadgeOptionKeyDown(e, groupIndex)}
                    placeholder="输入 Badge 名称，按 Enter 添加"
                  />
                </div>
                <div className="badge-chip-list">
                  {group.options.map((option, optionIndex) => {
                    const label = option.trim();
                    if (!label) return null;
                    return (
                      <span className="badge-chip" key={`${label}-${optionIndex}`}>
                        <span className="badge-chip-label">{label}</span>
                        <button
                          className="badge-chip-remove"
                          type="button"
                          aria-label={`删除 ${label}`}
                          title="删除"
                          onClick={() => removeBadgeOption(groupIndex, optionIndex)}
                        >
                          ×
                        </button>
                      </span>
                    );
                  })}
                  {group.options.every((option) => !option.trim()) ? (
                    <span className="muted badge-chip-empty">暂无 Badge</span>
                  ) : null}
                </div>
              </div>
            </div>
          ))}
          {badgeConfig.groups.length === 0 ? (
            <p className="muted">暂未配置 Badge 分组。</p>
          ) : null}
        </div>

        <div className="badge-config-actions">
          <button
            className="btn-primary"
            type="button"
            onClick={saveBadgeConfig}
            disabled={savingBadgeConfig}
          >
            {savingBadgeConfig ? "保存中..." : "保存 Badge 配置"}
          </button>
          {badgeConfigMessage ? (
            <span className="badge-config-message">{badgeConfigMessage}</span>
          ) : null}
        </div>
      </div>

      {/* External Dependencies */}
      <div className="section">
        <div className="section-header">
          <h3>外部依赖</h3>
          <button
            className="btn-small"
            type="button"
            onClick={refreshExternalDependencies}
            disabled={checkingDependencies}
          >
            {checkingDependencies ? "检测中..." : "重新检测"}
          </button>
        </div>
        <p className="section-desc">
          PDF 渲染和图片标准化依赖本机命令。Windows 下需要确保 Poppler 和 ImageMagick 已安装并加入 PATH。
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
            </div>
          ))}
          {!checkingDependencies && dependencyStatuses.length === 0 ? (
            <p className="muted">暂未获取依赖状态。</p>
          ) : null}
        </div>
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
            <dt>应用</dt>
            <dd>InvoiceVault : v{APP_CONFIG.version}</dd>
            <dt>数据目录</dt>
            <dd>
              <button
                className="path-link path-button"
                type="button"
                onClick={() => openFolder(health.app_data_dir)}
                title="点击打开文件夹"
              >
                {health.app_data_dir}
              </button>
            </dd>
            <dt>数据库</dt>
            <dd>
              <button
                className="path-link path-button"
                type="button"
                onClick={() => openParentFolder(health.database_path)}
                title="点击打开所在目录"
              >
                {health.database_path}
              </button>
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

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export default AdvancedPage;
