import React from "react";
import type { AgentSession, AgentMessage, PendingConfirmation } from "../types";
import {
  createAgentSession,
  listAgentSessions,
  getAgentSession,
  deleteAgentSession,
  sendAgentMessage,
  confirmAgentAction,
} from "../api";
import { open } from "@tauri-apps/plugin-dialog";

type Props = {
  llmBaseUrl: string;
  llmModel: string;
  llmApiKey: string;
  onError: (error: string) => void;
};

const EXAMPLE_QUESTIONS = [
  "本月发票总金额是多少？",
  "搜索办公用品相关的发票",
  "导出发票为 CSV 格式",
];

export function AgentPage({ llmBaseUrl, llmModel, llmApiKey, onError }: Props) {
  const [sessions, setSessions] = React.useState<AgentSession[]>([]);
  const [activeSessionId, setActiveSessionId] = React.useState<number | null>(null);
  const [messages, setMessages] = React.useState<AgentMessage[]>([]);
  const [input, setInput] = React.useState("");
  const [loading, setLoading] = React.useState(false);
  const [pendingConfirm, setPendingConfirm] = React.useState<PendingConfirmation | null>(null);
  const messagesEnd = React.useRef<HTMLDivElement>(null);

  const llmConfig = React.useMemo(
    () => ({
      base_url: llmBaseUrl,
      api_key: llmApiKey,
      model: llmModel,
      timeout_seconds: 60,
    }),
    [llmBaseUrl, llmModel, llmApiKey],
  );

  // Load sessions on mount
  React.useEffect(() => {
    listAgentSessions()
      .then(setSessions)
      .catch((err) => onError(String(err)));
  }, []);

  // Load messages when switching sessions
  React.useEffect(() => {
    if (activeSessionId === null) {
      setMessages([]);
      setPendingConfirm(null);
      return;
    }
    getAgentSession(activeSessionId)
      .then((msgs) => {
        setMessages(msgs);
        setPendingConfirm(null);
      })
      .catch((err) => onError(String(err)));
  }, [activeSessionId]);

  // Scroll to bottom on new messages
  React.useEffect(() => {
    messagesEnd.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const handleNewSession = async () => {
    try {
      const session = await createAgentSession();
      setSessions((prev) => [session, ...prev]);
      setActiveSessionId(session.id);
    } catch (err) {
      onError(String(err));
    }
  };

  const handleDeleteSession = async (id: number) => {
    try {
      await deleteAgentSession(id);
      setSessions((prev) => prev.filter((s) => s.id !== id));
      if (activeSessionId === id) {
        setActiveSessionId(null);
        setMessages([]);
      }
    } catch (err) {
      onError(String(err));
    }
  };

  const handleSend = async () => {
    const text = input.trim();
    if (!text || loading) return;
    if (!llmBaseUrl || !llmApiKey || !llmModel) {
      onError("请先在设置页配置 LLM Provider");
      return;
    }

    let sessionId = activeSessionId;
    if (sessionId === null) {
      try {
        const session = await createAgentSession();
        setSessions((prev) => [session, ...prev]);
        sessionId = session.id;
        setActiveSessionId(sessionId);
      } catch (err) {
        onError(String(err));
        return;
      }
    }

    setInput("");
    setLoading(true);

    try {
      const response = await sendAgentMessage(sessionId, text, llmConfig);
      setMessages((prev) => [...prev, ...response.messages]);
      setPendingConfirm(response.pending_confirmation);

      // Refresh session list (title may have updated)
      listAgentSessions()
        .then(setSessions)
        .catch(() => {});
    } catch (err) {
      onError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleConfirm = async (confirmed: boolean) => {
    if (activeSessionId === null || !pendingConfirm) return;

    let extraParams: Record<string, unknown> | null = null;

    // For export confirmations, open file picker
    if (confirmed && pendingConfirm.tool_name === "export_invoices") {
      try {
        const path = await open({
          title: "选择导出位置",
          defaultPath: `invoices_export.${pendingConfirm.arguments && (pendingConfirm.arguments as Record<string, unknown>).format === "xlsx" ? "xlsx" : "csv"}`,
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
        onError("文件选择失败");
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

    const confirm = pendingConfirm;
    setPendingConfirm(null);
    setLoading(true);

    try {
      const response = await confirmAgentAction(
        activeSessionId,
        confirmed,
        extraParams,
        llmConfig,
      );
      setMessages((prev) => [...prev, ...response.messages]);
      setPendingConfirm(response.pending_confirmation);

      // Refresh session list
      listAgentSessions()
        .then(setSessions)
        .catch(() => {});
    } catch (err) {
      onError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
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
        const entries = Object.entries(parsed).slice(0, 5);
        const more = Object.keys(parsed).length > 5;
        return (
          <div className="chat-tool-summary">
            {entries.map(([key, value]) => (
              <div key={key} className="chat-tool-field">
                <span className="chat-tool-field-key">{formatFieldName(key)}</span>
                <span className="chat-tool-field-value">
                  {typeof value === "number" ? value.toLocaleString() : String(value).slice(0, 80)}
                </span>
              </div>
            ))}
            {more && <span className="chat-tool-more">...</span>}
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

  return (
    <div className="page agent-page">
      {/* Session sidebar */}
      <div className="agent-sessions">
        <button className="btn-primary" onClick={handleNewSession} style={{ width: "100%" }}>
          + 新对话
        </button>
        <div className="agent-session-list">
          {sessions.map((s) => (
            <div
              key={s.id}
              className={`agent-session-item ${s.id === activeSessionId ? "active" : ""}`}
              onClick={() => setActiveSessionId(s.id)}
            >
              <span className="agent-session-title">
                {s.title || "新对话"}
              </span>
              <span className="agent-session-time">
                {s.updated_at.slice(0, 10)}
              </span>
              <button
                className="agent-session-delete"
                onClick={(e) => {
                  e.stopPropagation();
                  handleDeleteSession(s.id);
                }}
                title="删除会话"
              >
                ×
              </button>
            </div>
          ))}
          {sessions.length === 0 && (
            <p className="muted" style={{ padding: 12 }}>
              暂无对话记录
            </p>
          )}
        </div>
      </div>

      {/* Chat area */}
      <div className="agent-chat">
        {activeSessionId === null ? (
          <div className="agent-chat-empty">
            <h3>有什么可以帮助你的？</h3>
            <p className="muted">选择或创建一个会话，开始与 Agent 对话</p>
            <div className="example-questions">
              {EXAMPLE_QUESTIONS.map((q) => (
                <button
                  key={q}
                  className="example-question-btn"
                  onClick={() => {
                    setInput(q);
                    handleNewSession();
                  }}
                >
                  {q}
                </button>
              ))}
            </div>
          </div>
        ) : (
          <>
            <div className="chat-messages">
              {messages.length === 0 && !loading && (
                <div className="agent-chat-empty">
                  <p className="muted">发送消息开始对话</p>
                </div>
              )}
              {messages.map((msg) => (
                <div key={msg.id} className={`chat-message chat-message-${msg.role}`}>
                  <div className="chat-bubble">
                    {msg.role === "assistant" && msg.tool_call_json && (
                      <div className="chat-tool-call">{toolCallSummary(msg)}</div>
                    )}
                    {msg.role === "tool" ? (
                      <div className="chat-tool-result">
                        <span className="chat-tool-label">工具结果</span>
                        {formatToolResult(msg.content)}
                      </div>
                    ) : (
                      <div className="chat-content">{msg.content}</div>
                    )}
                  </div>
                </div>
              ))}

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

              {loading && (
                <div className="chat-message chat-message-assistant">
                  <div className="chat-bubble">
                    <span className="chat-loading">思考中...</span>
                  </div>
                </div>
              )}
              <div ref={messagesEnd} />
            </div>

            <div className="chat-input-bar">
              <textarea
                value={input}
                onChange={(e) => setInput(e.target.value)}
                onKeyDown={handleKeyDown}
                placeholder="输入消息，Enter 发送，Shift+Enter 换行"
                rows={2}
                disabled={loading}
                spellCheck={false}
              />
              <button
                className="btn-primary"
                onClick={handleSend}
                disabled={loading || !input.trim()}
              >
                发送
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

function toolLabel(name: string): string {
  const labels: Record<string, string> = {
    search_invoices: "搜索发票",
    get_invoice_detail: "获取发票详情",
    get_dashboard_stats: "获取统计数据",
    export_invoices: "导出发票",
    update_invoice: "更新发票",
  };
  return labels[name] ?? name;
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
  };
  return names[key] ?? key;
}
