import React from "react";
import { Eye, EyeOff } from "lucide-react";
import type {
  LlmConnectionTestResult,
  LocalEmbeddingStatus,
  EmbeddingTestResult,
} from "../../types";
import {
  testLlmConnection,
  setLlmConfig,
  setEmbeddingEnabled,
  getEmbeddingStatus,
  downloadEmbeddingModel,
  testEmbeddingConnection,
  regenerateAllEmbeddings,
  setLlmAuditEnabled as apiSetLlmAuditEnabled,
} from "../../api";
import { useLlmStore } from "../../stores/llmStore";
import { LlmDiagnosticDialog } from "../LlmDiagnosticDialog";

const LLM_PRESETS = [
    {
        label: "MiMo",
        baseUrl: "https://api.xiaomimimo.com/v1",
        model: "mimo-v2.5",
        color: "orange" as const,
    },
    {
        label: "百炼",
        baseUrl: "https://dashscope.aliyuncs.com/apps/v1",
        model: "qwen3.6-plus",
        color: "blue" as const,
    },
    {
        label: "Kimi",
        baseUrl: "https://api.moonshot.cn/v1",
        model: "kimi-k2.6",
        color: "black" as const,
    },
    {
        label: "GLM",
        baseUrl: "https://open.bigmodel.cn/api/paas/v4",
        model: "glm-4.6v",
        color: "red" as const,
    }
];

export function AiProviderPage() {
  // --- LLM Panel ---
  const llm = useLlmStore((s) => s.llm);
  const setLlmField = useLlmStore((s) => s.setLlmField);
  const resetLlm = useLlmStore((s) => s.resetLlm);
  const markLlmTestPassed = useLlmStore((s) => s.markLlmTestPassed);
  const [llmTestResult, setLlmTestResult] = React.useState<LlmConnectionTestResult | null>(null);
  const [isTestingLlm, setIsTestingLlm] = React.useState(false);
  const [llmTestError, setLlmTestError] = React.useState<string | null>(null);
  const [llmSaveMsg, setLlmSaveMsg] = React.useState<string | null>(null);
  const [savingLlm, setSavingLlm] = React.useState(false);
  const [showApiKey, setShowApiKey] = React.useState(false);

  // --- SCNet OCR ---
  const scnetApiKey = useLlmStore((s) => s.scnetApiKey);
  const setScnetApiKey = useLlmStore((s) => s.setScnetApiKey);
  const [showScnetKey, setShowScnetKey] = React.useState(false);
  const [scnetSaveMsg, setScnetSaveMsg] = React.useState<string | null>(null);
  const [savingScnet, setSavingScnet] = React.useState(false);

  // --- Embedding Panel (local model) ---
  const [embStatus, setEmbStatus] = React.useState<LocalEmbeddingStatus>({
    enabled: true,
    model_loaded: false,
    model_dir: null,
    dimensions: null,
  });
  const [embTestResult, setEmbTestResult] = React.useState<EmbeddingTestResult | null>(null);
  const [isTestingEmb, setIsTestingEmb] = React.useState(false);
  const [embTestError, setEmbTestError] = React.useState<string | null>(null);
  const [isDownloadingEmb, setIsDownloadingEmb] = React.useState(false);
  const [embDownloadMsg, setEmbDownloadMsg] = React.useState<string | null>(null);
  const [isRegeneratingEmb, setIsRegeneratingEmb] = React.useState(false);
  const [embRegenMsg, setEmbRegenMsg] = React.useState<string | null>(null);

  // --- Audit ---
  const auditEnabled = useLlmStore((s) => s.auditEnabled);
  const setAuditEnabled = useLlmStore((s) => s.setAuditEnabled);

  // --- Diagnostic ---
  const [showDiagnostic, setShowDiagnostic] = React.useState(false);

  // Load embedding status on mount
  React.useEffect(() => {
    let cancelled = false;
    getEmbeddingStatus()
      .then((status) => {
        if (cancelled || !status) return;
        setEmbStatus(status);
      })
      .catch(() => {});
    return () => { cancelled = true; };
  }, []);

  // --- LLM handlers ---
  const handleTestLlm = async () => {
    setIsTestingLlm(true);
    setLlmTestResult(null);
    setLlmTestError(null);
    markLlmTestPassed(false);
    try {
      const result = await testLlmConnection({
        base_url: llm.config.baseUrl,
        api_key: llm.config.apiKey,
        model: llm.config.model,
        timeout_seconds: 30,
      });
      setLlmTestResult(result);
      markLlmTestPassed(true);
    } catch (err) {
      setLlmTestError(String(err));
    } finally {
      setIsTestingLlm(false);
    }
  };

  const handleSaveLlm = async () => {
    setSavingLlm(true);
    setLlmSaveMsg(null);
    try {
      await setLlmConfig({
        base_url: llm.config.baseUrl,
        api_key: llm.config.apiKey,
        model: llm.config.model,
        scnet_ocr_api_key: scnetApiKey || undefined,
      });
      resetLlm(llm.config);
      setLlmSaveMsg("已保存 LLM 配置");
    } catch (err) {
      setLlmSaveMsg(String(err));
    } finally {
      setSavingLlm(false);
    }
  };

  const handleSaveScnet = async () => {
    setSavingScnet(true);
    setScnetSaveMsg(null);
    try {
      await setLlmConfig({
        base_url: llm.config.baseUrl,
        api_key: llm.config.apiKey,
        model: llm.config.model,
        scnet_ocr_api_key: scnetApiKey || undefined,
      });
      setScnetSaveMsg("已保存 SCNet 配置");
    } catch (err) {
      setScnetSaveMsg(String(err));
    } finally {
      setSavingScnet(false);
    }
  };

  const handlePresetClick = (preset: typeof LLM_PRESETS[number]) => {
    setLlmField("baseUrl", preset.baseUrl);
    setLlmField("model", preset.model);
    setLlmTestResult(null);
    setLlmTestError(null);
  };

  // --- Embedding handlers ---
  const handleToggleEmbedding = async (checked: boolean) => {
    setEmbStatus((prev) => ({ ...prev, enabled: checked }));
    try {
      await setEmbeddingEnabled(checked);
    } catch {
      // revert on error
      setEmbStatus((prev) => ({ ...prev, enabled: !checked }));
    }
  };

  const handleDownloadModel = async () => {
    setIsDownloadingEmb(true);
    setEmbDownloadMsg(null);
    setEmbTestError(null);
    try {
      const status = await downloadEmbeddingModel();
      setEmbStatus(status);
      setEmbDownloadMsg("模型下载完成");
    } catch (err) {
      setEmbDownloadMsg(null);
      setEmbTestError(String(err));
    } finally {
      setIsDownloadingEmb(false);
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

  const handleRegenerateEmbeddings = async () => {
    setIsRegeneratingEmb(true);
    setEmbRegenMsg(null);
    try {
      const result = await regenerateAllEmbeddings();
      setEmbRegenMsg(
        `完成：${result.success_count}/${result.total_invoices} 成功` +
          (result.failure_count > 0 ? `，${result.failure_count} 失败` : ""),
      );
    } catch (err) {
      setEmbRegenMsg(String(err));
    } finally {
      setIsRegeneratingEmb(false);
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
      {/* LLM Provider */}
      <div className="section">
        <h3>LLM Provider（多模态）</h3>
        <p className="section-desc">
          推荐使用具备视觉和文本能力的多模态模型，同时满足发票图片识别和 AI 助手对话需求。
        </p>

        <div className="preset-badges">
          {LLM_PRESETS.map((preset) => (
            <button
              key={preset.label}
              className={`preset-badge preset-badge-${preset.color}`}
              onClick={() => handlePresetClick(preset)}
              type="button"
            >
              {preset.label}
            </button>
          ))}
        </div>

        <div className="form-grid">
          <label className="form-field">
            <span>Base URL</span>
            <input
              value={llm.config.baseUrl}
              onChange={(e) => setLlmField("baseUrl", e.target.value)}
              placeholder="https://api.openai.com/v1"
              spellCheck={false}
            />
          </label>
          <label className="form-field">
            <span>Model</span>
            <input
              value={llm.config.model}
              onChange={(e) => setLlmField("model", e.target.value)}
              placeholder="qwen-vl-max"
              spellCheck={false}
            />
          </label>
          <label className="form-field">
            <span>API Key</span>
            <div className="input-with-toggle">
              <input
                value={llm.config.apiKey}
                onChange={(e) => setLlmField("apiKey", e.target.value)}
                type={showApiKey ? "text" : "password"}
                placeholder="sk-..."
                spellCheck={false}
              />
              <button
                className="input-toggle-btn"
                type="button"
                onClick={() => setShowApiKey((v) => !v)}
                title={showApiKey ? "隐藏" : "显示"}
              >
                {showApiKey ? <EyeOff size={16} /> : <Eye size={16} />}
              </button>
            </div>
          </label>
        </div>

        <div className="provider-actions">
          <button className="btn-primary" onClick={handleTestLlm} disabled={isTestingLlm}>
            {isTestingLlm ? "测试中..." : "测试连接"}
          </button>
          <button
            className="btn-primary"
            onClick={handleSaveLlm}
            disabled={savingLlm || !llm.dirty || !llm.testPassed}
          >
            {savingLlm ? "保存中..." : "保存设置"}
          </button>
          {llmSaveMsg ? <span className="badge-config-message">{llmSaveMsg}</span> : null}
        </div>

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

      {/* SCNet OCR */}
      <div className="section">
        <h3>SCNet 专业票据 OCR（可选）</h3>
        <p className="section-desc">
          配置后，识别时会调用 SCNet 专业票据 OCR 对关键字段（发票号码、金额、购销方等）进行交叉验证，提高识别准确率。
          未配置则仅使用 LLM 多模态识别。
        </p>

        <div className="form-grid">
          <label className="form-field">
            <span>SCNet API Key</span>
            <div className="input-with-toggle">
              <input
                value={scnetApiKey}
                onChange={(e) => setScnetApiKey(e.target.value)}
                type={showScnetKey ? "text" : "password"}
                placeholder="留空则不启用 SCNet OCR"
                spellCheck={false}
              />
              <button
                className="input-toggle-btn"
                type="button"
                onClick={() => setShowScnetKey((v) => !v)}
                title={showScnetKey ? "隐藏" : "显示"}
              >
                {showScnetKey ? <EyeOff size={16} /> : <Eye size={16} />}
              </button>
            </div>
          </label>
        </div>

        <div className="provider-actions">
          <button className="btn-primary" onClick={handleSaveScnet} disabled={savingScnet}>
            {savingScnet ? "保存中..." : "保存 SCNet 配置"}
          </button>
          {scnetSaveMsg ? <span className="badge-config-message">{scnetSaveMsg}</span> : null}
        </div>
      </div>

      {/* Local Embedding Model */}
      <div className="section">
        <h3>本地 Embedding 模型</h3>
        <p className="section-desc">
          使用 BAAI/bge-small-zh-v1.5 (ONNX)，用于语义搜索和去重。
          模型大小 ~23MB，首次使用自动下载。运行时内存约 100-150MB。
        </p>

        <label className="settings-toggle-card">
          <input
            type="checkbox"
            checked={embStatus.model_loaded && embStatus.enabled}
            disabled={!embStatus.model_loaded}
            onChange={(e) => handleToggleEmbedding(e.target.checked)}
          />
          <span>
            <strong>启用本地 Embedding</strong>
            <small>开启后发票导入时自动生成语义向量，支持语义搜索和去重。</small>
          </span>
        </label>

        <div className="test-result" style={{ marginTop: 12 }}>
          <div className="test-result-row">
            <span>模型状态</span>
            <strong>
              {embStatus.model_loaded ? "已加载" : "未下载"}
            </strong>
          </div>
          {embStatus.dimensions != null ? (
            <div className="test-result-row">
              <span>向量维度</span>
              <strong>{embStatus.dimensions}</strong>
            </div>
          ) : null}
          {embStatus.model_dir ? (
            <div className="test-result-row">
              <span>模型路径</span>
              <strong className="mono" style={{ fontSize: "0.75em", wordBreak: "break-all" }}>
                {embStatus.model_dir}
              </strong>
            </div>
          ) : null}
        </div>

        <div className="provider-actions" style={{ marginTop: 12 }}>
          {!embStatus.model_loaded ? (
            <button
              className="btn-primary"
              onClick={handleDownloadModel}
              disabled={isDownloadingEmb}
            >
              {isDownloadingEmb ? "下载中..." : "下载模型"}
            </button>
          ) : null}
          <button
            className="btn-primary"
            onClick={handleTestEmbedding}
            disabled={isTestingEmb || !embStatus.model_loaded}
          >
            {isTestingEmb ? "测试中..." : "测试推理"}
          </button>
          <button
            className="btn-primary"
            onClick={handleRegenerateEmbeddings}
            disabled={isRegeneratingEmb || !embStatus.model_loaded || !embStatus.enabled}
          >
            {isRegeneratingEmb ? "生成中..." : "重新生成全部 Embedding"}
          </button>
          {embDownloadMsg ? (
            <span className="badge-config-message">{embDownloadMsg}</span>
          ) : null}
          {embRegenMsg ? (
            <span className="badge-config-message">{embRegenMsg}</span>
          ) : null}
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

      {/* End-to-End Diagnostic */}
      <div className="section">
        <h3>端到端诊断</h3>
        <p className="section-desc">
          综合测试 OCR 多模态识别、Agent 文本生成和 Embedding 的完整链路，验证实际可用性。
        </p>
        <button className="btn-primary" onClick={() => setShowDiagnostic(true)}>
          运行诊断测试
        </button>
      </div>

      <LlmDiagnosticDialog
        open={showDiagnostic}
        onClose={() => setShowDiagnostic(false)}
      />
    </>
  );
}

export default AiProviderPage;
