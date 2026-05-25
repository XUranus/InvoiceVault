import React from "react";
import type { AgentAttachment } from "../../../types";
import { useAgentStore } from "../hooks/useAgentStore";
import { FileText, X } from "lucide-react";

interface AttachmentBarProps {
  attachments: AgentAttachment[];
}

export function AttachmentBar({ attachments }: AttachmentBarProps) {
  const removeAttachment = useAgentStore((s) => s.removeAttachment);

  if (attachments.length === 0) return null;

  return (
    <div className="flex flex-wrap gap-2">
      {attachments.map((attachment) => (
        <div
          key={attachment.id}
          className="flex items-center gap-2 px-3 py-2 rounded-md text-xs"
          style={{
            backgroundColor: "var(--color-surface-subtle)",
            border: "1px solid var(--color-border)",
          }}
        >
          <FileText
            className="w-4 h-4"
            style={{ color: "var(--color-text-muted)" }}
          />
          <span style={{ color: "var(--color-text-secondary)" }}>
            {attachment.original_name}
          </span>
          <button
            onClick={() => removeAttachment(attachment.id)}
            className="ml-1 hover:opacity-70 transition-opacity"
          >
            <X
              className="w-3 h-3"
              style={{ color: "var(--color-text-muted)" }}
            />
          </button>
        </div>
      ))}
    </div>
  );
}
