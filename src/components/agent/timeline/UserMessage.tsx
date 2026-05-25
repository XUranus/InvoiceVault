import React from "react";
import type { AgentMessage } from "../../../types";
import { AttachmentList } from "./AttachmentList";

interface UserMessageProps {
  message: AgentMessage;
}

export function UserMessage({ message }: UserMessageProps) {
  return (
    <div
      className="rounded-lg px-4 py-3"
      style={{
        backgroundColor: "var(--color-primary-bg)",
        border: "1px solid var(--color-primary)",
      }}
    >
      <div
        className="text-sm whitespace-pre-wrap"
        style={{ color: "var(--color-text)" }}
      >
        {message.content}
      </div>
      {message.attachments.length > 0 && (
        <div className="mt-2">
          <AttachmentList attachments={message.attachments} />
        </div>
      )}
    </div>
  );
}
