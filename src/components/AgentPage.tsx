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
                      <MarkdownContent content={msg.content} />
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
