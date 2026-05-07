import React from "react";
import type {
  LlmConnectionTestResult,
  EmbeddingConfig,
  EmbeddingTestResult,
} from "../../types";
import {
  testLlmConnection,
  setLlmConfig,
  setAgentLlmConfig,
  setEmbeddingConfig,
  getEmbeddingConfig,
  testEmbeddingConnection,
  setLlmAuditEnabled as apiSetLlmAuditEnabled,
} from "../../api";
import { useLlmStore } from "../../stores/llmStore";

export function AiProviderPage() {
  // --- OCR Panel ---
  const ocr = useLlmStore((s) => s.ocr);
  const setOcrField = useLlmStore((s) => s.setOcrField);
  const resetOcr = useLlmStore((s) => s.resetOcr);
  const markOcrTestPassed = useLlmStore((s) => s.markOcrTestPassed);
  const [ocrTestResult, setOcrTestResult] = React.useState<LlmConnectionTestResult | null>(null);
  const [isTestingOcr, setIsTestingOcr] = React.useState(false);
  const [ocrTestError, setOcrTestError] = React.useState<string | null>(null);
  const [ocrSaveMsg, setOcrSaveMsg] = React.useState<string | null>(null);
  const [savingOcr, setSavingOcr] = React.useState(false);

  // --- Agent Panel ---
  const agent = useLlmStore((s) => s.agent);
  const setAgentField = useLlmStore((s) => s.setAgentField);
  const resetAgent = useLlmStore((s) => s.resetAgent);
  const markAgentTestPassed = useLlmStore((s) => s.markAgentTestPassed);
  const [agentTestResult, setAgentTestResult] = React.useState<LlmConnectionTestResult | null>(null);
  const [isTestingAgent, setIsTestingAgent] = React.useState(false);
  const [agentTestError, setAgentTestError] = React.useState<string | null>(null);
  const [agentSaveMsg, setAgentSaveMsg] = React.useState<string | null>(null);
  const [savingAgent, setSavingAgent] = React.useState(false);

  // --- Embedding Panel ---
  const [embConfig, setEmbConfig] = React.useState<EmbeddingConfig>({
    base_url: "",
    api_key: "",
    model: "text-embedding-v4",
    enabled: true,
  });
  const [embDirty, setEmbDirty] = React.useState(false);
  const [embTestPassed, setEmbTestPassed] = React.useState(false);
  const [embTestResult, setEmbTestResult] = React.useState<EmbeddingTestResult | null>(null);
  const [isTestingEmb, setIsTestingEmb] = React.useState(false);
  const [embTestError, setEmbTestError] = React.useState<string | null>(null);
  const [embSaveMsg, setEmbSaveMsg] = React.useState<string | null>(null);
  const [savingEmb, setSavingEmb] = React.useState(false);

  // --- Audit ---
  const auditEnabled = useLlmStore((s) => s.auditEnabled);
  const setAuditEnabled = useLlmStore((s) => s.setAuditEnabled);

  // Load embedding config on mount
  React.useEffect(() => {
    let cancelled = false;
    getEmbeddingConfig()
      .then((emb) => {
        if (cancelled || !emb) return;
        setEmbConfig({ ...emb, enabled: emb.enabled !== false });
      })
      .catch(() => {});
    return () => { cancelled = true; };
  }, []);

  // --- OCR handlers ---
  const handleTestOcr = async () => {
    setIsTestingOcr(true);
    setOcrTestResult(null);
    setOcrTestError(null);
    markOcrTestPassed(false);
    try {
      const result = await testLlmConnection({
        base_url: ocr.config.baseUrl,
        api_key: ocr.config.apiKey,
        model: ocr.config.model,
        timeout_seconds: 30,
      });
      setOcrTestResult(result);
      markOcrTestPassed(true);
    } catch (err) {
      setOcrTestError(String(err));
    } finally {
      setIsTestingOcr(false);
    }
  };

  const handleSaveOcr = async () => {
    setSavingOcr(true);
    setOcrSaveMsg(null);
    try {
      await setLlmConfig({
        base_url: ocr.config.baseUrl,
        api_key: ocr.config.apiKey,
        model: ocr.config.model,
      });
      resetOcr(ocr.config);
      setOcrSaveMsg("已保存 OCR 配置");
    } catch (err) {
      setOcrSaveMsg(String(err));
    } finally {
      setSavingOcr(false);
    }
  };

  // --- Agent handlers ---
  const handleTestAgent = async () => {
    setIsTestingAgent(true);
    setAgentTestResult(null);
    setAgentTestError(null);
    markAgentTestPassed(false);
    try {
      const result = await testLlmConnection({
        base_url: agent.config.baseUrl,
        api_key: agent.config.apiKey,
        model: agent.config.model,
        timeout_seconds: 30,
      });
      setAgentTestResult(result);
      markAgentTestPassed(true);
    } catch (err) {
      setAgentTestError(String(err));
    } finally {
      setIsTestingAgent(false);
    }
  };

  const handleSaveAgent = async () => {
    setSavingAgent(true);
    setAgentSaveMsg(null);
    try {
      await setAgentLlmConfig({
        base_url: agent.config.baseUrl,
        api_key: agent.config.apiKey,
        model: agent.config.model,
      });
      resetAgent(agent.config);
      setAgentSaveMsg("已保存 Agent 配置");
    } catch (err) {
      setAgentSaveMsg(String(err));
    } finally {
      setSavingAgent(false);
    }
  };

  // --- Embedding handlers ---
  const handleEmbFieldChange = (field: keyof EmbeddingConfig, value: string | boolean) => {
    setEmbConfig((prev) => ({ ...prev, [field]: value }));
    setEmbDirty(true);
    setEmbTestPassed(false);
    setEmbSaveMsg(null);
  };

  const handleTestEmbedding = async () => {
    setIsTestingEmb(true);
    setEmbTestResult(null);
    setEmbTestError(null);
    setEmbTestPassed(false);
    try {
      await setEmbeddingConfig(embConfig);
      const result = await testEmbeddingConnection();
      setEmbTestResult(result);
      setEmbTestPassed(true);
    } catch (err) {
      setEmbTestError(String(err));
    } finally {
      setIsTestingEmb(false);
    }
  };

  const handleSaveEmbedding = async () => {
    setSavingEmb(true);
    setEmbSaveMsg(null);
    try {
      await setEmbeddingConfig(embConfig);
      setEmbDirty(false);
      setEmbSaveMsg("已保存 Embedding 配置");
    } catch (err) {
      setEmbSaveMsg(String(err));
    } finally {
      setSavingEmb(false);
    }
  };

  // --- Audit handler ---
  const handleAuditToggle = async (checked: boolean) => {
    setAuditEnabled(checked);
    try {
      await apiSetLlmAuditEnabled(checked);
    } catch {
      // ignore
    }
  };

  return (
    <>
      {/* OCR Provider */}
      <div className="section">
        <h3>OCR 识别 Provider（多模态）</h3>
        <p className="section-desc">
          配置用于发票图片识别的多模态模型（Vision API）。
        </p>

        <div className="form-grid">
          <label className="form-field">
            <span>Base URL</span>
            <input
              value={ocr.config.baseUrl}
              onChange={(e) => setOcrField("baseUrl", e.target.value)}
              placeholder="https://api.openai.com/v1"
              spellCheck={false}
            />
          </label>
          <label className="form-field">
            <span>Model</span>
            <input
              value={ocr.config.model}
              onChange={(e) => setOcrField("model", e.target.value)}
              placeholder="qwen-vl-max"
              spellCheck={false}
            />
          </label>
          <label className="form-field">
            <span>API Key</span>
            <input
              value={ocr.config.apiKey}
              onChange={(e) => setOcrField("apiKey", e.target.value)}
              type="password"
              placeholder="sk-..."
              spellCheck={false}
            />
          </label>
        </div>

        <div className="provider-actions">
          <button className="btn-primary" onClick={handleTestOcr} disabled={isTestingOcr}>
            {isTestingOcr ? "测试中..." : "测试连接"}
          </button>
          <button
            className="btn-primary"
            onClick={handleSaveOcr}
            disabled={savingOcr || !ocr.dirty || !ocr.testPassed}
          >
            {savingOcr ? "保存中..." : "保存设置"}
          </button>
          {ocrSaveMsg ? <span className="badge-config-message">{ocrSaveMsg}</span> : null}
        </div>

        {ocrTestError ? (
          <div className="alert alert-error" style={{ marginTop: 12 }}>
            {ocrTestError}
          </div>
        ) : null}

        {ocrTestResult ? (
          <div className="test-result">
            <div className="test-result-row">
              <span>模型</span>
              <strong>{ocrTestResult.model}</strong>
            </div>
            <div className="test-result-row">
              <span>延迟</span>
              <strong>{ocrTestResult.duration_ms} ms</strong>
            </div>
            <div className="test-result-row">
              <span>响应</span>
              <strong className="mono">{ocrTestResult.response_preview}</strong>
            </div>
          </div>
        ) : null}
      </div>

      {/* Agent LLM Provider */}
      <div className="section">
        <h3>Agent LLM Provider（文本）</h3>
        <p className="section-desc">
          配置用于 AI 助手对话的文本模型。
        </p>

        <div className="form-grid">
          <label className="form-field">
            <span>Base URL</span>
            <input
              value={agent.config.baseUrl}
              onChange={(e) => setAgentField("baseUrl", e.target.value)}
              placeholder="https://api.openai.com/v1"
              spellCheck={false}
            />
          </label>
          <label className="form-field">
            <span>Model</span>
            <input
              value={agent.config.model}
              onChange={(e) => setAgentField("model", e.target.value)}
              placeholder="qwen3.6-plus"
              spellCheck={false}
            />
          </label>
          <label className="form-field">
            <span>API Key</span>
            <input
              value={agent.config.apiKey}
              onChange={(e) => setAgentField("apiKey", e.target.value)}
              type="password"
              placeholder="sk-..."
              spellCheck={false}
            />
          </label>
        </div>

        <div className="provider-actions">
          <button className="btn-primary" onClick={handleTestAgent} disabled={isTestingAgent}>
            {isTestingAgent ? "测试中..." : "测试连接"}
          </button>
          <button
            className="btn-primary"
            onClick={handleSaveAgent}
            disabled={savingAgent || !agent.dirty || !agent.testPassed}
          >
            {savingAgent ? "保存中..." : "保存设置"}
          </button>
          {agentSaveMsg ? <span className="badge-config-message">{agentSaveMsg}</span> : null}
        </div>

        {agentTestError ? (
          <div className="alert alert-error" style={{ marginTop: 12 }}>
            {agentTestError}
          </div>
        ) : null}

        {agentTestResult ? (
          <div className="test-result">
            <div className="test-result-row">
              <span>模型</span>
              <strong>{agentTestResult.model}</strong>
            </div>
            <div className="test-result-row">
              <span>延迟</span>
              <strong>{agentTestResult.duration_ms} ms</strong>
            </div>
            <div className="test-result-row">
              <span>响应</span>
              <strong className="mono">{agentTestResult.response_preview}</strong>
            </div>
          </div>
        ) : null}
      </div>

      {/* Embedding Provider */}
      <div className="section">
        <h3>Embedding Provider</h3>
        <p className="section-desc">
          配置用于语义搜索和去重的 Embedding 模型。
        </p>

        <div className="form-grid">
          <label className="form-field">
            <span>Base URL</span>
            <input
              value={embConfig.base_url}
              onChange={(e) => handleEmbFieldChange("base_url", e.target.value)}
              placeholder="https://api.openai.com/v1"
              spellCheck={false}
            />
          </label>
          <label className="form-field">
            <span>Model</span>
            <input
              value={embConfig.model}
              onChange={(e) => handleEmbFieldChange("model", e.target.value)}
              placeholder="text-embedding-v4"
              spellCheck={false}
            />
          </label>
          <label className="form-field">
            <span>API Key</span>
            <input
              value={embConfig.api_key}
              onChange={(e) => handleEmbFieldChange("api_key", e.target.value)}
              type="password"
              placeholder="sk-..."
              spellCheck={false}
            />
          </label>
        </div>

        <div className="provider-actions">
          <button className="btn-primary" onClick={handleTestEmbedding} disabled={isTestingEmb}>
            {isTestingEmb ? "测试中..." : "测试连接"}
          </button>
          <button
            className="btn-primary"
            onClick={handleSaveEmbedding}
            disabled={savingEmb || !embDirty || !embTestPassed}
          >
            {savingEmb ? "保存中..." : "保存设置"}
          </button>
          {embSaveMsg ? <span className="badge-config-message">{embSaveMsg}</span> : null}
        </div>

        {embTestError ? (
          <div className="alert alert-error" style={{ marginTop: 12 }}>
            {embTestError}
          </div>
        ) : null}

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

      {/* LLM Audit */}
      <div className="section">
        <h3>LLM 审计日志</h3>
        <label className="settings-toggle-card">
          <input
            type="checkbox"
            checked={auditEnabled}
            onChange={(e) => handleAuditToggle(e.target.checked)}
          />
          <span>
            <strong>开启审计日志</strong>
            <small>
              归档所有 LLM Provider 的 request / response，JSONL 文件保存到 app data 下的 llm_audit 目录。
            </small>
          </span>
        </label>
      </div>
    </>
  );
}

export default AiProviderPage;
