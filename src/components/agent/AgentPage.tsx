import React, { useEffect } from "react";
import { SessionSidebar } from "./panels/SessionSidebar";
import { Timeline } from "./timeline/Timeline";
import { ChatInput } from "./input/ChatInput";
import { ArtifactPanel } from "./panels/ArtifactPanel";
import { ConfirmationDialog } from "./panels/ConfirmationDialog";
import { useAgentStore } from "./hooks/useAgentStore";
import { useAgentStream } from "./hooks/useAgentStream";

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

        {/* Timeline */}
        <Timeline messages={messages} />

        {/* Chat input */}
        <ChatInput />
      </div>

      {/* Artifact panel */}
      <ArtifactPanel />

      {/* Confirmation dialog */}
      <ConfirmationDialog />
    </div>
  );
}

export default AgentPage;
