import React from "react";
import { motion } from "framer-motion";
import { CheckCircle, XCircle, Loader2, Clock } from "lucide-react";
import type { AgentTask } from "../../../types";

const TASK_NAME_MAP: Record<string, string> = {
  export_invoices: "导出发票",
  export_invoices_with_template: "按模板导出",
};

const STATUS_LABEL_MAP: Record<string, string> = {
  completed: "完成",
  failed: "失败",
  running: "运行中",
};

function getTaskDisplayName(toolName: string): string {
  return TASK_NAME_MAP[toolName] || toolName;
}

interface TaskListProps {
  tasks: AgentTask[];
}

export function TaskList({ tasks }: TaskListProps) {
  const getStatusIcon = (status: string) => {
    switch (status) {
      case "completed":
        return (
          <CheckCircle className="w-4 h-4" style={{ color: "var(--color-success)" }} />
        );
      case "failed":
        return (
          <XCircle className="w-4 h-4" style={{ color: "var(--color-error)" }} />
        );
      case "running":
        return (
          <motion.div
            animate={{ rotate: 360 }}
            transition={{ duration: 1, repeat: Infinity, ease: "linear" }}
          >
            <Loader2 className="w-4 h-4" style={{ color: "var(--color-primary)" }} />
          </motion.div>
        );
      default:
        return (
          <Clock className="w-4 h-4" style={{ color: "var(--color-text-muted)" }} />
        );
    }
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case "completed":
        return {
          bg: "var(--color-success-bg)",
          text: "var(--color-success)",
        };
      case "failed":
        return {
          bg: "var(--color-error-bg)",
          text: "var(--color-error-text)",
        };
      case "running":
        return {
          bg: "var(--color-primary-bg)",
          text: "var(--color-primary-text)",
        };
      default:
        return {
          bg: "var(--color-surface-subtle)",
          text: "var(--color-text-muted)",
        };
    }
  };

  const formatTime = (dateStr: string | null) => {
    if (!dateStr) return "";
    const date = new Date(dateStr);
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  };

  const formatDuration = (task: AgentTask) => {
    if (!task.completed_at || !task.created_at) return "";
    const start = new Date(task.created_at);
    const end = new Date(task.completed_at);
    const duration = end.getTime() - start.getTime();
    if (duration < 1000) return `${duration}ms`;
    return `${(duration / 1000).toFixed(1)}s`;
  };

  if (tasks.length === 0) {
    return (
      <div
        className="flex flex-col items-center justify-center h-full py-8"
        style={{ color: "var(--color-text-muted)" }}
      >
        <Clock className="w-8 h-8 mb-2 opacity-50" />
        <p className="text-sm">暂无任务</p>
      </div>
    );
  }

  return (
    <div className="p-2 overflow-y-auto h-full">
      {tasks.map((task) => {
        const statusColor = getStatusColor(task.status);
        return (
          <motion.div
            key={task.id}
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            className="p-3 rounded-md mb-2"
            style={{
              backgroundColor: "var(--color-surface-subtle)",
              border: "1px solid var(--color-border)",
            }}
          >
            <div className="flex items-start gap-2">
              {getStatusIcon(task.status)}
              <div className="flex-1 min-w-0">
                <div className="flex items-center justify-between">
                  <p
                    className="text-sm font-medium"
                    style={{ color: "var(--color-text)" }}
                  >
                    {getTaskDisplayName(task.tool_name)}
                  </p>
                  <span
                    className="text-xs px-1.5 py-0.5 rounded"
                    style={{
                      backgroundColor: statusColor.bg,
                      color: statusColor.text,
                    }}
                  >
                    {STATUS_LABEL_MAP[task.status] || task.status}
                  </span>
                </div>
                <p
                  className="text-xs mt-1"
                  style={{ color: "var(--color-text-muted)" }}
                >
                  {formatTime(task.created_at)}
                  {task.completed_at && ` · ${formatDuration(task)}`}
                </p>
                {task.error_message && (
                  <p
                    className="text-xs mt-1 p-1.5 rounded"
                    style={{
                      backgroundColor: "var(--color-error-bg)",
                      color: "var(--color-error-text)",
                    }}
                  >
                    {task.error_message}
                  </p>
                )}
              </div>
            </div>
          </motion.div>
        );
      })}
    </div>
  );
}
