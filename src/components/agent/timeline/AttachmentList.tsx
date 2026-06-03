import React from "react";
import type { AgentAttachment } from "../../../types";
import { FileText, X } from "lucide-react";

interface AttachmentListProps {
  attachments: AgentAttachment[];
  onRemove?: (id: number) => void;
}

export function AttachmentList({ attachments, onRemove }: AttachmentListProps) {
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
          <span
            className="text-xs"
            style={{ color: "var(--color-text-muted)" }}
          >
            ({formatBytes(attachment.byte_size)})
          </span>
          {onRemove && (
            <button
              onClick={() => onRemove(attachment.id)}
              aria-label={`移除附件 ${attachment.original_name}`}
              className="ml-1 hover:opacity-70 transition-opacity"
            >
              <X
                className="w-3 h-3"
                style={{ color: "var(--color-text-muted)" }}
              />
            </button>
          )}
        </div>
      ))}
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
}
