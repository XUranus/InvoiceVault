import React, { useEffect } from "react";
import { SessionSidebar } from "./panels/SessionSidebar";
import { Timeline } from "./timeline/Timeline";
import { ChatInput } from "./input/ChatInput";
import { ArtifactPanel } from "./panels/ArtifactPanel";
import { useAgentStore } from "./hooks/useAgentStore";
import { useAgentStream } from "./hooks/useAgentStream";
import { MessageSquare, Sparkles, Paperclip } from "lucide-react";

function AgentPage() {
  const messages = useAgentStore((s) => s.messages);
  const activeSessionId = useAgentStore((s) => s.activeSessionId);
  const sessions = useAgentStore((s) => s.sessions);
  const loading = useAgentStore((s) => s.loading);
  const { isActive } = useAgentStream();

  // Get current session title
  const currentSession = sessions.find((s) => s.id === activeSessionId);
  const sessionTitle = currentSession
    ? currentSession.title
    : activeSessionId
      ? `会话${activeSessionId}`
      : "AI 助手";

  return (
    <div className="flex h-full" style={{ backgroundColor: "var(--color-bg)" }}>
      {/* Session sidebar */}
      <SessionSidebar />

      {/* Main chat area */}
      <div className="flex-1 flex flex-col min-w-0">
        {/* Header */}
        <div
          className="flex items-center justify-between px-6 py-3 border-b"
          style={{
            borderColor: "var(--color-border)",
            backgroundColor: "var(--color-surface)",
          }}
        >
          <h2
            className="text-lg font-semibold"
            style={{ color: "var(--color-text)" }}
          >
            {sessionTitle}
          </h2>
          {isActive && (
            <div
              className="flex items-center gap-2 text-xs px-2 py-1 rounded"
              style={{
                backgroundColor: "var(--color-primary-bg)",
                color: "var(--color-primary-text)",
              }}
            >
              <div
                className="w-2 h-2 rounded-full animate-pulse"
                style={{ backgroundColor: "var(--color-primary)" }}
              />
              传输中...
            </div>
          )}
        </div>

        {activeSessionId !== null ? (
          <>
            {/* Timeline */}
            <Timeline messages={messages} />

            {/* Chat input */}
            <ChatInput />
          </>
        ) : (
          <div className="agent-chat-empty">
            <div className="agent-empty-hero">
              <MessageSquare
                size={48}
                style={{ color: "var(--color-primary)", marginBottom: 8 }}
              />
              <h2 className="agent-empty-title">AI 智能助手</h2>
              <p className="agent-empty-subtitle">
                选择左侧会话或创建新会话，开始与 AI 对话。<br />
                支持发票数据分析、文档处理、智能问答。
              </p>
            </div>
            <div
              className="flex gap-6 text-sm"
              style={{ color: "var(--color-text-secondary)" }}
            >
              <div className="flex items-center gap-2">
                <Sparkles size={16} style={{ color: "var(--color-primary)" }} />
                智能分析发票数据
              </div>
              <div className="flex items-center gap-2">
                <Paperclip size={16} style={{ color: "var(--color-primary)" }} />
                上传附件辅助问答
              </div>
              <div className="flex items-center gap-2">
                <MessageSquare size={16} style={{ color: "var(--color-primary)" }} />
                多会话并行管理
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Artifact panel */}
      <ArtifactPanel />
    </div>
  );
}

export default AgentPage;
