import React from "react";
import type { AppHealth, LlmConnectionTestResult } from "../types";
import { testLlmConnection } from "../api";

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
    </div>
  );
}
