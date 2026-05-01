import React from "react";
import type { AppHealth, LlmConnectionTestResult } from "../types";
import { testLlmConnection } from "../api";

type Props = {
  health: AppHealth | null;
  error: string | null;
  llmBaseUrl: string;
  llmModel: string;
  llmApiKey: string;
  onBaseUrlChange: (value: string) => void;
  onModelChange: (value: string) => void;
  onApiKeyChange: (value: string) => void;
};

export function StatusPanel({
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
  const [isTestingLlm, setIsTestingLlm] = React.useState(false);
  const [testError, setTestError] = React.useState<string | null>(null);

  const handleTestLlm = async () => {
    setIsTestingLlm(true);
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
      setIsTestingLlm(false);
    }
  };

  return (
    <>
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
      <div className="settings-block">
        <h2>LLM Provider</h2>
        <label>
          Base URL
          <input
            value={llmBaseUrl}
            onChange={(event) => onBaseUrlChange(event.target.value)}
            spellCheck={false}
          />
        </label>
        <label>
          Model
          <input
            value={llmModel}
            onChange={(event) => onModelChange(event.target.value)}
            spellCheck={false}
          />
        </label>
        <label>
          API Key
          <input
            value={llmApiKey}
            onChange={(event) => onApiKeyChange(event.target.value)}
            type="password"
            spellCheck={false}
          />
        </label>
        <button type="button" onClick={handleTestLlm} disabled={isTestingLlm}>
          {isTestingLlm ? "测试中" : "测试连接"}
        </button>
        {testError ? <pre className="error-box">{testError}</pre> : null}
        {llmTestResult ? (
          <dl className="llm-result">
            <dt>模型</dt>
            <dd>{llmTestResult.model}</dd>
            <dt>耗时</dt>
            <dd>{llmTestResult.duration_ms} ms</dd>
            <dt>响应</dt>
            <dd>{llmTestResult.response_preview}</dd>
          </dl>
        ) : null}
      </div>
    </>
  );
}
