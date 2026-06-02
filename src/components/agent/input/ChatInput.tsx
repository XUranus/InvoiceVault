import React, { useState, useRef, useEffect } from "react";
import { motion } from "framer-motion";
import { Send, Paperclip } from "lucide-react";
import { pickAnyFiles } from "../../../api";
import { useAgentStore } from "../hooks/useAgentStore";
import { AttachmentBar } from "./AttachmentBar";

export function ChatInput() {
  const [message, setMessage] = useState("");
  const [isAttaching, setIsAttaching] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const sendMessage = useAgentStore((s) => s.sendMessage);
  const attachFile = useAgentStore((s) => s.attachFile);
  const pendingAttachments = useAgentStore((s) => s.pendingAttachments);
  const streamState = useAgentStore((s) => s.streamState);
  const activeSessionId = useAgentStore((s) => s.activeSessionId);

  const isStreaming =
    streamState !== null &&
    streamState.phase !== "done" &&
    streamState.phase !== "error";

  // Clear message when session changes
  useEffect(() => {
    setMessage("");
  }, [activeSessionId]);

  // Auto-resize textarea
  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
      textareaRef.current.style.height =
        Math.min(textareaRef.current.scrollHeight, 200) + "px";
    }
  }, [message]);

  const handleSend = async () => {
    if (!message.trim() || isStreaming || activeSessionId === null) return;
    const content = message.trim();
    setMessage("");
    await sendMessage(content);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const handleAttachFile = async () => {
    if (isAttaching) return;

    setIsAttaching(true);
    try {
      const paths = await pickAnyFiles();
      if (paths.length === 0) return;
      for (const path of paths) {
        await attachFile(path);
      }
    } catch (err) {
      console.error("Failed to attach file:", err);
    } finally {
      setIsAttaching(false);
    }
  };

  return (
    <div
      className="border-t px-4 pt-2 pb-3"
      style={{ borderColor: "var(--color-border)" }}
    >
      {/* Attachment bar */}
      {pendingAttachments.length > 0 && (
        <div className="mb-2">
          <AttachmentBar attachments={pendingAttachments} />
        </div>
      )}

      {/* Input area */}
      <div
        className="flex items-end gap-2 rounded-xl px-3 py-2"
        style={{
          backgroundColor: "var(--color-input-bg)",
          border: "1px solid var(--color-border)",
        }}
      >
        {/* Attach button */}
        <button
          onClick={handleAttachFile}
          disabled={isAttaching}
          className="p-1.5 rounded-lg transition-colors"
          style={{
            color: isAttaching ? "var(--color-text-muted)" : "var(--color-text-secondary)",
            opacity: isAttaching ? 0.5 : 1,
            cursor: isAttaching ? "wait" : "pointer",
          }}
          onMouseEnter={(e) => {
            if (!isAttaching)
              e.currentTarget.style.backgroundColor = "var(--color-surface-subtle)";
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.backgroundColor = "transparent";
          }}
        >
          <Paperclip className="w-4 h-4" />
        </button>

        {/* Textarea */}
        <textarea
          ref={textareaRef}
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={
            activeSessionId === null
              ? "请选择或创建会话开始..."
              : "输入消息..."
          }
          disabled={activeSessionId === null}
          className="flex-1 resize-none bg-transparent outline-none text-sm min-h-[24px] max-h-[200px]"
          style={{
            color: "var(--color-text)",
            caretColor: "var(--color-primary)",
          }}
          rows={1}
        />

        {/* Send button */}
        <motion.button
          onClick={handleSend}
          disabled={!message.trim() || isStreaming || activeSessionId === null}
          whileHover={{ scale: 1.05 }}
          whileTap={{ scale: 0.95 }}
          className="p-1.5 rounded-lg transition-colors"
          style={{
            backgroundColor:
              message.trim() && !isStreaming && activeSessionId !== null
                ? "var(--color-primary)"
                : "var(--color-surface-subtle)",
            color:
              message.trim() && !isStreaming && activeSessionId !== null
                ? "var(--color-on-primary)"
                : "var(--color-text-muted)",
            cursor:
              message.trim() && !isStreaming && activeSessionId !== null
                ? "pointer"
                : "not-allowed",
          }}
        >
          <Send className="w-4 h-4" />
        </motion.button>
      </div>
      <div style={{ display: "flex", justifyContent: "flex-end", marginTop: 4 }}>
        <span style={{ fontSize: 11, color: "var(--color-text-muted)" }}>
          Enter 发送 · Shift+Enter 换行
        </span>
      </div>
    </div>
  );
}
