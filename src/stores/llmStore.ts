import { create } from "zustand";
import {
  getLlmConfig,
  getLlmAuditEnabled,
} from "../api";

type ProviderConfig = {
  baseUrl: string;
  model: string;
  apiKey: string;
};

type PanelState = {
  config: ProviderConfig;
  dirty: boolean;
  testPassed: boolean;
};

type LlmStore = {
  llm: PanelState;
  setLlmField: (field: keyof ProviderConfig, value: string) => void;
  resetLlm: (config: ProviderConfig) => void;
  markLlmTestPassed: (passed: boolean) => void;

  auditEnabled: boolean;
  setAuditEnabled: (v: boolean) => void;

  loadConfigFromBackend: () => Promise<void>;
};

const defaultProvider: ProviderConfig = {
  baseUrl: "",
  model: "",
  apiKey: "",
};

function makePanel(config: ProviderConfig): PanelState {
  return { config, dirty: false, testPassed: false };
}

export const useLlmStore = create<LlmStore>((set) => ({
  llm: makePanel({ ...defaultProvider }),
  auditEnabled: true,

  setLlmField: (field, value) =>
    set((s) => ({
      llm: {
        ...s.llm,
        config: { ...s.llm.config, [field]: value },
        dirty: true,
        testPassed: false,
      },
    })),

  resetLlm: (config) => set({ llm: makePanel(config) }),

  markLlmTestPassed: (passed) =>
    set((s) => ({ llm: { ...s.llm, testPassed: passed } })),

  setAuditEnabled: (auditEnabled) => set({ auditEnabled }),

  loadConfigFromBackend: async () => {
    try {
      const [llmCfg, audit] = await Promise.all([
        getLlmConfig(),
        getLlmAuditEnabled(),
      ]);
      set({
        llm: makePanel({
          baseUrl: llmCfg?.base_url ?? defaultProvider.baseUrl,
          model: llmCfg?.model ?? defaultProvider.model,
          apiKey: llmCfg?.api_key ?? defaultProvider.apiKey,
        }),
        auditEnabled: audit !== false,
      });
    } catch {
      // keep defaults
    }
  },
}));
