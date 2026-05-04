import { create } from "zustand";
import { getLlmConfig } from "../api";

type LlmStore = {
  llmBaseUrl: string;
  llmModel: string;
  llmApiKey: string;
  llmAuditEnabled: boolean;

  setLlmBaseUrl: (v: string) => void;
  setLlmModel: (v: string) => void;
  setLlmApiKey: (v: string) => void;
  setLlmAuditEnabled: (v: boolean) => void;
  loadConfigFromBackend: () => Promise<void>;
};

export const useLlmStore = create<LlmStore>((set) => ({
  llmBaseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
  llmModel: "qwen3.6-plus",
  llmApiKey: "",
  llmAuditEnabled: true,

  setLlmBaseUrl: (llmBaseUrl) => set({ llmBaseUrl }),
  setLlmModel: (llmModel) => set({ llmModel }),
  setLlmApiKey: (llmApiKey) => set({ llmApiKey }),
  setLlmAuditEnabled: (llmAuditEnabled) => set({ llmAuditEnabled }),

  loadConfigFromBackend: async () => {
    try {
      const cfg = await getLlmConfig();
      if (cfg) {
        set({
          llmBaseUrl: cfg.base_url,
          llmModel: cfg.model,
          llmApiKey: cfg.api_key,
          llmAuditEnabled: cfg.audit_enabled !== false,
        });
      }
    } catch {
      // Backend config not available, keep defaults
    }
  },
}));
