import React, { useRef, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { AlertCircle } from "lucide-react";
import { TimelineItem } from "./TimelineItem";
import type { AgentMessage } from "../../../types";
import { useAgentStore } from "../hooks/useAgentStore";

interface TimelineProps {
  messages: AgentMessage[];
}

export function Timeline({ messages }: TimelineProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const streamState = useAgentStore((s) => s.streamState);

  // Auto-scroll to bottom when new messages arrive
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages, streamState]);

  // Build a map of tool_call_id to tool result message
  const toolResultMap = new Map<string, AgentMessage>();
  messages.forEach((msg) => {
    if (msg.role === "tool" && msg.tool_call_id) {
      toolResultMap.set(msg.tool_call_id, msg);
    }
  });

  return (
    <div
      ref={scrollRef}
      className="flex-1 overflow-y-auto px-6 py-4"
      style={{ scrollBehavior: "smooth" }}
    >
      <div className="relative">
        {/* Vertical timeline line */}
        <div
          className="absolute left-[15px] top-0 bottom-0 w-[2px]"
          style={{ backgroundColor: "var(--color-border)" }}
        />

        <AnimatePresence initial={false}>
          {messages
            .filter((msg) => msg.role !== "tool")
            .map((msg) => (
              <motion.div
                key={msg.id}
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.2, ease: "easeOut" }}
              >
                <TimelineItem message={msg} toolResultMap={toolResultMap} />
              </motion.div>
            ))}
        </AnimatePresence>

        {/* Streaming indicator */}
        {streamState &&
          streamState.phase !== "done" &&
          streamState.phase !== "error" && (
            <motion.div
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              className="relative pl-10 pb-4"
            >
              <div
                className="absolute left-[11px] top-[8px] w-[10px] h-[10px] rounded-full border-2"
                style={{
                  borderColor: "var(--color-primary)",
                  backgroundColor: "var(--color-primary-bg)",
                }}
              />
              <div
                className="rounded-lg px-4 py-3"
                style={{ backgroundColor: "var(--color-surface)" }}
              >
                <StreamIndicator phase={streamState.phase} toolName={streamState.toolName} />
              </div>
            </motion.div>
          )}

        {/* Error display */}
        {streamState && streamState.phase === "error" && (
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            className="relative pl-10 pb-4"
          >
            <div
              className="absolute left-[11px] top-[8px] w-[10px] h-[10px] rounded-full border-2"
              style={{
                borderColor: "var(--color-error)",
                backgroundColor: "var(--color-error-bg)",
              }}
            />
            <div
              className="rounded-lg px-4 py-3"
              style={{
                backgroundColor: "var(--color-error-bg)",
                border: "1px solid var(--color-error)",
              }}
            >
              <div className="flex items-center gap-2">
                <AlertCircle
                  className="w-4 h-4 shrink-0"
                  style={{ color: "var(--color-error)" }}
                />
                <span className="text-sm" style={{ color: "var(--color-error)" }}>
                  {streamState.errorMessage || "执行出错，请重试"}
                </span>
              </div>
            </div>
          </motion.div>
        )}
      </div>
    </div>
  );
}

function StreamIndicator({
  phase,
  toolName,
}: {
  phase: string;
  toolName: string | null;
}) {
  if (phase === "thinking") {
    return (
      <div className="flex items-center gap-2">
        <div className="flex gap-1">
          {[0, 1, 2].map((i) => (
            <motion.div
              key={i}
              className="w-2 h-2 rounded-full"
              style={{ backgroundColor: "var(--color-primary)" }}
              animate={{ y: [0, -6, 0] }}
              transition={{
                duration: 0.6,
                repeat: Infinity,
                delay: i * 0.15,
              }}
            />
          ))}
        </div>
        <span
          className="text-sm"
          style={{ color: "var(--color-text-muted)" }}
        >
          思考中...
        </span>
      </div>
    );
  }

  if (phase === "tool_call" || phase === "tool_result") {
    return (
      <div className="flex items-center gap-2">
        <motion.div
          className="w-2 h-2 rounded-full"
          style={{ backgroundColor: "var(--color-primary)" }}
          animate={{ scale: [1, 1.2, 1] }}
          transition={{ duration: 1, repeat: Infinity }}
        />
        <span className="text-sm" style={{ color: "var(--color-text-muted)" }}>
          {phase === "tool_call" ? `执行 ${toolName || "工具"}...` : `处理结果...`}
        </span>
      </div>
    );
  }

  if (phase === "answering") {
    return (
      <div className="flex items-center gap-2">
        <motion.div
          className="w-2 h-2 rounded-full"
          style={{ backgroundColor: "var(--color-success)" }}
          animate={{ opacity: [1, 0.5, 1] }}
          transition={{ duration: 1.5, repeat: Infinity }}
        />
        <span className="text-sm" style={{ color: "var(--color-text-muted)" }}>
          正在回复...
        </span>
      </div>
    );
  }

  if (phase === "starting") {
    return (
      <div className="flex items-center gap-2">
        <motion.div
          className="w-2 h-2 rounded-full"
          style={{ backgroundColor: "var(--color-text-muted)" }}
          animate={{ opacity: [1, 0.3, 1] }}
          transition={{ duration: 1, repeat: Infinity }}
        />
        <span className="text-sm" style={{ color: "var(--color-text-muted)" }}>
          连接中...
        </span>
      </div>
    );
  }

  return null;
}
