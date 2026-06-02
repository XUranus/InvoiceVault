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
            .filter((msg) => {
              if (msg.role !== "tool") return true;
              // Show pending confirmation tool messages as standalone cards
              try {
                const parsed = JSON.parse(msg.content);
                if (parsed.__pending_confirmation) return true;
              } catch { /* ignore */ }
              return false;
            })
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
          <AgentErrorCard message={streamState.errorMessage} />
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
          思考中...
        </span>
      </div>
    );
  }

  return null;
}

function AgentErrorCard({ message }: { message: string | null }) {
  const [expanded, setExpanded] = React.useState(false);
  const raw = message || "执行出错，请重试";

  // Extract HTTP status code if present
  const httpMatch = raw.match(/HTTP\s+(\d{3})/);
  const status = httpMatch ? httpMatch[1] : null;

  // User-friendly summary
  let summary = "请求失败";
  if (status === "429") summary = "请求过于频繁（速率限制）";
  else if (status === "401") summary = "API Key 无效或已过期";
  else if (status === "403") summary = "访问被拒绝，请检查 API Key 权限";
  else if (status === "404") summary = "API 地址不存在，请检查 Base URL";
  else if (status && Number(status) >= 500) summary = `LLM 服务异常 (HTTP ${status})`;
  else if (raw.includes("timed out") || raw.includes("timeout")) summary = "请求超时，请检查网络或增大超时时间";
  else if (raw.includes("dns error") || raw.includes("resolve")) summary = "DNS 解析失败，请检查网络";
  else if (raw.includes("connect") && raw.includes("error")) summary = "连接失败，请检查网络或 API 地址";
  else if (raw.includes("sending request")) summary = "网络请求失败，请检查网络连接";
  else summary = raw.slice(0, 80);

  return (
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
            {summary}
          </span>
          <button
            type="button"
            onClick={() => setExpanded((v) => !v)}
            style={{
              marginLeft: "auto",
              fontSize: 12,
              color: "var(--color-text-muted)",
              background: "none",
              border: "none",
              cursor: "pointer",
              textDecoration: "underline",
            }}
          >
            {expanded ? "收起" : "详情"}
          </button>
        </div>
        {expanded && (
          <pre
            style={{
              marginTop: 8,
              padding: 8,
              borderRadius: 6,
              fontSize: 12,
              lineHeight: 1.5,
              whiteSpace: "pre-wrap",
              wordBreak: "break-all",
              background: "var(--color-bg)",
              color: "var(--color-text-secondary)",
              maxHeight: 200,
              overflow: "auto",
            }}
          >
            {raw}
          </pre>
        )}
      </div>
    </motion.div>
  );
}
