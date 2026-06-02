import { useNavigate } from "react-router-dom";
import { useAppStore } from "../stores/appStore";
import { Sparkles, FileText, Bot, Settings } from "lucide-react";

export function OnboardingDialog() {
  const navigate = useNavigate();
  const dismissOnboarding = useAppStore((s) => s.dismissOnboarding);

  const handleGoToSettings = () => {
    dismissOnboarding();
    navigate("/settings/ai");
  };

  const handleDismiss = () => {
    dismissOnboarding();
  };

  return (
    <div className="modal-overlay" onClick={handleDismiss}>
      <div
        className="modal-card onboarding-card"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="onboarding-header">
          <Sparkles size={28} className="onboarding-logo" />
          <h2 className="modal-title">欢迎使用 票匣</h2>
        </div>

        <p className="modal-message">
          票匣是一款智能发票管理工具，可以帮助您自动识别、归档和分析发票数据。
        </p>

        <ul className="onboarding-features">
          <li>
            <FileText size={16} />
            <span>导入 PDF / 图片，自动识别发票信息</span>
          </li>
          <li>
            <Bot size={16} />
            <span>AI 助手帮你分析和查询发票</span>
          </li>
          <li>
            <Settings size={16} />
            <span>自动检测重复发票，智能归档</span>
          </li>
        </ul>

        <p className="onboarding-hint">
          使用这些功能需要配置一个 LLM API Key（如 DashScope、OpenAI 等）。
        </p>

        <div className="modal-actions">
          <button className="btn-primary" onClick={handleGoToSettings}>
            去配置
          </button>
          <button className="btn-small" onClick={handleDismiss}>
            稍后再说
          </button>
        </div>
      </div>
    </div>
  );
}
