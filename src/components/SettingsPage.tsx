import React from "react";
import type {
  AppHealth,
  LlmConnectionTestResult,
  ChromaConfig,
  EmbeddingConfig,
} from "../types";
import {
  testLlmConnection,
  setChromaConfig,
  getChromaConfig,
  setEmbeddingConfig,
  getEmbeddingConfig,
  testChromaConnection,
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
}: Props) {
  const [llmTestResult, setLlmTestResult] =
    React.useState<LlmConnectionTestResult | null>(null);
  const [isTesting, setIsTesting] = React.useState(false);
  const [testError, setTestError] = React.useState<string | null>(null);

  const [chromaConfig, setChromaConfigState] = React.useState<ChromaConfig>({
    base_url: "http://localhost:8000",
    enabled: false,
  });
  const [chromaTestResult, setChromaTestResult] =
    React.useState<boolean | null>(null);

  const [embConfig, setEmbConfig] = React.useState<EmbeddingConfig>({
    base_url: llmBaseUrl,
    api_key: llmApiKey,
    model: "text-embedding-3-small",
    enabled: false,
  });

  React.useEffect(() => {
    getChromaConfig()
      .then(setChromaConfigState)
      .catch(() => {});
    getEmbeddingConfig()
      .then(setEmbConfig)
      .catch(() => {});
  }, []);

  const handleTest = async () => {
    setIsTesting(true);
    setLlmTestResult(null);
    setTestError(null);
    try {
      const result = await testLlmConnection({
        base_url: llmBaseUrl,
        api_key: llmApiKey,
        model: llmModel,
        timeout_seconds: 30,
      });
      setLlmTestResult(result);
    } catch (err) {
      setTestError(String(err));
    } finally {
      setIsTesting(false);
    }
  };

  const handleSetChromaConfig = async () => {
    try {
      await setChromaConfig(chromaConfig);
    } catch (err) {
      setTestError(String(err));
    }
  };

  const handleSetEmbConfig = async () => {
    try {
      await setEmbeddingConfig(embConfig);
    } catch (err) {
      setTestError(String(err));
    }
  };

  const handleTestChroma = async () => {
    try {
      await setChromaConfig(chromaConfig);
      const ok = await testChromaConnection();
      setChromaTestResult(ok);
    } catch (err) {
      setTestError(String(err));
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

        <button className="btn-primary" onClick={handleTest} disabled={isTesting}>
          {isTesting ? "测试中..." : "测试连接"}
        </button>

        {testError ? (
          <div className="alert alert-error" style={{ marginTop: 12 }}>
            {testError}
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
          配置 OpenAI-compatible Embedding 模型用于语义搜索和去重。默认复用 LLM 的 Base URL 和 API Key。
        </p>

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
              placeholder="text-embedding-3-small"
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

          <label className="form-field">
            <span>启用</span>
            <input
              type="checkbox"
              checked={embConfig.enabled}
              onChange={(e) =>
                setEmbConfig({ ...embConfig, enabled: e.target.checked })
              }
            />
          </label>
        </div>

        <button className="btn-primary" onClick={handleSetEmbConfig}>
          保存 Embedding 配置
        </button>
      </div>

      <div className="section">
        <h3>ChromaDB 连接</h3>
        <p className="section-desc">
          ChromaDB 作为外部向量存储服务运行。使用 Docker 启动：<code>docker run -p 8000:8000 chromadb/chroma</code>
        </p>

        <div className="form-grid">
          <label className="form-field">
            <span>Base URL</span>
            <input
              value={chromaConfig.base_url}
              onChange={(e) =>
                setChromaConfigState({
                  ...chromaConfig,
                  base_url: e.target.value,
                })
              }
              placeholder="http://localhost:8000"
              spellCheck={false}
            />
          </label>

          <label className="form-field">
            <span>启用</span>
            <input
              type="checkbox"
              checked={chromaConfig.enabled}
              onChange={(e) =>
                setChromaConfigState({
                  ...chromaConfig,
                  enabled: e.target.checked,
                })
              }
            />
          </label>
        </div>

        <div className="form-actions" style={{ gap: 10 }}>
          <button className="btn-primary" onClick={handleSetChromaConfig}>
            保存配置
          </button>
          <button className="btn-primary" onClick={handleTestChroma}>
            测试连接
          </button>
        </div>

        {chromaTestResult !== null ? (
          <div className="test-result" style={{ marginTop: 12 }}>
            <div className="test-result-row">
              <span>ChromaDB 状态</span>
              <strong>
                {chromaTestResult ? "已连接" : "无法连接"}
              </strong>
            </div>
          </div>
        ) : null}
      </div>

      {health ? (
        <div className="section">
          <h3>系统信息</h3>
          <dl className="info-grid">
            <dt>数据目录</dt>
            <dd>{health.app_data_dir}</dd>
            <dt>数据库</dt>
            <dd>{health.database_path}</dd>
            <dt>迁移版本</dt>
            <dd>{health.migration_version}</dd>
          </dl>
        </div>
      ) : null}

      <WatchDirManager />
    </div>
  );
}
