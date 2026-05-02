import React from "react";
import type {
  AppHealth,
  LlmConnectionTestResult,
  ChromaConfig,
  EmbeddingConfig,
  EmbeddingTestResult,
} from "../types";
import {
  testLlmConnection,
  setChromaConfig,
  getChromaConfig,
  setEmbeddingConfig,
  getEmbeddingConfig,
  testEmbeddingConnection,
} from "../api";
import { WatchDirManager } from "./WatchDirManager";

type Props = {
  health: AppHealth | null;
  error: string | null;
  llmBaseUrl: string;
  llmModel: string;
  llmApiKey: string;
  onBaseUrlChange: (v: string) => void;
  onModelChange: (v: string) => void;
  onApiKeyChange: (v: string) => void;
  theme: "light" | "dark";
  onToggleTheme: () => void;
};

export function SettingsPage({
  health,
  error,
  llmBaseUrl,
  llmModel,
  llmApiKey,
  onBaseUrlChange,
  onModelChange,
  onApiKeyChange,
  theme,
  onToggleTheme,
}: Props) {
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

  React.useEffect(() => {
    getChromaConfig()
      .then((cfg) => setChromaConfigState({ enabled: cfg.enabled !== false }))
      .catch(() => {});
    getEmbeddingConfig()
      .then((cfg) => setEmbConfig({ ...cfg, enabled: cfg.enabled !== false }))
      .catch(() => {});
  }, []);

  // Auto-save chroma config on change
  React.useEffect(() => {
    setChromaConfig(chromaConfig).catch(() => {});
  }, [chromaConfig]);

  // Auto-save embedding config on change
  React.useEffect(() => {
    setEmbeddingConfig(embConfig).catch(() => {});
  }, [embConfig]);

  // Test embedding on mount and when config changes (debounced)
  React.useEffect(() => {
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

  const openFolder = async (path: string) => {
    try {
      const { open } = await import("@tauri-apps/plugin-shell");
      await open(path);
    } catch {
      // ignore if shell plugin not available
    }
  };

  return (
    <div className="page">
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
              onChange={(e) => onBaseUrlChange(e.target.value)}
              placeholder="https://api.openai.com/v1"
              spellCheck={false}
            />
          </label>

          <label className="form-field">
            <span>Model</span>
            <input
              value={llmModel}
              onChange={(e) => onModelChange(e.target.value)}
              placeholder="qwen3.6-plus"
              spellCheck={false}
            />
          </label>

          <label className="form-field">
            <span>API Key</span>
            <input
              value={llmApiKey}
              onChange={(e) => onApiKeyChange(e.target.value)}
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
        <h3>外观</h3>
        <p className="section-desc">
          当前: {theme === "dark" ? "暗色主题" : "亮色主题"}
        </p>
        <button className="btn-primary" onClick={onToggleTheme}>
          {theme === "dark" ? "☀️ 切换到亮色主题" : "🌙 切换到暗色主题"}
        </button>
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

      <WatchDirManager />
    </div>
  );
}
