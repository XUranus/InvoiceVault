import React from "react";
import type {
  LlmConnectionTestResult,
  ChromaConfig,
  EmbeddingConfig,
  EmbeddingTestResult,
  ExternalDependencyStatus,
  BadgeConfig,
} from "../types";
import {
  testLlmConnection,
  setChromaConfig,
  getChromaConfig,
  setEmbeddingConfig,
  getEmbeddingConfig,
  testEmbeddingConnection,
  setLlmConfig,
  getRecognitionQueueStatus,
  setRecognitionConcurrency,
  exportLogs,
  exportBackup,
  cleanupStorage,
  checkExternalDependencies,
  getBadgeConfig,
  setBadgeConfig,
} from "../api";
import type { ExportLogsResult, CleanupStorageResult } from "../types";
import { WatchDirManager } from "./WatchDirManager";
import { ConfirmDialog } from "./ConfirmDialog";
import { useAppStore } from "../stores/appStore";
import { useLlmStore } from "../stores/llmStore";

export function SettingsPage() {
  const health = useAppStore((s) => s.health);
  const error = useAppStore((s) => s.error);
  const {
    llmBaseUrl,
    llmModel,
    llmApiKey,
    setLlmBaseUrl,
    setLlmModel,
    setLlmApiKey,
  } = useLlmStore();
  const theme = useAppStore((s) => s.theme);
  const toggleTheme = useAppStore((s) => s.toggleTheme);
  const [llmTestResult, setLlmTestResult] =
    React.useState<LlmConnectionTestResult | null>(null);
  const [isTestingLlm, setIsTestingLlm] = React.useState(false);
  const [llmTestError, setLlmTestError] = React.useState<string | null>(null);

  const [chromaConfig, setChromaConfigState] = React.useState<ChromaConfig>({
    enabled: true,
  });

  const [embConfig, setEmbConfig] = React.useState<EmbeddingConfig>({
    base_url: llmBaseUrl,
    api_key: llmApiKey,
    model: "text-embedding-v4",
    enabled: true,
  });

  const [embTestResult, setEmbTestResult] =
    React.useState<EmbeddingTestResult | null>(null);
  const [isTestingEmb, setIsTestingEmb] = React.useState(false);
  const [embTestError, setEmbTestError] = React.useState<string | null>(null);

  const [recognitionConcurrency, setRecognitionConcurrencyState] = React.useState(3);

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
  const [dependencyStatuses, setDependencyStatuses] = React.useState<ExternalDependencyStatus[]>([]);
  const [checkingDependencies, setCheckingDependencies] = React.useState(false);
  const [badgeConfig, setBadgeConfigState] = React.useState<BadgeConfig>({ groups: [] });
  const [savingBadgeConfig, setSavingBadgeConfig] = React.useState(false);
  const [badgeConfigMessage, setBadgeConfigMessage] = React.useState<string | null>(null);

  // Gate all auto-save effects until async config load completes.
  // Without this, the initial render fires auto-save with default state
  // before the backend values are fetched, creating spurious config_change events.
  const configLoaded = React.useRef(false);
  const lastSavedChroma = React.useRef<string>("");
  const lastSavedEmb = React.useRef<string>("");

  React.useEffect(() => {
    let cancelled = false;
    Promise.all([
      getChromaConfig()
        .then((cfg) => {
          if (cancelled) return;
          const val = { enabled: cfg.enabled !== false };
          setChromaConfigState(val);
          lastSavedChroma.current = JSON.stringify(val);
        })
        .catch(() => {}),
      getEmbeddingConfig()
        .then((cfg) => {
          if (cancelled) return;
          const val = { ...cfg, enabled: cfg.enabled !== false };
          setEmbConfig(val);
          lastSavedEmb.current = JSON.stringify(val);
        })
        .catch(() => {}),
      getRecognitionQueueStatus()
        .then((status) => {
          if (!cancelled) setRecognitionConcurrencyState(status.max_concurrent);
        })
        .catch(() => {}),
      getBadgeConfig()
        .then((cfg) => {
          if (!cancelled) setBadgeConfigState(cfg);
        })
        .catch(() => {}),
    ]).finally(() => {
      if (!cancelled) configLoaded.current = true;
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

  // Auto-save LLM config on change
  React.useEffect(() => {
    if (!configLoaded.current) return;
    const timer = setTimeout(() => {
      setLlmConfig({
        base_url: llmBaseUrl,
        api_key: llmApiKey,
        model: llmModel,
      }).catch(() => {});
    }, 600);
    return () => clearTimeout(timer);
  }, [llmBaseUrl, llmModel, llmApiKey]);

  // Auto-save chroma config on change
  React.useEffect(() => {
    if (!configLoaded.current) return;
    const current = JSON.stringify(chromaConfig);
    if (current === lastSavedChroma.current) return;
    lastSavedChroma.current = current;
    setChromaConfig(chromaConfig).catch(() => {});
  }, [chromaConfig]);

  // Auto-save embedding config on change
  React.useEffect(() => {
    if (!configLoaded.current) return;
    const current = JSON.stringify(embConfig);
    if (current === lastSavedEmb.current) return;
    lastSavedEmb.current = current;
    setEmbeddingConfig(embConfig).catch(() => {});
  }, [embConfig]);

  // Test embedding when config changes (debounced)
  React.useEffect(() => {
    if (!configLoaded.current) return;
    const timer = setTimeout(async () => {
      try {
        await testEmbeddingConnection();
        setEmbTestError(null);
      } catch {
        setEmbTestError("Embedding 服务连接失败，语义搜索和去重功能暂时不可用");
      }
    }, 600);
    return () => clearTimeout(timer);
  }, [embConfig.base_url, embConfig.api_key, embConfig.model]);

  const handleTestLlm = async () => {
    setIsTestingLlm(true);
    setLlmTestResult(null);
    setLlmTestError(null);
    try {
      const result = await testLlmConnection({
        base_url: llmBaseUrl,
        api_key: llmApiKey,
        model: llmModel,
        timeout_seconds: 30,
      });
      setLlmTestResult(result);
    } catch (err) {
      setLlmTestError(String(err));
    } finally {
      setIsTestingLlm(false);
    }
  };

  const handleTestEmbedding = async () => {
    setIsTestingEmb(true);
    setEmbTestResult(null);
    setEmbTestError(null);
    try {
      const result = await testEmbeddingConnection();
      setEmbTestResult(result);
    } catch (err) {
      setEmbTestError(String(err));
    } finally {
      setIsTestingEmb(false);
    }
  };

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

  const openFolder = async (path: string) => {
    try {
      const { open } = await import("@tauri-apps/plugin-shell");
      await open(path);
    } catch {
      // ignore if shell plugin not available
    }
  };

  const updateBadgeGroupName = (index: number, name: string) => {
    setBadgeConfigState((prev) => ({
      groups: prev.groups.map((group, idx) =>
        idx === index ? { ...group, name } : group,
      ),
    }));
    setBadgeConfigMessage(null);
  };

  const updateBadgeOption = (groupIndex: number, optionIndex: number, value: string) => {
    setBadgeConfigState((prev) => ({
      groups: prev.groups.map((group, idx) =>
        idx === groupIndex
          ? {
              ...group,
              options: group.options.map((option, optIdx) =>
                optIdx === optionIndex ? value : option,
              ),
            }
          : group,
      ),
    }));
    setBadgeConfigMessage(null);
  };

  const addBadgeGroup = () => {
    setBadgeConfigState((prev) => ({
      groups: [...prev.groups, { name: "", options: [""] }],
    }));
    setBadgeConfigMessage(null);
  };

  const removeBadgeGroup = (index: number) => {
    setBadgeConfigState((prev) => ({
      groups: prev.groups.filter((_, idx) => idx !== index),
    }));
    setBadgeConfigMessage(null);
  };

  const addBadgeOption = (groupIndex: number) => {
    setBadgeConfigState((prev) => ({
      groups: prev.groups.map((group, idx) =>
        idx === groupIndex ? { ...group, options: [...group.options, ""] } : group,
      ),
    }));
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

  const saveBadgeConfig = async () => {
    setSavingBadgeConfig(true);
    setBadgeConfigMessage(null);
    try {
      await setBadgeConfig(badgeConfig);
      const latest = await getBadgeConfig();
      setBadgeConfigState(latest);
      setBadgeConfigMessage("已保存 Badge 配置");
    } catch (err) {
      setBadgeConfigMessage(String(err));
    } finally {
      setSavingBadgeConfig(false);
    }
  };

  return (
    <div className="page settings-page">
      <h2 className="page-title">设置</h2>

      {error ? <div className="alert alert-error">{error}</div> : null}

      <div className="section">
        <h3>LLM Provider</h3>
        <p className="section-desc">
          配置 OpenAI-compatible 多模态模型用于发票识别。API Key 仅保存在当前会话中。
        </p>

        <div className="form-grid">
          <label className="form-field">
            <span>Base URL</span>
            <input
              value={llmBaseUrl}
              onChange={(e) => setLlmBaseUrl(e.target.value)}
              placeholder="https://api.openai.com/v1"
              spellCheck={false}
            />
          </label>

          <label className="form-field">
            <span>Model</span>
            <input
              value={llmModel}
              onChange={(e) => setLlmModel(e.target.value)}
              placeholder="qwen3.6-plus"
              spellCheck={false}
            />
          </label>

          <label className="form-field">
            <span>API Key</span>
            <input
              value={llmApiKey}
              onChange={(e) => setLlmApiKey(e.target.value)}
              type="password"
              placeholder="sk-..."
              spellCheck={false}
            />
          </label>
        </div>

        <button className="btn-primary" onClick={handleTestLlm} disabled={isTestingLlm}>
          {isTestingLlm ? "测试中..." : "测试连接"}
        </button>

        {llmTestError ? (
          <div className="alert alert-error" style={{ marginTop: 12 }}>
            {llmTestError}
          </div>
        ) : null}

        {llmTestResult ? (
          <div className="test-result">
            <div className="test-result-row">
              <span>模型</span>
              <strong>{llmTestResult.model}</strong>
            </div>
            <div className="test-result-row">
              <span>延迟</span>
              <strong>{llmTestResult.duration_ms} ms</strong>
            </div>
            <div className="test-result-row">
              <span>响应</span>
              <strong className="mono">{llmTestResult.response_preview}</strong>
            </div>
          </div>
        ) : null}
      </div>

      <div className="section">
        <h3>Embedding Provider</h3>
        <p className="section-desc">
          配置 OpenAI-compatible Embedding 模型用于语义搜索和去重。修改后自动保存。
        </p>

        {embTestError ? (
          <div className="alert alert-warn" style={{ marginBottom: 12 }}>
            {embTestError}
          </div>
        ) : null}

        <div className="form-grid">
          <label className="form-field">
            <span>Base URL</span>
            <input
              value={embConfig.base_url}
              onChange={(e) =>
                setEmbConfig({ ...embConfig, base_url: e.target.value })
              }
              placeholder={llmBaseUrl || "https://api.openai.com/v1"}
              spellCheck={false}
            />
          </label>

          <label className="form-field">
            <span>Model</span>
            <input
              value={embConfig.model}
              onChange={(e) =>
                setEmbConfig({ ...embConfig, model: e.target.value })
              }
              placeholder="text-embedding-v4"
              spellCheck={false}
            />
          </label>

          <label className="form-field">
            <span>API Key</span>
            <input
              value={embConfig.api_key}
              onChange={(e) =>
                setEmbConfig({ ...embConfig, api_key: e.target.value })
              }
              type="password"
              placeholder={llmApiKey ? "(复用 LLM API Key)" : "sk-..."}
              spellCheck={false}
            />
          </label>
        </div>

        <button className="btn-primary" onClick={handleTestEmbedding} disabled={isTestingEmb}>
          {isTestingEmb ? "测试中..." : "测试 Embedding 连接"}
        </button>

        {embTestResult ? (
          <div className="test-result" style={{ marginTop: 12 }}>
            <div className="test-result-row">
              <span>模型</span>
              <strong>{embTestResult.model}</strong>
            </div>
            <div className="test-result-row">
              <span>维度</span>
              <strong>{embTestResult.dimensions}</strong>
            </div>
            <div className="test-result-row">
              <span>延迟</span>
              <strong>{embTestResult.duration_ms} ms</strong>
            </div>
          </div>
        ) : null}
      </div>

      <div className="section">
        <h3>识别任务</h3>
        <p className="section-desc">
          设置同时进行的发票识别任务数量。导入文件后会自动开始识别。
        </p>
        <div className="form-field">
          <span>最大并发数 (1-10)</span>
          <input
            type="number"
            min={1}
            max={10}
            value={recognitionConcurrency}
            onChange={(e) => {
              const v = Math.max(1, Math.min(10, Number(e.target.value) || 1));
              setRecognitionConcurrencyState(v);
              setRecognitionConcurrency(v).catch(() => {});
            }}
          />
        </div>
      </div>

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

      <div className="section">
        <h3>外观</h3>
        <p className="section-desc">
          当前: {theme === "dark" ? "暗色主题" : "亮色主题"}
        </p>
        <button className="btn-primary" onClick={toggleTheme}>
          {theme === "dark" ? "☀️ 切换到亮色主题" : "🌙 切换到暗色主题"}
        </button>
      </div>

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
                  className="btn-danger btn-small"
                  type="button"
                  onClick={() => removeBadgeGroup(groupIndex)}
                >
                  删除分组
                </button>
              </div>
              <div className="badge-option-editor">
                {group.options.map((option, optionIndex) => (
                  <div className="badge-option-row" key={optionIndex}>
                    <input
                      value={option}
                      onChange={(e) =>
                        updateBadgeOption(groupIndex, optionIndex, e.target.value)
                      }
                      placeholder="例如：京东"
                    />
                    <button
                      className="btn-small"
                      type="button"
                      onClick={() => removeBadgeOption(groupIndex, optionIndex)}
                    >
                      删除
                    </button>
                  </div>
                ))}
                <button
                  className="btn-small"
                  type="button"
                  onClick={() => addBadgeOption(groupIndex)}
                >
                  添加选项
                </button>
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

      {health ? (
        <div className="section">
          <h3>系统信息</h3>
          <dl className="info-grid">
            <dt>数据目录</dt>
            <dd>
              <a
                className="path-link"
                onClick={() => openFolder(health.app_data_dir)}
                title="点击打开文件夹"
              >
                {health.app_data_dir}
              </a>
            </dd>
            <dt>数据库</dt>
            <dd>
              <a
                className="path-link"
                onClick={() => openFolder(health.database_path)}
                title="点击打开文件夹"
              >
                {health.database_path}
              </a>
            </dd>
            <dt>迁移版本</dt>
            <dd>{health.migration_version}</dd>
          </dl>
        </div>
      ) : null}

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

      <WatchDirManager />
    </div>
  );
}

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export default SettingsPage;
