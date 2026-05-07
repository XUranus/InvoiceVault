import React from "react";
import type { PriceConfig } from "../../types";
import {
  getRecognitionQueueStatus,
  setRecognitionConcurrency,
  getPriceConfig,
  setPriceConfig,
} from "../../api";
import { useAppStore } from "../../stores/appStore";
import { Sun, Moon } from "lucide-react";

export function GeneralPage() {
  const theme = useAppStore((s) => s.theme);
  const toggleTheme = useAppStore((s) => s.toggleTheme);

  const [recognitionConcurrency, setRecognitionConcurrencyState] = React.useState(3);
  const [priceConfig, setPriceConfigState] = React.useState<PriceConfig>({
    llm_input_price_per_1k: 0.0008,
    llm_output_price_per_1k: 0.002,
    embedding_input_price_per_1k: 0.0007,
    embedding_output_price_per_1k: 0.0007,
  });

  React.useEffect(() => {
    let cancelled = false;
    Promise.all([
      getRecognitionQueueStatus().catch(() => null),
      getPriceConfig().catch(() => null),
    ]).then(([recog, price]) => {
      if (cancelled) return;
      if (recog) setRecognitionConcurrencyState(recog.max_concurrent);
      if (price) setPriceConfigState(price);
    });
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

      {/* Recognition Concurrency */}
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
    </>
  );
}

export default GeneralPage;
