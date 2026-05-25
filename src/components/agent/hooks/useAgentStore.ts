import { create } from "zustand";
import type {
  AgentSession,
  AgentMessage,
  AgentAttachment,
  AgentTask,
  AgentArtifact,
  PendingConfirmation,
  AgentStreamEvent,
} from "../../../types";
import {
  listAgentSessions,
  createAgentSession,
  deleteAgentSession,
  getAgentSession,
  updateAgentSessionTitle,
  generateSessionTitleApi,
  sendAgentMessage,
  confirmAgentAction,
  listAgentTasks,
  listAgentArtifacts,
  attachAgentFile,
  removeAgentAttachment,
} from "../../../api";
import { useLlmStore } from "../../../stores/llmStore";

function getLlmConfig() {
  const llm = useLlmStore.getState().llm;
  return {
    base_url: llm.config.baseUrl,
    api_key: llm.config.apiKey,
    model: llm.config.model,
    timeout_seconds: 120,
  };
}

type StreamPhase =
  | "idle"
  | "starting"
  | "thinking"
  | "tool_call"
  | "tool_result"
  | "answering"
  | "done"
  | "error";

interface StreamState {
  streamId: string;
  phase: StreamPhase;
  toolName: string | null;
  deltaContent: string;
  errorMessage: string | null;
}

interface AgentState {
  sessions: AgentSession[];
  activeSessionId: number | null;
  messages: AgentMessage[];
  artifacts: AgentArtifact[];
  tasks: AgentTask[];
  pendingConfirm: PendingConfirmation | null;
  streamState: StreamState | null;
  pendingAttachments: AgentAttachment[];
  loading: boolean;

  setActiveSession: (id: number | null) => void;
  loadSessions: () => Promise<void>;
  createSession: () => Promise<number>;
  deleteSession: (id: number) => Promise<void>;
  loadMessages: (sessionId: number) => Promise<void>;
  sendMessage: (content: string) => Promise<void>;
  confirmAction: (
    confirmed: boolean,
    extra?: Record<string, unknown>
  ) => Promise<void>;
  attachFile: (path: string) => Promise<void>;
  removeAttachment: (id: number) => Promise<void>;
  refreshArtifacts: () => Promise<void>;
  refreshTasks: () => Promise<void>;
  handleStreamEvent: (event: AgentStreamEvent) => Promise<void>;
  resetStream: () => void;
  setPendingConfirm: (confirm: PendingConfirmation | null) => void;
  updateSessionTitle: (sessionId: number, title: string) => Promise<void>;
  generateSessionTitle: (sessionId: number) => Promise<void>;
}

export const useAgentStore = create<AgentState>((set, get) => ({
  sessions: [],
  activeSessionId: null,
  messages: [],
  artifacts: [],
  tasks: [],
  pendingConfirm: null,
  streamState: null,
  pendingAttachments: [],
  loading: false,

  setActiveSession: (id) => {
    set({ activeSessionId: id, messages: [], streamState: null, pendingConfirm: null });
    if (id !== null) {
      get().loadMessages(id);
      get().refreshTasks();
      get().refreshArtifacts();
    }
  },

  loadSessions: async () => {
    try {
      const sessions = await listAgentSessions();
      set({ sessions });
    } catch (err) {
      console.error("Failed to load sessions:", err);
    }
  },

  createSession: async () => {
    const session = await createAgentSession();
    await get().loadSessions();
    get().setActiveSession(session.id);
    return session.id;
  },

  deleteSession: async (id) => {
    await deleteAgentSession(id);
    const { activeSessionId } = get();
    if (activeSessionId === id) {
      set({ activeSessionId: null, messages: [], streamState: null });
    }
    await get().loadSessions();
  },

  loadMessages: async (sessionId) => {
    try {
      set({ loading: true });
      const messages = await getAgentSession(sessionId);
      set({
        messages,
        loading: false,
      });
    } catch (err) {
      console.error("Failed to load messages:", err);
      set({ loading: false });
    }
  },

  sendMessage: async (content) => {
    const { activeSessionId, pendingAttachments } = get();
    if (activeSessionId === null) return;

    const attachmentIds = pendingAttachments.map((a) => a.id);

    // Optimistic: add user message to UI
    const optimisticMsg: AgentMessage = {
      id: Date.now(),
      session_id: activeSessionId,
      role: "user",
      content,
      tool_call_json: null,
      tool_call_id: null,
      created_at: new Date().toISOString(),
      attachments: pendingAttachments,
    };
    set((state) => ({
      messages: [...state.messages, optimisticMsg],
      pendingAttachments: [],
      streamState: {
        streamId: "",
        phase: "starting",
        toolName: null,
        deltaContent: "",
        errorMessage: null,
      },
    }));

    try {
      const config = getLlmConfig();
      const response = await sendAgentMessage(activeSessionId, content, config, attachmentIds);
      // Reload messages and refresh tasks/artifacts
      if (activeSessionId !== null) {
        await get().loadMessages(activeSessionId);
      }
      get().refreshTasks();
      get().refreshArtifacts();
      // Handle pending confirmation
      if (response.pending_confirmation) {
        set({
          pendingConfirm: response.pending_confirmation,
          streamState: null,
        });
      } else {
        set({
          streamState: {
            streamId: "",
            phase: "done",
            toolName: null,
            deltaContent: "",
            errorMessage: null,
          },
        });
      }
      // Check if session title is default and generate a new one
      const currentSession = get().sessions.find((s) => s.id === activeSessionId);
      if (currentSession && currentSession.title === "新对话") {
        get().generateSessionTitle(activeSessionId);
      }
    } catch (err) {
      // Reload messages to show any tool calls that were executed before the error
      if (activeSessionId !== null) {
        await get().loadMessages(activeSessionId);
      }
      set({
        streamState: {
          streamId: "",
          phase: "error",
          toolName: null,
          deltaContent: "",
          errorMessage: String(err),
        },
      });
    }
  },

  confirmAction: async (confirmed, extra) => {
    const { activeSessionId } = get();
    if (activeSessionId === null) return;

    set({ pendingConfirm: null });
    set({
      streamState: {
        streamId: "",
        phase: "starting",
        toolName: null,
        deltaContent: "",
        errorMessage: null,
      },
    });

    try {
      const config = getLlmConfig();
      const response = await confirmAgentAction(activeSessionId, confirmed, extra || null, config);
      // Reload messages and refresh tasks/artifacts
      if (activeSessionId !== null) {
        await get().loadMessages(activeSessionId);
      }
      get().refreshTasks();
      get().refreshArtifacts();
      // Handle pending confirmation
      if (response.pending_confirmation) {
        set({
          pendingConfirm: response.pending_confirmation,
          streamState: null,
        });
      } else {
        set({
          streamState: {
            streamId: "",
            phase: "done",
            toolName: null,
            deltaContent: "",
            errorMessage: null,
          },
        });
      }
    } catch (err) {
      set({
        streamState: {
          streamId: "",
          phase: "error",
          toolName: null,
          deltaContent: "",
          errorMessage: String(err),
        },
      });
    }
  },

  attachFile: async (path) => {
    const { activeSessionId } = get();
    if (activeSessionId === null) return;
    try {
      const attachment = await attachAgentFile(activeSessionId, path);
      set((state) => ({
        pendingAttachments: [...state.pendingAttachments, attachment],
      }));
    } catch (err) {
      console.error("Failed to attach file:", err);
    }
  },

  removeAttachment: async (id) => {
    try {
      await removeAgentAttachment(id);
      set((state) => ({
        pendingAttachments: state.pendingAttachments.filter((a) => a.id !== id),
      }));
    } catch (err) {
      console.error("Failed to remove attachment:", err);
    }
  },

  refreshArtifacts: async () => {
    const { activeSessionId } = get();
    if (activeSessionId === null) return;
    try {
      const artifacts = await listAgentArtifacts(activeSessionId);
      set({ artifacts });
    } catch (err) {
      console.error("Failed to load artifacts:", err);
    }
  },

  refreshTasks: async () => {
    const { activeSessionId } = get();
    if (activeSessionId === null) return;
    try {
      const tasks = await listAgentTasks(activeSessionId);
      set({ tasks });
    } catch (err) {
      console.error("Failed to load tasks:", err);
    }
  },

  handleStreamEvent: async (event) => {
    const { activeSessionId } = get();
    if (event.session_id !== activeSessionId) return;

    switch (event.type) {
      case "started":
        set({
          streamState: {
            streamId: event.stream_id,
            phase: "thinking",
            toolName: null,
            deltaContent: "",
            errorMessage: null,
          },
        });
        break;

      case "assistant_delta":
        set((state) => ({
          streamState: state.streamState
            ? {
                ...state.streamState,
                phase: "answering",
                deltaContent: state.streamState.deltaContent + event.delta,
              }
            : null,
        }));
        break;

      case "tool_call":
        set((state) => ({
          streamState: state.streamState
            ? {
                ...state.streamState,
                phase: "tool_call",
                toolName: event.tool_name,
              }
            : null,
        }));
        break;

      case "tool_result":
        set((state) => ({
          streamState: state.streamState
            ? {
                ...state.streamState,
                phase: "tool_result",
                toolName: event.tool_name,
              }
            : null,
        }));
        // Refresh tasks and artifacts after tool execution
        get().refreshTasks();
        get().refreshArtifacts();
        break;

      case "pending_confirmation":
        set({
          pendingConfirm: event.pending_confirmation,
          streamState: null,
        });
        break;

      case "finished":
        set({
          streamState: {
            streamId: event.stream_id,
            phase: "done",
            toolName: null,
            deltaContent: "",
            errorMessage: null,
          },
        });
        // Reload messages to get the final state
        if (activeSessionId !== null) {
          await get().loadMessages(activeSessionId);

          // Check if session title is default and generate a new one
          const currentSession = get().sessions.find((s) => s.id === activeSessionId);
          if (currentSession && currentSession.title === "新对话") {
            // Generate title in background
            get().generateSessionTitle(activeSessionId);
          }
        }
        break;

      case "error":
        set({
          streamState: {
            streamId: event.stream_id,
            phase: "error",
            toolName: null,
            deltaContent: "",
            errorMessage: event.message,
          },
        });
        break;
    }
  },

  resetStream: () => {
    set({ streamState: null });
  },

  setPendingConfirm: (pendingConfirm) => {
    set({ pendingConfirm });
  },

  updateSessionTitle: async (sessionId, title) => {
    try {
      await updateAgentSessionTitle(sessionId, title);
      await get().loadSessions();
    } catch (err) {
      console.error("Failed to update session title:", err);
    }
  },

  generateSessionTitle: async (sessionId) => {
    try {
      const config = getLlmConfig();
      const title = await generateSessionTitleApi(sessionId, config);
      if (title) {
        await get().loadSessions();
      }
    } catch (err) {
      console.error("Failed to generate session title:", err);
    }
  },
}));
