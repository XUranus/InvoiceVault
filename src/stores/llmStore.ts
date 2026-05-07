import { create } from "zustand";
import {
  getLlmConfig,
  getAgentLlmConfig,
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
  ocr: PanelState;
  setOcrField: (field: keyof ProviderConfig, value: string) => void;
  resetOcr: (config: ProviderConfig) => void;
  markOcrTestPassed: (passed: boolean) => void;

  agent: PanelState;
  setAgentField: (field: keyof ProviderConfig, value: string) => void;
  resetAgent: (config: ProviderConfig) => void;
  markAgentTestPassed: (passed: boolean) => void;

  auditEnabled: boolean;
  setAuditEnabled: (v: boolean) => void;

  loadConfigFromBackend: () => Promise<void>;
};

const defaultProvider: ProviderConfig = {
  baseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
  model: "qwen3.6-plus",
  apiKey: "",
};

function makePanel(config: ProviderConfig): PanelState {
  return { config, dirty: false, testPassed: false };
}

export const useLlmStore = create<LlmStore>((set) => ({
  ocr: makePanel({ ...defaultProvider }),
  agent: makePanel({ ...defaultProvider }),
  auditEnabled: true,

  setOcrField: (field, value) =>
    set((s) => ({
      ocr: {
        ...s.ocr,
        config: { ...s.ocr.config, [field]: value },
        dirty: true,
        testPassed: false,
      },
    })),

  resetOcr: (config) => set({ ocr: makePanel(config) }),

  markOcrTestPassed: (passed) =>
    set((s) => ({ ocr: { ...s.ocr, testPassed: passed } })),

  setAgentField: (field, value) =>
    set((s) => ({
      agent: {
        ...s.agent,
        config: { ...s.agent.config, [field]: value },
        dirty: true,
        testPassed: false,
      },
    })),

  resetAgent: (config) => set({ agent: makePanel(config) }),

  markAgentTestPassed: (passed) =>
    set((s) => ({ agent: { ...s.agent, testPassed: passed } })),

  setAuditEnabled: (auditEnabled) => set({ auditEnabled }),

  loadConfigFromBackend: async () => {
    try {
      const [ocrCfg, agentCfg, audit] = await Promise.all([
        getLlmConfig(),
        getAgentLlmConfig(),
        getLlmAuditEnabled(),
      ]);
      set({
        ocr: makePanel({
          baseUrl: ocrCfg?.base_url ?? defaultProvider.baseUrl,
          model: ocrCfg?.model ?? defaultProvider.model,
          apiKey: ocrCfg?.api_key ?? defaultProvider.apiKey,
        }),
        agent: makePanel({
          baseUrl: agentCfg?.base_url ?? defaultProvider.baseUrl,
          model: agentCfg?.model ?? defaultProvider.model,
          apiKey: agentCfg?.api_key ?? defaultProvider.apiKey,
        }),
        auditEnabled: audit !== false,
      });
    } catch {
      // keep defaults
    }
  },
}));
