import React from "react";
import type { AgentMessage, PendingConfirmation } from "../../../types";
import { useAgentStore } from "../hooks/useAgentStore";
import { UserMessage } from "./UserMessage";
import { AssistantMessage } from "./AssistantMessage";
import { ToolCallCard } from "./ToolCallCard";
import { PendingConfirmationCard } from "./PendingConfirmationCard";

interface TimelineItemProps {
  message: AgentMessage;
  toolResultMap: Map<string, AgentMessage>;
}

function isPendingConfirmation(msg: AgentMessage): PendingConfirmation | null {
  if (msg.role !== "tool" || !msg.content) return null;
  try {
    const parsed = JSON.parse(msg.content);
    if (parsed.__pending_confirmation) {
      return {
        tool_name: parsed.tool_name,
        arguments: parsed.arguments || {},
        message: parsed.message,
        options: parsed.options,
      };
    }
  } catch { /* ignore */ }
  return null;
}

export function TimelineItem({ message, toolResultMap }: TimelineItemProps) {
  const confirmAction = useAgentStore((s) => s.confirmAction);
  const isUser = message.role === "user";
  const isAssistant = message.role === "assistant";
  const isToolResult = message.role === "tool";

  // Check if this is a pending confirmation (standalone card)
  const pending = isPendingConfirmation(message);
  if (pending) {
    return (
      <div className="relative pl-10 pb-2">
        <div
          className="absolute left-[11px] top-[8px] w-[10px] h-[10px] rounded-full border-2"
          style={{
            borderColor: "var(--color-warn, #d97706)",
            backgroundColor: "var(--color-warn-bg, #fef3c7)",
          }}
        />
        <PendingConfirmationCard
          pending={pending}
          onConfirm={(extraParams) => confirmAction(true, extraParams)}
          onCancel={() => confirmAction(false)}
        />
      </div>
    );
  }

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

  // Regular tool result messages are shown inside ToolCallCard, skip
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
                message={toolResult || undefined}
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
