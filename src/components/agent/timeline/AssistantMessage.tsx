import React from "react";
import type { AgentMessage } from "../../../types";
import { MarkdownRenderer } from "../shared/MarkdownRenderer";

interface AssistantMessageProps {
  message: AgentMessage;
}

export function AssistantMessage({ message }: AssistantMessageProps) {
  if (!message.content) return null;

  return (
    <div
      className="rounded-lg px-4 py-3"
      style={{
        backgroundColor: "var(--color-surface)",
        border: "1px solid var(--color-chat-assistant-border)",
      }}
    >
      <MarkdownRenderer content={message.content} />
    </div>
  );
}
