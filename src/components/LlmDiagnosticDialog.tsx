import React from "react";
import type { DiagnosticStep, DiagnosticResult } from "../types";
import { runLlmDiagnostic } from "../api";
import { CheckCircle, XCircle, Loader, TestTube } from "lucide-react";

type Props = {
  open: boolean;
  onClose: () => void;
};

export function LlmDiagnosticDialog({ open, onClose }: Props) {
  const [running, setRunning] = React.useState(false);
  const [result, setResult] = React.useState<DiagnosticResult | null>(null);
  const [currentStep, setCurrentStep] = React.useState(-1);
  const [error, setError] = React.useState<string | null>(null);

  const handleRun = React.useCallback(async () => {
    setRunning(true);
    setResult(null);
    setError(null);
    setCurrentStep(0);

    try {
      const diagResult = await runLlmDiagnostic();
      setResult(diagResult);
      setCurrentStep(diagResult.steps.length);
    } catch (err) {
      setError(String(err));
    } finally {
      setRunning(false);
    }
  }, []);

  // Auto-run on open
  React.useEffect(() => {
    if (open && !result && !running) {
      handleRun();
    }
  }, [open, handleRun, result, running]);

  if (!open) return null;

  return (
    <div className="modal-overlay" onClick={running ? undefined : onClose}>
      <div
        className="modal-card diagnostic-card"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="diagnostic-header">
          <TestTube size={20} />
          <h3 className="modal-title">LLM 端到端诊断</h3>
        </div>

        <div className="diagnostic-steps">
          {running && !result
            ? [0, 1, 2, 3].map((i) => (
                <DiagnosticStepRow
                  key={i}
                  step={
                    i < currentStep
                      ? null
                      : i === currentStep
                        ? {
                            name: STEP_NAMES[i],
                            passed: false,
                            duration_ms: 0,
                            message: "检测中...",
                            details: null,
                          }
                        : null
                  }
                  index={i}
                  isRunning={i === currentStep}
                  isPending={i > currentStep}
                />
              ))
            : null}

          {result
            ? result.steps.map((step, i) => (
                <DiagnosticStepRow
                  key={i}
                  step={step}
                  index={i}
                  isRunning={false}
                  isPending={false}
                />
              ))
            : null}
        </div>

        {error ? <p className="text-error diagnostic-error">{error}</p> : null}

        {result ? (
          <div className="diagnostic-summary">
            {result.score != null ? (
              <span
                className={`diagnostic-score ${result.score >= 50 ? "score-pass" : "score-fail"}`}
              >
                识别准确度: {result.score.toFixed(0)}%
              </span>
            ) : null}
            <span
              className={`diagnostic-overall ${result.all_passed ? "overall-pass" : "overall-fail"}`}
            >
              {result.all_passed ? "全部通过" : "部分项目未通过"}
            </span>
          </div>
        ) : null}

        <div className="modal-actions">
          {result && !running ? (
            <button className="btn-small" onClick={handleRun}>
              重新测试
            </button>
          ) : null}
          <button className="btn-primary" onClick={onClose} disabled={running}>
            关闭
          </button>
        </div>
      </div>
    </div>
  );
}

const STEP_NAMES = ["文本生成", "图片识别", "结果对比", "Embedding"];

function DiagnosticStepRow({
  step,
  index,
  isRunning,
  isPending,
}: {
  step: DiagnosticStep | null;
  index: number;
  isRunning: boolean;
  isPending: boolean;
}) {
  const name = step?.name ?? STEP_NAMES[index];
  const [expanded, setExpanded] = React.useState(false);

  if (isPending) {
    return (
      <div className="diagnostic-step step-pending">
        <span className="step-icon step-icon-pending">{index + 1}</span>
        <span className="step-name">{name}</span>
        <span className="step-status muted">等待中</span>
      </div>
    );
  }

  if (isRunning) {
    return (
      <div className="diagnostic-step step-running">
        <span className="step-icon">
          <Loader size={16} className="inline-spinner" />
        </span>
        <span className="step-name">{name}</span>
        <span className="step-status muted">检测中...</span>
      </div>
    );
  }

  if (!step) return null;

  return (
    <div
      className={`diagnostic-step ${step.passed ? "step-passed" : "step-failed"} ${step.details ? "step-expandable" : ""}`}
      onClick={() => step.details && setExpanded((v) => !v)}
    >
      <span className="step-icon">
        {step.passed ? (
          <CheckCircle size={16} className="icon-pass" />
        ) : (
          <XCircle size={16} className="icon-fail" />
        )}
      </span>
      <span className="step-name">{name}</span>
      <span className="step-status">
        {step.message}
        {step.duration_ms > 0 ? (
          <span className="step-duration"> ({step.duration_ms}ms)</span>
        ) : null}
      </span>
      {expanded && step.details ? (
        <pre className="step-details">{step.details}</pre>
      ) : null}
    </div>
  );
}
