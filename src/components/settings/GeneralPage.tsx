import React from "react";
import type { PriceConfig } from "../../types";
import {
  getPriceConfig,
  setPriceConfig,
  regenerateAllDuplicates,
} from "../../api";
import { useAppStore } from "../../stores/appStore";
import { useRefreshStore } from "../../stores/refreshStore";
import { Sun, Moon } from "lucide-react";

export function GeneralPage() {
  const theme = useAppStore((s) => s.theme);
  const toggleTheme = useAppStore((s) => s.toggleTheme);

  const [priceConfig, setPriceConfigState] = React.useState<PriceConfig>({
    llm_input_price_per_1k: 0.0008,
    llm_output_price_per_1k: 0.002,
    embedding_input_price_per_1k: 0.0007,
    embedding_output_price_per_1k: 0.0007,
  });

  const refreshInvoices = useAppStore((s) => s.refreshInvoices);
  const triggerInvoicesRefresh = useRefreshStore((s) => s.triggerInvoicesRefresh);

  const [regenerating, setRegenerating] = React.useState(false);
  const [regenResult, setRegenResult] = React.useState<string | null>(null);

  const handleRegenerate = async () => {
    if (regenerating) return;
    setRegenerating(true);
    setRegenResult(null);
    try {
      const count = await regenerateAllDuplicates();
      setRegenResult(`完成：已重新检测 ${count} 张发票的重复状态`);
      refreshInvoices();
      triggerInvoicesRefresh();
    } catch (err) {
      setRegenResult(`失败：${String(err)}`);
    } finally {
      setRegenerating(false);
    }
  };

  React.useEffect(() => {
    let cancelled = false;
    getPriceConfig()
      .then((price) => {
        if (cancelled) return;
        if (price) setPriceConfigState(price);
      })
      .catch(() => {});
    return () => { cancelled = true; };
  }, []);

  return (
    <>
      {/* Appearance */}
      <div className="section">
        <h3>外观</h3>
        <p className="section-desc">
          当前: {theme === "dark" ? "暗色主题" : "亮色主题"}
        </p>
        <button className="btn-primary" onClick={toggleTheme}>
          {theme === "dark" ? <><Sun size={16} /> 切换到亮色主题</> : <><Moon size={16} /> 切换到暗色主题</>}
        </button>
      </div>

      {/* Price Config */}
      <div className="section">
        <h3>LLM 价格配置</h3>
        <p className="section-desc">
          配置每千 token 的价格（¥），用于仪表盘用量费用预估。默认为 qwen-plus 官方价格。
        </p>
        <div className="form-grid">
          <label className="form-field">
            <span>LLM 输入价格 (¥/千token)</span>
            <input
              type="number"
              step="0.0001"
              min="0"
              value={priceConfig.llm_input_price_per_1k}
              onChange={(e) =>
                setPriceConfigState((prev) => ({
                  ...prev,
                  llm_input_price_per_1k: Number(e.target.value) || 0,
                }))
              }
            />
          </label>
          <label className="form-field">
            <span>LLM 输出价格 (¥/千token)</span>
            <input
              type="number"
              step="0.0001"
              min="0"
              value={priceConfig.llm_output_price_per_1k}
              onChange={(e) =>
                setPriceConfigState((prev) => ({
                  ...prev,
                  llm_output_price_per_1k: Number(e.target.value) || 0,
                }))
              }
            />
          </label>
          <label className="form-field">
            <span>Embedding 输入价格 (¥/千token)</span>
            <input
              type="number"
              step="0.0001"
              min="0"
              value={priceConfig.embedding_input_price_per_1k}
              onChange={(e) =>
                setPriceConfigState((prev) => ({
                  ...prev,
                  embedding_input_price_per_1k: Number(e.target.value) || 0,
                }))
              }
            />
          </label>
          <label className="form-field">
            <span>Embedding 输出价格 (¥/千token)</span>
            <input
              type="number"
              step="0.0001"
              min="0"
              value={priceConfig.embedding_output_price_per_1k}
              onChange={(e) =>
                setPriceConfigState((prev) => ({
                  ...prev,
                  embedding_output_price_per_1k: Number(e.target.value) || 0,
                }))
              }
            />
          </label>
        </div>
      </div>

      {/* Duplicate Detection */}
      <div className="section">
        <h3>重复检测</h3>
        <p className="section-desc">
          清除所有已有的重复检测结果，根据当前阈值重新匹配所有发票并生成告警。
        </p>
        <button
          className="btn-primary"
          onClick={handleRegenerate}
          disabled={regenerating}
        >
          {regenerating ? "检测中…" : "重新检测重复"}
        </button>
        {regenResult && (
          <p className="section-desc" style={{ marginTop: 8 }}>{regenResult}</p>
        )}
      </div>
    </>
  );
}

export default GeneralPage;
