import React from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Plus, Trash2, MessageSquare } from "lucide-react";
import { useAgentSession } from "../hooks/useAgentSession";
import { useAgentStore } from "../hooks/useAgentStore";

export function SessionSidebar() {
  const {
    sessions,
    activeSessionId,
    createNewSession,
    deleteSession,
    switchSession,
  } = useAgentSession();
  const loading = useAgentStore((s) => s.loading);

  return (
    <div
      className="w-[220px] flex flex-col border-r h-full"
      style={{
        backgroundColor: "var(--color-agent-sessions-bg)",
        borderColor: "var(--color-border)",
      }}
    >
      {/* Header */}
      <div
        className="flex items-center justify-between p-3 border-b"
        style={{ borderColor: "var(--color-border)" }}
      >
        <h3
          className="text-sm font-medium"
          style={{ color: "var(--color-text)" }}
        >
          会话
        </h3>
        <motion.button
          whileHover={{ scale: 1.05 }}
          whileTap={{ scale: 0.95 }}
          onClick={createNewSession}
          className="p-1.5 rounded-md hover:opacity-80 transition-opacity"
          style={{
            backgroundColor: "var(--color-primary-bg)",
            color: "var(--color-primary-text)",
          }}
        >
          <Plus className="w-4 h-4" />
        </motion.button>
      </div>

      {/* Session list */}
      <div className="flex-1 overflow-y-auto p-2">
        <AnimatePresence initial={false}>
          {sessions.map((session) => (
            <motion.div
              key={session.id}
              initial={{ opacity: 0, height: 0 }}
              animate={{ opacity: 1, height: "auto" }}
              exit={{ opacity: 0, height: 0 }}
              transition={{ duration: 0.15 }}
            >
              <button
                onClick={() => switchSession(session.id)}
                className="w-full flex items-center gap-2 p-2 rounded-md mb-1 text-left transition-colors group"
                style={{
                  backgroundColor:
                    activeSessionId === session.id
                      ? "var(--color-sidebar-active)"
                      : "transparent",
                  color:
                    activeSessionId === session.id
                      ? "var(--color-sidebar-text-active)"
                      : "var(--color-sidebar-text)",
                }}
              >
                <MessageSquare className="w-4 h-4 flex-shrink-0" />
                <span className="flex-1 text-sm truncate">
                  {session.title || `会话${session.id}`}
                </span>
                <motion.button
                  whileHover={{ scale: 1.1 }}
                  whileTap={{ scale: 0.9 }}
                  onClick={(e) => {
                    e.stopPropagation();
                    deleteSession(session.id);
                  }}
                  className="opacity-0 group-hover:opacity-100 p-1 rounded hover:opacity-80 transition-opacity"
                  style={{ color: "var(--color-error-text)" }}
                >
                  <Trash2 className="w-3.5 h-3.5" />
                </motion.button>
              </button>
            </motion.div>
          ))}
        </AnimatePresence>

        {sessions.length === 0 && !loading && (
          <div
            className="text-center py-8 text-sm"
            style={{ color: "var(--color-text-muted)" }}
          >
            暂无会话
            <br />
            点击 + 创建新会话
          </div>
        )}

        {loading && (
          <div className="flex justify-center py-4">
            <motion.div
              className="w-5 h-5 rounded-full border-2"
              style={{
                borderColor: "var(--color-border)",
                borderTopColor: "var(--color-primary)",
              }}
              animate={{ rotate: 360 }}
              transition={{ duration: 1, repeat: Infinity, ease: "linear" }}
            />
          </div>
        )}
      </div>
    </div>
  );
}
