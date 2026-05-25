import React from "react";
import type { AgentMessage } from "../../../types";
import { UserMessage } from "./UserMessage";
import { AssistantMessage } from "./AssistantMessage";
import { ToolCallCard } from "./ToolCallCard";

interface TimelineItemProps {
  message: AgentMessage;
  toolResultMap: Map<string, AgentMessage>;
}

export function TimelineItem({ message, toolResultMap }: TimelineItemProps) {
  const isUser = message.role === "user";
  const isAssistant = message.role === "assistant";
  const isToolResult = message.role === "tool";

  // Parse tool call from assistant message
  let toolCalls: Array<{ name: string; args: Record<string, unknown>; id?: string }> = [];
  if (isAssistant && message.tool_call_json) {
    try {
      const parsed = JSON.parse(message.tool_call_json);
      const calls = Array.isArray(parsed) ? parsed : [parsed];

      toolCalls = calls
        .filter((tc: any) => tc && tc.function)
        .map((tc: any) => ({
          id: tc.id,
          name: tc.function.name || "unknown",
          args: (() => {
            try {
              const argsStr = tc.function.arguments || "{}";
              return typeof argsStr === "string" ? JSON.parse(argsStr) : argsStr;
            } catch {
              return {};
            }
          })(),
        }));
    } catch (e) {
      console.error("Failed to parse tool_call_json:", e);
    }
  }

  const hasToolCalls = toolCalls.length > 0;

  // Tool result messages are already shown inside ToolCallCard, skip entirely
  if (isToolResult && !hasToolCalls) {
    return null;
  }

  return (
    <div className="relative pl-10 pb-2">
      {/* Timeline node */}
      <div
        className="absolute left-[11px] top-[8px] w-[10px] h-[10px] rounded-full border-2"
        style={{
          borderColor: isUser
            ? "var(--color-badge-blue-text)"
            : hasToolCalls || isToolResult
              ? "var(--color-success)"
              : "var(--color-badge-blue-text)",
          backgroundColor: isUser
            ? "var(--color-badge-blue-bg)"
            : hasToolCalls || isToolResult
              ? "var(--color-success-bg)"
              : "var(--color-badge-blue-bg)",
        }}
      />

      {/* Content */}
      {isUser ? (
        <UserMessage message={message} />
      ) : hasToolCalls ? (
        // Show tool calls from assistant message
        <div className="space-y-2">
          {toolCalls.map((toolCall, index) => {
            // Find the corresponding tool result message
            const toolResult = toolCall.id ? toolResultMap.get(toolCall.id) : undefined;

            return (
              <ToolCallCard
                key={index}
                toolCall={toolCall}
                message={toolResult || message}
              />
            );
          })}
        </div>
      ) : (
        <AssistantMessage message={message} />
      )}
    </div>
  );
}
