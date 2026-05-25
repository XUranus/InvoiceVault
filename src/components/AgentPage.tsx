import React from "react";
import type {
  AgentSession,
  AgentMessage,
  PendingConfirmation,
  AgentAttachment,
  AgentArtifact,
  AgentStreamEvent,
} from "../types";
import {
  createAgentSession,
  listAgentSessions,
  getAgentSession,
  deleteAgentSession,
  sendAgentMessageStream,
  confirmAgentActionStream,
  attachAgentFile,
  listAgentArtifacts,
  openAgentArtifactFile,
  openAgentArtifactFolder,
  deleteAgentArtifact,
} from "../api";
import { open, save } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { useAppStore } from "../stores/appStore";
import { useLlmStore } from "../stores/llmStore";

const TASK_CARDS = [
  { label: "统计本月发票金额", query: "统计本月发票金额" },
  { label: "搜索办公用品发票", query: "搜索办公用品相关的发票" },
  { label: "导出为 CSV", query: "导出发票为 CSV 格式" },
  { label: "检查重复风险", query: "检查发票库中是否有重复的发票" },
] as const;

type StreamUiState = {
  streamId: string;
  sessionId: number;
  assistantMessageId: number;
  phase: "starting" | "thinking" | "tool" | "answering" | "done" | "error";
  toolName: string | null;
  errorMessage: string | null;
};

export function AgentPage() {
  const llm = useLlmStore((s) => s.llm);
  const auditEnabled = useLlmStore((s) => s.auditEnabled);
  const setError = useAppStore((s) => s.setError);
  const [sessions, setSessions] = React.useState<AgentSession[]>([]);
  const [activeSessionId, setActiveSessionId] = React.useState<number | null>(null);
  const [messages, setMessages] = React.useState<AgentMessage[]>([]);
  const [input, setInput] = React.useState("");
  const [loading, setLoading] = React.useState(false);
  const [pendingConfirm, setPendingConfirm] = React.useState<PendingConfirmation | null>(null);
  const [pendingAttachments, setPendingAttachments] = React.useState<AgentAttachment[]>([]);
  const [artifacts, setArtifacts] = React.useState<AgentArtifact[]>([]);
  const [streamState, setStreamState] = React.useState<StreamUiState | null>(null);
  const [sessionSearch, setSessionSearch] = React.useState("");
  const [sessionRenameId, setSessionRenameId] = React.useState<number | null>(null);
  const [sessionRenameValue, setSessionRenameValue] = React.useState("");
  const [collapsedTools, setCollapsedTools] = React.useState<Set<number>>(new Set());
  const messagesEnd = React.useRef<HTMLDivElement>(null);
  const streamStateRef = React.useRef<StreamUiState | null>(null);
  const activeStreamIdRef = React.useRef<string | null>(null);
  const activeSessionIdRef = React.useRef<number | null>(null);
  const skipNextSessionLoadRef = React.useRef<number | null>(null);

  const llmConfig = React.useMemo(
    () => ({
      base_url: llm.config.baseUrl,
      api_key: llm.config.apiKey,
      model: llm.config.model,
      timeout_seconds: 60,
    }),
    [llm.config.baseUrl, llm.config.model, llm.config.apiKey],
  );

  const filteredSessions = React.useMemo(() => {
    const q = sessionSearch.trim().toLowerCase();
    if (!q) return sessions;
    return sessions.filter((s) => (s.title || "新对话").toLowerCase().includes(q));
  }, [sessions, sessionSearch]);

  const groupedSessions = React.useMemo(() => groupSessions(filteredSessions), [filteredSessions]);

  React.useEffect(() => {
    streamStateRef.current = streamState;
  }, [streamState]);

  React.useEffect(() => {
    activeSessionIdRef.current = activeSessionId;
  }, [activeSessionId]);

  // Load sessions on mount
  React.useEffect(() => {
    listAgentSessions()
      .then(setSessions)
      .catch((err) => setError(String(err)));
  }, []);

  React.useEffect(() => {
    let unlisten: (() => void) | null = null;
    listen<AgentStreamEvent>("agent://stream", (event) => {
      const payload = event.payload;
      if (payload.stream_id !== activeStreamIdRef.current) return;
      const current = streamStateRef.current;
      if (!current || payload.session_id !== current.sessionId) return;
      if (activeSessionIdRef.current !== payload.session_id) return;

      if (payload.type === "assistant_delta") {
        setMessages((prev) =>
          prev.map((msg) =>
            msg.id === current.assistantMessageId
              ? { ...msg, content: msg.content + payload.delta }
              : msg,
          ),
        );
        setStreamState((prev) =>
          prev ? { ...prev, phase: "answering", toolName: null } : prev,
        );
      } else if (payload.type === "tool_call") {
        setStreamState((prev) =>
          prev ? { ...prev, phase: "tool", toolName: payload.tool_name } : prev,
        );
      } else if (payload.type === "tool_result") {
        setStreamState((prev) =>
          prev ? { ...prev, phase: "thinking", toolName: payload.tool_name } : prev,
        );
      } else if (payload.type === "pending_confirmation") {
        setPendingConfirm(payload.pending_confirmation);
        setStreamState((prev) => (prev ? { ...prev, phase: "done" } : prev));
      } else if (payload.type === "error") {
        setStreamState((prev) =>
          prev ? { ...prev, phase: "error", errorMessage: payload.message } : prev,
        );
      }
    })
      .then((cleanup) => {
        unlisten = cleanup;
      })
      .catch(() => {});

    return () => {
      unlisten?.();
    };
  }, [setError]);

  const refreshArtifacts = React.useCallback(async (sessionId: number) => {
    try {
      const result = await listAgentArtifacts(sessionId);
      setArtifacts(result);
    } catch {
      setArtifacts([]);
    }
  }, []);

  // Load messages when switching sessions
  React.useEffect(() => {
    if (activeSessionId === null) {
      setMessages([]);
      setPendingConfirm(null);
      setPendingAttachments([]);
      setArtifacts([]);
      return;
    }
    if (skipNextSessionLoadRef.current === activeSessionId) {
      skipNextSessionLoadRef.current = null;
      setPendingConfirm(null);
      setArtifacts([]);
      return;
    }
    Promise.all([
      getAgentSession(activeSessionId),
      listAgentArtifacts(activeSessionId),
    ])
      .then(([msgs, artifacts]) => {
        setMessages(msgs);
        setArtifacts(artifacts);
        setPendingConfirm(null);
      })
      .catch((err) => setError(String(err)));
  }, [activeSessionId, refreshArtifacts]);

  React.useEffect(() => {
    if (activeSessionId === null) return;
    refreshArtifacts(activeSessionId).catch(() => {});
  }, [activeSessionId, messages.length, refreshArtifacts]);

  // Scroll to bottom on new messages
  React.useEffect(() => {
    messagesEnd.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const copyText = async (value: string) => {
    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(value);
        return;
      }
      if (!fallbackCopyText(value)) {
        throw new Error("clipboard unavailable");
      }
    } catch (err) {
      if (!fallbackCopyText(value)) {
        setError(`复制失败：${String(err)}`);
      }
    }
  };

  const handleOpenArtifact = async (artifactId: number) => {
    if (activeSessionId === null) return;
    try {
      await openAgentArtifactFile(activeSessionId, artifactId);
    } catch (err) {
      setError(`打开产物失败：${String(err)}`);
    }
  };

  const handleOpenArtifactFolder = async (artifactId: number) => {
    if (activeSessionId === null) return;
    try {
      await openAgentArtifactFolder(activeSessionId, artifactId);
    } catch (err) {
      setError(`打开产物目录失败：${String(err)}`);
    }
  };

  const handleDeleteArtifact = async (artifactId: number) => {
    if (activeSessionId === null) return;
    const confirmed = window.confirm("移除此产物记录？已导出的文件不会被删除。");
    if (!confirmed) return;
    try {
      await deleteAgentArtifact(activeSessionId, artifactId);
      setArtifacts((prev) => prev.filter((artifact) => artifact.id !== artifactId));
    } catch (err) {
      setError(`删除产物失败：${String(err)}`);
    }
  };

  const startStreamingPlaceholder = (
    sessionId: number,
    userText: string | null,
    attachments: AgentAttachment[] = [],
  ) => {
    const streamId = createStreamId();
    const baseMessageId = -Date.now();
    const assistantMessageId = baseMessageId - 1;
    const optimisticMessages: AgentMessage[] = [];
    if (userText !== null) {
      optimisticMessages.push(
        createTempMessage(sessionId, "user", userText, attachments, baseMessageId),
      );
    }
    optimisticMessages.push(
      createTempMessage(sessionId, "assistant", "", [], assistantMessageId),
    );

    activeStreamIdRef.current = streamId;
    setStreamState({
      streamId,
      sessionId,
      assistantMessageId,
      phase: "starting",
      toolName: null,
      errorMessage: null,
    });
    setMessages((prev) => [...prev, ...optimisticMessages]);
    return { streamId, tempIds: optimisticMessages.map((msg) => msg.id) };
  };

  const finishStreamingResponse = (
    response: { messages: AgentMessage[]; pending_confirmation: PendingConfirmation | null },
    tempIds: number[],
  ) => {
    setMessages((prev) => appendUniqueMessages(
      prev.filter((msg) => !tempIds.includes(msg.id)),
      response.messages,
    ));
    setPendingConfirm(response.pending_confirmation);
  };

  const handleNewSession = async () => {
    try {
      const session = await createAgentSession();
      setSessions((prev) => [session, ...prev]);
      setActiveSessionId(session.id);
    } catch (err) {
      setError(String(err));
    }
  };

  const handleDeleteSession = async (id: number) => {
    try {
      await deleteAgentSession(id);
      setSessions((prev) => prev.filter((s) => s.id !== id));
      if (activeSessionId === id) {
        setActiveSessionId(null);
        setMessages([]);
        setPendingAttachments([]);
        setArtifacts([]);
      }
    } catch (err) {
      setError(String(err));
    }
  };

  const handleSend = async () => {
    const text = input.trim();
    if ((!text && pendingAttachments.length === 0) || loading) return;
    if (!llm.config.baseUrl || !llm.config.apiKey || !llm.config.model) {
      setError("请先在设置页配置 LLM Provider");
      return;
    }

    let sessionId = activeSessionId;
    if (sessionId === null) {
      try {
        const session = await createAgentSession();
        setSessions((prev) => [session, ...prev]);
        skipNextSessionLoadRef.current = session.id;
        sessionId = session.id;
        setActiveSessionId(sessionId);
      } catch (err) {
        setError(String(err));
        return;
      }
    }

    const attachments = pendingAttachments;
    const visibleText = text || "请查看我上传的附件。";
    const attachmentIds = attachments.map((attachment) => attachment.id);
    const { streamId, tempIds } = startStreamingPlaceholder(
      sessionId,
      visibleText,
      attachments,
    );
    setInput("");
    setPendingAttachments([]);
    setPendingConfirm(null);
    setLoading(true);

    try {
      const response = await sendAgentMessageStream(
        streamId,
        sessionId,
        visibleText,
        llmConfig,
        attachmentIds,
      );
      finishStreamingResponse(response, tempIds);

      // Refresh session list (title may have updated)
      listAgentSessions()
        .then(setSessions)
        .catch(() => {});
      refreshArtifacts(sessionId).catch(() => {});
    } catch (err) {
      const errorMsg = String(err);
      setStreamState((prev) =>
        prev ? { ...prev, phase: "error" as const, errorMessage: prev.errorMessage ?? errorMsg } : prev,
      );
    } finally {
      activeStreamIdRef.current = null;
      setLoading(false);
      if (streamStateRef.current?.phase !== "error") {
        setStreamState(null);
      }
    }
  };


  const ensureSession = async (): Promise<number | null> => {
    if (activeSessionId !== null) return activeSessionId;
    try {
      const session = await createAgentSession();
      setSessions((prev) => [session, ...prev]);
      setActiveSessionId(session.id);
      return session.id;
    } catch (err) {
      setError(String(err));
      return null;
    }
  };

  const handleAttach = async () => {
    if (loading) return;
    const sessionId = await ensureSession();
    if (sessionId === null) return;
    try {
      const selected = await open({
        title: "选择表格附件",
        multiple: true,
        filters: [
          { name: "Spreadsheet", extensions: ["xlsx", "csv"] },
        ],
      });
      const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
      if (paths.length === 0) return;
      const uploaded: AgentAttachment[] = [];
      for (const path of paths) {
        uploaded.push(await attachAgentFile(sessionId, path));
      }
      setPendingAttachments((prev) => [...prev, ...uploaded]);
    } catch (err) {
      setError(String(err));
    }
  };

  const handleConfirm = async (confirmed: boolean) => {
    if (activeSessionId === null || !pendingConfirm) return;

    let extraParams: Record<string, unknown> | null = null;

    // For export confirmations, open file picker
    if (
      confirmed &&
      (pendingConfirm.tool_name === "export_invoices" ||
        pendingConfirm.tool_name === "export_invoices_with_template")
    ) {
      try {
        const requestedFormat =
          (pendingConfirm.arguments as Record<string, unknown>).format ??
          (pendingConfirm.tool_name === "export_invoices_with_template" ? "xlsx" : "csv");
        const extension = requestedFormat === "xlsx" ? "xlsx" : "csv";
        const path = await save({
          title: "选择导出位置",
          defaultPath: `invoices_export.${extension}`,
          filters: [
            {
              name: "Spreadsheet",
              extensions: ["csv", "xlsx"],
            },
          ],
        });
        if (path) {
          extraParams = { output_path: path };
        } else {
          // User cancelled file picker
          handleConfirmImpl(false, null);
          return;
        }
      } catch {
        setError("文件选择失败");
        return;
      }
    }

    handleConfirmImpl(confirmed, extraParams);
  };

  const handleConfirmImpl = async (
    confirmed: boolean,
    extraParams: Record<string, unknown> | null,
  ) => {
    if (activeSessionId === null) return;

    setPendingConfirm(null);
    const { streamId, tempIds } = startStreamingPlaceholder(activeSessionId, null);
    setLoading(true);

    try {
      const response = await confirmAgentActionStream(
        streamId,
        activeSessionId,
        confirmed,
        extraParams,
        llmConfig,
      );
      finishStreamingResponse(response, tempIds);

      // Refresh session list
      listAgentSessions()
        .then(setSessions)
        .catch(() => {});
      refreshArtifacts(activeSessionId).catch(() => {});
    } catch (err) {
      setError(String(err));
      getAgentSession(activeSessionId)
        .then(setMessages)
        .catch(() => {});
    } finally {
      activeStreamIdRef.current = null;
      setStreamState(null);
      setLoading(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const handleRenameSession = (id: number, currentTitle: string) => {
    setSessionRenameId(id);
    setSessionRenameValue(currentTitle || "");
  };

  const handleRenameSessionSave = (id: number) => {
    const title = sessionRenameValue.trim();
    setSessions((prev) => prev.map((s) => (s.id === id ? { ...s, title: title || s.title } : s)));
    setSessionRenameId(null);
    setSessionRenameValue("");
  };

  const handleRenameSessionCancel = () => {
    setSessionRenameId(null);
    setSessionRenameValue("");
  };

  const toolCallSummary = (msg: AgentMessage): string | null => {
    if (!msg.tool_call_json) return null;
    try {
      const calls: { function: { name: string; arguments: string } }[] =
        JSON.parse(msg.tool_call_json);
      return calls
        .map((c) => {
          const name = c.function.name;
          return `${toolLabel(name)}`;
        })
        .join(", ");
    } catch {
      return null;
    }
  };

  const formatToolResult = (content: string): React.ReactNode => {
    try {
      const parsed = JSON.parse(content);
      if (typeof parsed === "object" && parsed !== null && !Array.isArray(parsed)) {
        if (
          typeof (parsed as { artifact?: unknown }).artifact === "object" &&
          (parsed as { artifact?: unknown }).artifact !== null &&
          typeof (parsed as { export?: unknown }).export === "object" &&
          (parsed as { export?: unknown }).export !== null
        ) {
          const artifact = (parsed as {
            artifact: {
              title?: string;
              file_path?: string | null;
              byte_size?: number | null;
            };
            export: {
              row_count?: number;
              format?: string;
              columns?: string[];
            };
            task?: {
              id?: number;
              status?: string;
            } | null;
          });
          return (
            <div className="chat-tool-summary agent-artifact-result">
              <div className="chat-tool-field">
                <span className="chat-tool-field-key">产物</span>
                <span className="chat-tool-field-value">
                  {artifact.artifact.title ?? "导出结果"}
                </span>
              </div>
              <div className="chat-tool-field">
                <span className="chat-tool-field-key">文件</span>
                <span className="chat-tool-field-value mono">
                  {artifact.artifact.file_path ?? "未记录路径"}
                </span>
              </div>
              <div className="chat-tool-field">
                <span className="chat-tool-field-key">内容</span>
                <span className="chat-tool-field-value">
                  {(artifact.export.row_count ?? 0).toLocaleString()} 行 · {(artifact.export.columns ?? []).length} 列 · {artifact.export.format ?? "文件"}
                </span>
              </div>
              {artifact.task?.id ? (
                <div className="chat-tool-field">
                  <span className="chat-tool-field-key">任务</span>
                  <span className="chat-tool-field-value">
                    #{artifact.task.id} · {artifact.task.status ?? "completed"}
                  </span>
                </div>
              ) : null}
            </div>
          );
        }
        if (
          Array.isArray((parsed as { columns?: unknown }).columns) &&
          Array.isArray((parsed as { sample_rows?: unknown }).sample_rows)
        ) {
          const preview = parsed as {
            row_count?: number;
            columns: string[];
            sample_rows: string[][];
          };
          return (
            <div className="chat-tool-summary export-preview-summary">
              <div className="chat-tool-field">
                <span className="chat-tool-field-key">匹配行数</span>
                <span className="chat-tool-field-value">
                  {(preview.row_count ?? 0).toLocaleString()}
                </span>
              </div>
              <div className="chat-tool-field">
                <span className="chat-tool-field-key">导出列</span>
                <span className="chat-tool-field-value">
                  {preview.columns.join(" / ")}
                </span>
              </div>
              {preview.sample_rows.length > 0 ? (
                <div className="chat-md-table-wrap">
                  <table className="chat-md-table">
                    <thead>
                      <tr>
                        {preview.columns.map((column) => (
                          <th key={column}>{column}</th>
                        ))}
                      </tr>
                    </thead>
                    <tbody>
                      {preview.sample_rows.slice(0, 5).map((row, rowIndex) => (
                        <tr key={rowIndex}>
                          {preview.columns.map((column, columnIndex) => (
                            <td key={column}>{row[columnIndex] ?? ""}</td>
                          ))}
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              ) : null}
            </div>
          );
        }
        const entries = Object.entries(parsed).slice(0, 8);
        const more = Object.keys(parsed).length > 8;
        return (
          <div className="chat-tool-summary">
            {entries.map(([key, value]) => (
              <div key={key} className="chat-tool-field">
                <span className="chat-tool-field-key">{formatFieldName(key)}</span>
                <span className="chat-tool-field-value">
                  {formatToolValue(value)}
                </span>
              </div>
            ))}
            {more && <span className="chat-tool-more">还有更多字段</span>}
          </div>
        );
      }
      if (Array.isArray(parsed) && parsed.length > 0) {
        return (
          <div className="chat-tool-summary">
            {parsed.slice(0, 3).map((item, i) => (
              <div key={i} className="chat-tool-field">
                {typeof item === "object" ? JSON.stringify(item).slice(0, 60) : String(item)}
              </div>
            ))}
            {parsed.length > 3 && <span className="chat-tool-more">共 {parsed.length} 条结果</span>}
          </div>
        );
      }
      return <span>{content.slice(0, 300)}</span>;
    } catch {
      return <span>{content.length > 200 ? content.slice(0, 200) + "..." : content}</span>;
    }
  };

  const activeSession = sessions.find((s) => s.id === activeSessionId);

  const renderSessionItem = (s: AgentSession) => {
    const isActive = s.id === activeSessionId;
    const isRenaming = sessionRenameId === s.id;
    return (
      <div
        key={s.id}
        className={`agent-session-item ${isActive ? "active" : ""}`}
        onClick={() => !isRenaming && setActiveSessionId(s.id)}
      >
        {isRenaming ? (
          <input
            className="agent-session-rename-input"
            value={sessionRenameValue}
            onChange={(e) => setSessionRenameValue(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleRenameSessionSave(s.id);
              if (e.key === "Escape") handleRenameSessionCancel();
            }}
            onBlur={() => handleRenameSessionSave(s.id)}
            onClick={(e) => e.stopPropagation()}
            autoFocus
          />
        ) : (
          <>
            <div className="agent-session-content">
              <span className="agent-session-title">{s.title || "新对话"}</span>
              <span className="agent-session-time">{formatRelativeTime(s.updated_at)}</span>
            </div>
            <div className="agent-session-actions">
              <button
                className="agent-session-action-btn"
                onClick={(e) => {
                  e.stopPropagation();
                  handleRenameSession(s.id, s.title || "");
                }}
                title="重命名"
              >
                ✎
              </button>
              <button
                className="agent-session-action-btn agent-session-delete"
                onClick={(e) => {
                  e.stopPropagation();
                  handleDeleteSession(s.id);
                }}
                title="删除"
              >
                ×
              </button>
            </div>
          </>
        )}
      </div>
    );
  };

  const renderSessionGroup = (label: string, items: AgentSession[]) => {
    if (items.length === 0) return null;
    return (
      <div className="agent-session-group">
        <div className="agent-session-group-label">{label}</div>
        {items.map(renderSessionItem)}
      </div>
    );
  };

  return (
    <div className="page agent-page">
      {/* Session sidebar */}
      <div className="agent-sessions">
        <div className="agent-sessions-header">
          <button className="agent-new-session-btn" onClick={handleNewSession}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
            新建对话
          </button>
        </div>
        <div className="agent-search-wrap">
          <input
            className="agent-search-input"
            type="text"
            placeholder="搜索对话..."
            value={sessionSearch}
            onChange={(e) => setSessionSearch(e.target.value)}
          />
        </div>
        <div className="agent-session-list">
          {renderSessionGroup("今天", groupedSessions.today)}
          {renderSessionGroup("最近7天", groupedSessions.week)}
          {renderSessionGroup("更早", groupedSessions.older)}
          {filteredSessions.length === 0 && (
            <p className="muted" style={{ padding: 12 }}>
              {sessions.length === 0 ? "暂无对话记录" : "没有匹配的对话"}
            </p>
          )}
        </div>
      </div>

      {/* Chat area */}
      <div className="agent-chat">
        <div className="agent-chat-body">
          {activeSessionId === null ? (
            <div className="agent-chat-empty">
              <div className="agent-empty-hero">
                <h3 className="agent-empty-title">发票 Agent</h3>
                <p className="agent-empty-subtitle">
                  基于你的发票库，我可以帮助你搜索、统计、筛选、导出和检查风险
                </p>
              </div>
              <div className="agent-task-grid">
                {TASK_CARDS.map((card) => (
                  <button
                    key={card.label}
                    className="agent-task-card"
                    onClick={() => {
                      setInput(card.query);
                      handleNewSession();
                    }}
                  >
                    <span className="agent-task-card-label">{card.label}</span>
                  </button>
                ))}
              </div>
            </div>
          ) : (
            <>
              <div className="agent-chat-session-header">
                <h4 className="agent-chat-session-title">{activeSession?.title || "新对话"}</h4>
                <span className="agent-chat-session-hint">可以搜索、统计、筛选、导出和检查风险</span>
              </div>
              <div className="chat-messages">
                {messages.length === 0 && !loading && (
                  <div className="agent-chat-empty">
                    <p className="muted">发送消息开始对话</p>
                  </div>
                )}
                {messages.map((msg, index) => {
                  // Skip tool messages — they are merged into the preceding tool-call card
                  if (msg.role === "tool") return null;

                  const isStreamingAssistant =
                    streamState?.assistantMessageId === msg.id;
                  const isToolCall = msg.role === "assistant" && !!msg.tool_call_json;

                  // Find the paired tool result message
                  let toolResultMsg: AgentMessage | null = null;
                  if (isToolCall) {
                    for (let j = index + 1; j < messages.length; j++) {
                      if (messages[j].role === "tool") {
                        toolResultMsg = messages[j];
                        break;
                      }
                      if (messages[j].role !== "tool") break;
                    }
                  }

                  const isCollapsed = !collapsedTools.has(msg.id);

                  return (
                    <div key={msg.id} className={`chat-message chat-message-${msg.role}`}>
                      <div className="chat-bubble">
                        {msg.role === "assistant" ? (
                          <div className="chat-card-header">
                            <span>
                              {isToolCall
                                ? "工具调用"
                                : isStreamingAssistant
                                  ? streamState?.phase === "error"
                                    ? "回复中断"
                                    : "正在回复"
                                  : "回复"}
                            </span>
                          </div>
                        ) : null}
                        {isToolCall && (
                          <div
                            className="chat-tool-call"
                            role="button"
                            tabIndex={0}
                            onClick={() =>
                              setCollapsedTools((prev) => {
                                const next = new Set(prev);
                                if (next.has(msg.id)) next.delete(msg.id);
                                else next.add(msg.id);
                                return next;
                              })
                            }
                            onKeyDown={(e) => {
                              if (e.key === "Enter" || e.key === " ") {
                                e.preventDefault();
                                setCollapsedTools((prev) => {
                                  const next = new Set(prev);
                                  if (next.has(msg.id)) next.delete(msg.id);
                                  else next.add(msg.id);
                                  return next;
                                });
                              }
                            }}
                          >
                            <span className="chat-tool-call-dot" />
                            <span className="chat-tool-call-name">{toolCallSummary(msg)}</span>
                            <svg
                              className={`chat-tool-toggle ${isCollapsed ? "is-collapsed" : ""}`}
                              width="12"
                              height="12"
                              viewBox="0 0 24 24"
                              fill="none"
                              stroke="currentColor"
                              strokeWidth="2.5"
                              strokeLinecap="round"
                              strokeLinejoin="round"
                            >
                              <polyline points="6 9 12 15 18 9" />
                            </svg>
                          </div>
                        )}
                        {isToolCall && toolResultMsg && isCollapsed && (
                          <div className="chat-tool-result">
                            <div className="chat-tool-result-body">
                              {formatToolResult(toolResultMsg.content)}
                            </div>
                          </div>
                        )}
                        {msg.attachments && msg.attachments.length > 0 ? (
                          <AttachmentList attachments={msg.attachments} />
                        ) : null}
                        {!isToolCall && (
                          isStreamingAssistant ? (
                            <StreamingAssistantContent
                              content={msg.content}
                              phase={streamState!.phase}
                              toolName={streamState!.toolName}
                              errorMessage={streamState!.errorMessage}
                              onDismiss={() => setStreamState(null)}
                            />
                          ) : (
                            <MarkdownContent content={msg.content} />
                          )
                        )}
                      </div>
                    </div>
                  );
                })}

                {/* Confirmation panel */}
                {pendingConfirm && (
                  <div className="chat-message chat-message-assistant">
                    <div className="confirmation-panel">
                      <p className="confirmation-title">
                        确认操作：{toolLabel(pendingConfirm.tool_name)}
                      </p>
                      <p className="confirmation-detail">{pendingConfirm.message}</p>
                      <div className="confirmation-actions">
                        <button
                          className="btn-primary"
                          onClick={() => handleConfirm(true)}
                          disabled={loading}
                        >
                          确认执行
                        </button>
                        <button
                          className="btn-secondary"
                          onClick={() => handleConfirm(false)}
                          disabled={loading}
                        >
                          取消
                        </button>
                      </div>
                    </div>
                  </div>
                )}

                <div ref={messagesEnd} />
              </div>
            </>
          )}
        </div>

        {/* Input bar - always visible */}
        <div className="chat-input-wrap">
          {pendingAttachments.length > 0 ? (
            <div className="chat-pending-attachments">
              {pendingAttachments.map((attachment) => (
                <span className="chat-attachment-chip" key={attachment.id}>
                  {attachment.original_name}
                  <button
                    type="button"
                    onClick={() => setPendingAttachments((prev) => prev.filter((item) => item.id !== attachment.id))}
                    title="移除附件"
                  >
                    ×
                  </button>
                </span>
              ))}
            </div>
          ) : null}
          <div className="chat-input-bar">
            <button
              className="btn-secondary chat-attach-btn"
              type="button"
              onClick={handleAttach}
              disabled={loading}
              title="上传 xlsx/csv 表格"
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48"/></svg>
            </button>
            <textarea
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="问我任何关于发票的问题…"
              rows={2}
              disabled={loading}
              spellCheck={false}
            />
            <button
              className="btn-primary chat-send-btn"
              onClick={handleSend}
              disabled={loading || (!input.trim() && pendingAttachments.length === 0)}
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><line x1="22" y1="2" x2="11" y2="13"/><polygon points="22 2 15 22 11 13 2 9 22 2"/></svg>
            </button>
          </div>
        </div>
      </div>

      {activeSessionId !== null ? (
        <ArtifactPanel
          artifacts={artifacts}
          onOpen={handleOpenArtifact}
          onOpenFolder={handleOpenArtifactFolder}
          onCopy={copyText}
          onDelete={handleDeleteArtifact}
        />
      ) : null}
    </div>
  );
}

function formatRelativeTime(dateStr: string): string {
  const date = new Date(dateStr);
  if (Number.isNaN(date.getTime())) return dateStr;
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const target = new Date(date.getFullYear(), date.getMonth(), date.getDate());
  const diffDays = Math.floor((today.getTime() - target.getTime()) / 86400000);
  if (diffDays === 0) return "今天";
  if (diffDays === 1) return "昨天";
  if (diffDays < 7) return `${diffDays}天前`;
  return `${date.getMonth() + 1}-${String(date.getDate()).padStart(2, "0")}`;
}

function groupSessions(sessions: AgentSession[]): {
  today: AgentSession[];
  week: AgentSession[];
  older: AgentSession[];
} {
  const now = new Date();
  const todayStart = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const weekStart = todayStart - 6 * 86400000;
  const today: AgentSession[] = [];
  const week: AgentSession[] = [];
  const older: AgentSession[] = [];
  for (const s of sessions) {
    const t = new Date(s.updated_at).getTime();
    if (t >= todayStart) today.push(s);
    else if (t >= weekStart) week.push(s);
    else older.push(s);
  }
  return { today, week, older };
}

function createStreamId(): string {
  return `agent-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

function createTempMessage(
  sessionId: number,
  role: string,
  content: string,
  attachments: AgentAttachment[] = [],
  id = -Date.now(),
): AgentMessage {
  return {
    id,
    session_id: sessionId,
    role,
    content,
    tool_call_json: null,
    tool_call_id: null,
    created_at: new Date().toISOString(),
    attachments,
  };
}

function appendUniqueMessages(
  current: AgentMessage[],
  next: AgentMessage[],
): AgentMessage[] {
  const seen = new Set(current.map((msg) => msg.id));
  const additions = next.filter((msg) => {
    if (seen.has(msg.id)) return false;
    seen.add(msg.id);
    return true;
  });
  return [...current, ...additions];
}

function StreamingAssistantContent({
  content,
  phase,
  toolName,
  errorMessage,
  onDismiss,
}: {
  content: string;
  phase: StreamUiState["phase"];
  toolName: string | null;
  errorMessage: string | null;
  onDismiss: () => void;
}) {
  return (
    <div className="streaming-assistant">
      {content ? <MarkdownContent content={content} /> : null}
      {phase === "error" ? (
        <div className="streaming-error">
          <div className="streaming-error-message">
            {formatErrorMessage(errorMessage)}
          </div>
          <button className="streaming-error-dismiss" onClick={onDismiss}>
            关闭
          </button>
        </div>
      ) : (
        <div className={`streaming-status streaming-status-${phase}`}>
          <span className="thinking-orbit" aria-hidden="true">
            <span />
            <span />
            <span />
          </span>
          <span>{streamingStatusText(phase, toolName)}</span>
          {phase === "answering" ? <span className="typing-caret" /> : null}
        </div>
      )}
    </div>
  );
}

function streamingStatusText(
  phase: StreamUiState["phase"],
  toolName: string | null,
): string {
  if (phase === "tool" && toolName) return `正在使用 ${toolLabel(toolName)}`;
  if (phase === "answering") return "正在生成";
  if (phase === "error") return "回复中断";
  return "思考中";
}

function formatErrorMessage(raw: string | null): string {
  if (!raw) return "请求失败，请稍后重试";
  const statusMatch = raw.match(/HTTP\s+(\d{3})/);
  if (statusMatch) {
    const status = statusMatch[1];
    if (status === "429") return "请求过于频繁或配额不足，请稍后重试";
    if (status === "401" || status === "403") return "API Key 无效或权限不足，请检查设置";
    if (status === "404") return "模型服务地址不可达，请检查设置";
    if (status === "500" || status === "502" || status === "503") return "模型服务暂时不可用，请稍后重试";
  }
  if (raw.includes("timeout") || raw.includes("timed out")) return "请求超时，请稍后重试";
  if (raw.includes("connection")) return "无法连接到模型服务，请检查网络和设置";
  return "请求失败，请稍后重试";
}

function toolLabel(name: string): string {
  const labels: Record<string, string> = {
    search_invoices: "搜索发票",
    get_invoice_detail: "获取发票详情",
    get_dashboard_stats: "获取统计数据",
    get_current_date_context: "获取日期上下文",
    get_invoice_field_catalog: "获取字段字典",
    list_message_attachments: "查看附件",
    inspect_spreadsheet: "检查表格",
    create_export_preview: "预览导出",
    export_invoices: "导出发票",
    export_invoices_with_template: "按模板导出",
    update_invoice: "更新发票",
  };
  return labels[name] ?? name;
}

function artifactTypeLabel(type: string): string {
  const labels: Record<string, string> = {
    export: "文件导出",
    csv: "CSV 导出",
    xlsx: "Excel 导出",
    search: "搜索结果",
  };
  return labels[type] ?? type;
}

function ArtifactPanel({
  artifacts,
  onOpen,
  onOpenFolder,
  onCopy,
  onDelete,
}: {
  artifacts: AgentArtifact[];
  onOpen: (artifactId: number) => void;
  onOpenFolder: (artifactId: number) => void;
  onCopy: (value: string) => void;
  onDelete: (artifactId: number) => void;
}) {
  return (
    <aside className="agent-artifacts">
      <div className="agent-artifacts-header">
        <h3>结果</h3>
        <span>{artifacts.length}</span>
      </div>
      <div className="agent-artifact-list">
        {artifacts.map((artifact) => {
          const metadata = parseArtifactMetadata(artifact.metadata_json);
          return (
            <div className="agent-artifact-card" key={artifact.id}>
              <div className="agent-artifact-title-row">
                <strong title={artifact.title}>{artifact.title}</strong>
                <span className="mini-tag tag-recognized">
                  {artifactTypeLabel(artifact.artifact_type)}
                </span>
              </div>
              <div className="agent-artifact-meta">
                {metadata.row_count != null ? (
                  <span>{metadata.row_count.toLocaleString()} 行</span>
                ) : null}
                {metadata.columns?.length ? (
                  <span>{metadata.columns.length} 列</span>
                ) : null}
                {artifact.byte_size != null ? (
                  <span>{formatFileSize(artifact.byte_size)}</span>
                ) : null}
              </div>
              {artifact.file_path ? (
                <div className="agent-artifact-actions">
                  <button type="button" className="agent-artifact-icon-btn icon-btn-open" onClick={() => onOpen(artifact.id)} title="打开文件">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
                  </button>
                  <button type="button" className="agent-artifact-icon-btn icon-btn-folder" onClick={() => onOpenFolder(artifact.id)} title="所在目录">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
                  </button>
                  <button type="button" className="agent-artifact-icon-btn icon-btn-copy" onClick={() => onCopy(artifact.file_path!)} title="复制路径">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>
                  </button>
                  <button type="button" className="agent-artifact-icon-btn icon-btn-delete" onClick={() => onDelete(artifact.id)} title="删除">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
                  </button>
                </div>
              ) : (
                <div className="agent-artifact-actions">
                  <button type="button" className="agent-artifact-icon-btn icon-btn-delete" onClick={() => onDelete(artifact.id)} title="删除">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
                  </button>
                </div>
              )}
            </div>
          );
        })}
        {artifacts.length === 0 ? (
          <p className="muted agent-artifacts-empty">暂无导出结果</p>
        ) : null}
      </div>
    </aside>
  );
}

function parseArtifactMetadata(value: string | null): {
  row_count?: number;
  format?: string;
  columns?: string[];
} {
  if (!value) return {};
  try {
    const parsed = JSON.parse(value) as {
      row_count?: unknown;
      format?: unknown;
      columns?: unknown;
    };
    return {
      row_count: typeof parsed.row_count === "number" ? parsed.row_count : undefined,
      format: typeof parsed.format === "string" ? parsed.format : undefined,
      columns: Array.isArray(parsed.columns)
        ? parsed.columns.filter((item): item is string => typeof item === "string")
        : undefined,
    };
  } catch {
    return {};
  }
}

function fallbackCopyText(value: string): boolean {
  const textarea = document.createElement("textarea");
  textarea.value = value;
  textarea.setAttribute("readonly", "true");
  textarea.style.position = "fixed";
  textarea.style.left = "-9999px";
  textarea.style.top = "0";
  document.body.appendChild(textarea);
  textarea.select();
  let copied = false;
  try {
    copied = document.execCommand("copy");
  } finally {
    document.body.removeChild(textarea);
  }
  return copied;
}

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function AttachmentList({ attachments }: { attachments: AgentAttachment[] }) {
  return (
    <div className="chat-attachment-list">
      {attachments.map((attachment) => (
        <span className="chat-attachment-chip" key={attachment.id}>
          {attachment.original_name}
        </span>
      ))}
    </div>
  );
}

type MarkdownBlock =
  | { type: "paragraph"; lines: string[] }
  | { type: "heading"; level: number; text: string }
  | { type: "list"; ordered: boolean; items: string[] }
  | { type: "quote"; lines: string[] }
  | { type: "code"; language: string | null; code: string }
  | { type: "table"; header: string[]; rows: string[][] }
  | { type: "rule" };

function MarkdownContent({ content }: { content: string }) {
  const blocks = React.useMemo(() => parseMarkdown(content), [content]);

  return (
    <div className="chat-content chat-markdown">
      {blocks.map((block, index) => renderMarkdownBlock(block, index))}
    </div>
  );
}

function parseMarkdown(content: string): MarkdownBlock[] {
  const lines = content.replace(/\r\n/g, "\n").split("\n");
  const blocks: MarkdownBlock[] = [];
  let paragraph: string[] = [];
  let index = 0;

  const flushParagraph = () => {
    if (paragraph.length > 0) {
      blocks.push({ type: "paragraph", lines: paragraph });
      paragraph = [];
    }
  };

  while (index < lines.length) {
    const line = lines[index];
    const trimmed = line.trim();

    if (!trimmed) {
      flushParagraph();
      index += 1;
      continue;
    }

    const fence = trimmed.match(/^```([A-Za-z0-9_-]+)?\s*$/);
    if (fence) {
      flushParagraph();
      const language = fence[1] ?? null;
      const codeLines: string[] = [];
      index += 1;
      while (index < lines.length && !lines[index].trim().startsWith("```")) {
        codeLines.push(lines[index]);
        index += 1;
      }
      if (index < lines.length) index += 1;
      blocks.push({ type: "code", language, code: codeLines.join("\n") });
      continue;
    }

    if (isTableStart(lines, index)) {
      flushParagraph();
      const header = splitTableRow(lines[index]);
      const rows: string[][] = [];
      index += 2;
      while (index < lines.length && lines[index].includes("|") && lines[index].trim()) {
        rows.push(splitTableRow(lines[index]));
        index += 1;
      }
      blocks.push({ type: "table", header, rows });
      continue;
    }

    const heading = trimmed.match(/^(#{1,4})\s+(.+)$/);
    if (heading) {
      flushParagraph();
      blocks.push({
        type: "heading",
        level: heading[1].length,
        text: heading[2].trim(),
      });
      index += 1;
      continue;
    }

    if (/^[-*_]{3,}$/.test(trimmed)) {
      flushParagraph();
      blocks.push({ type: "rule" });
      index += 1;
      continue;
    }

    const listMatch = trimmed.match(/^((?:[-*+])|\d+\.)\s+(.+)$/);
    if (listMatch) {
      flushParagraph();
      const ordered = /\d+\./.test(listMatch[1]);
      const items: string[] = [];
      while (index < lines.length) {
        const match = lines[index].trim().match(/^((?:[-*+])|\d+\.)\s+(.+)$/);
        if (!match || /\d+\./.test(match[1]) !== ordered) break;
        items.push(match[2]);
        index += 1;
      }
      blocks.push({ type: "list", ordered, items });
      continue;
    }

    if (trimmed.startsWith(">")) {
      flushParagraph();
      const quoteLines: string[] = [];
      while (index < lines.length && lines[index].trim().startsWith(">")) {
        quoteLines.push(lines[index].trim().replace(/^>\s?/, ""));
        index += 1;
      }
      blocks.push({ type: "quote", lines: quoteLines });
      continue;
    }

    paragraph.push(line);
    index += 1;
  }

  flushParagraph();
  return blocks;
}

function renderMarkdownBlock(block: MarkdownBlock, key: number): React.ReactNode {
  switch (block.type) {
    case "heading": {
      const children = renderInline(block.text);
      if (block.level === 1) {
        return <h3 key={key} className="chat-md-heading">{children}</h3>;
      }
      if (block.level === 2) {
        return <h4 key={key} className="chat-md-heading">{children}</h4>;
      }
      if (block.level === 3) {
        return <h5 key={key} className="chat-md-heading">{children}</h5>;
      }
      return <h6 key={key} className="chat-md-heading">{children}</h6>;
    }
    case "list": {
      const ListTag = block.ordered ? "ol" : "ul";
      return (
        <ListTag key={key} className="chat-md-list">
          {block.items.map((item, itemIndex) => (
            <li key={itemIndex}>{renderInline(item)}</li>
          ))}
        </ListTag>
      );
    }
    case "quote":
      return (
        <blockquote key={key} className="chat-md-quote">
          {renderLines(block.lines)}
        </blockquote>
      );
    case "code":
      return (
        <div key={key} className="chat-md-code-block">
          {block.language ? <div className="chat-md-code-lang">{block.language}</div> : null}
          <pre>
            <code>{block.code}</code>
          </pre>
        </div>
      );
    case "table":
      return (
        <div key={key} className="chat-md-table-wrap">
          <table className="chat-md-table">
            <thead>
              <tr>
                {block.header.map((cell, cellIndex) => (
                  <th key={cellIndex}>{renderInline(cell)}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {block.rows.map((row, rowIndex) => (
                <tr key={rowIndex}>
                  {block.header.map((_, cellIndex) => (
                    <td key={cellIndex}>{renderInline(row[cellIndex] ?? "")}</td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      );
    case "rule":
      return <hr key={key} className="chat-md-rule" />;
    case "paragraph":
    default:
      return (
        <p key={key} className="chat-md-paragraph">
          {renderLines(block.lines)}
        </p>
      );
  }
}

function renderLines(lines: string[]): React.ReactNode[] {
  return lines.flatMap((line, index) => {
    const nodes = renderInline(line);
    return index === lines.length - 1 ? nodes : [...nodes, <br key={`br-${index}`} />];
  });
}

function renderInline(text: string): React.ReactNode[] {
  const nodes: React.ReactNode[] = [];
  const pattern = /(`[^`]+`|\*\*[^*]+\*\*|\*[^*]+\*|\[[^\]]+\]\([^)]+\))/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;
  let key = 0;

  while ((match = pattern.exec(text)) !== null) {
    if (match.index > lastIndex) {
      nodes.push(text.slice(lastIndex, match.index));
    }

    const token = match[0];
    if (token.startsWith("`")) {
      nodes.push(<code key={key++}>{token.slice(1, -1)}</code>);
    } else if (token.startsWith("**")) {
      nodes.push(<strong key={key++}>{renderInline(token.slice(2, -2))}</strong>);
    } else if (token.startsWith("*")) {
      nodes.push(<em key={key++}>{renderInline(token.slice(1, -1))}</em>);
    } else {
      const link = token.match(/^\[([^\]]+)\]\(([^)]+)\)$/);
      if (link && isSafeLink(link[2])) {
        nodes.push(
          <a key={key++} href={link[2]} target="_blank" rel="noreferrer">
            {renderInline(link[1])}
          </a>,
        );
      } else {
        nodes.push(token);
      }
    }

    lastIndex = match.index + token.length;
  }

  if (lastIndex < text.length) {
    nodes.push(text.slice(lastIndex));
  }

  return nodes.length > 0 ? nodes : [text];
}

function isSafeLink(url: string): boolean {
  return /^(https?:|mailto:)/i.test(url);
}

function isTableStart(lines: string[], index: number): boolean {
  return (
    index + 1 < lines.length &&
    lines[index].includes("|") &&
    splitTableRow(lines[index]).length >= 2 &&
    splitTableRow(lines[index + 1]).every((cell) => /^:?-{3,}:?$/.test(cell))
  );
}

function splitTableRow(line: string): string[] {
  return line
    .trim()
    .replace(/^\|/, "")
    .replace(/\|$/, "")
    .split("|")
    .map((cell) => cell.trim());
}

function formatToolValue(value: unknown): string {
  if (value === null || value === undefined) return "无";
  if (typeof value === "number") return value.toLocaleString();
  if (typeof value === "boolean") return value ? "是" : "否";
  if (typeof value === "string") return value.length > 80 ? value.slice(0, 80) + "..." : value;
  if (Array.isArray(value)) {
    const items = value.slice(0, 3).map((v) => formatToolValue(v));
    const suffix = value.length > 3 ? ` 等${value.length}项` : "";
    return items.join("、") + suffix;
  }
  if (typeof value === "object") {
    const entries = Object.entries(value).slice(0, 3);
    const parts = entries.map(([k, v]) => `${formatFieldName(k)}: ${formatToolValue(v)}`);
    const suffix = Object.keys(value).length > 3 ? "..." : "";
    return parts.join("，") + suffix;
  }
  return String(value);
}

function formatFieldName(key: string): string {
  const names: Record<string, string> = {
    total_count: "总数",
    page: "页码",
    page_size: "每页条数",
    total_pages: "总页数",
    total_invoices: "发票总数",
    total_amount: "金额",
    this_month_count: "本月数量",
    this_month_amount: "本月金额",
    invoices: "发票列表",
    file_path: "文件路径",
    row_count: "行数",
    byte_size: "文件大小",
    format: "格式",
    currency: "币种",
    average_confidence: "平均置信度",
    pending_count: "待处理数",
    duplicate_count: "重复数",
    seller_name: "销售方",
    invoice_number: "发票号",
    issue_date: "日期",
    category: "类别",
    status: "状态",
    year: "年份",
    month: "月份",
    today: "今天",
    current_month: "当前月份",
    current_year: "当前年份",
    current_month_name: "月份名称",
    current_quarter: "当前季度",
    quarter_start: "季度起始",
    quarter_end: "季度结束",
    first_day_of_month: "月初",
    last_day_of_month: "月末",
    search_query: "搜索词",
    filters: "筛选条件",
    columns: "列",
    sample_rows: "样例行",
    seller: "销售方",
    buyer: "购买方",
    invoice_type: "发票类型",
    tax_amount: "税额",
    amount_without_tax: "不含税金额",
    amount_with_tax: "价税合计",
    confidence: "置信度",
    raw_file_id: "原始文件",
    export: "导出",
    artifact: "产物",
    task: "任务",
    title: "标题",
    id: "ID",
  };
  return names[key] ?? key;
}

export default AgentPage;
