import React from "react";
import { motion } from "framer-motion";
import { FileText, Folder, Trash2 } from "lucide-react";
import type { AgentArtifact } from "../../../types";
import {
  openAgentArtifactFile,
  openAgentArtifactFolder,
  deleteAgentArtifact,
} from "../../../api";
import { useAgentStore } from "../hooks/useAgentStore";

interface FileListProps {
  artifacts: AgentArtifact[];
  onSelect: (id: number) => void;
}

export function FileList({ artifacts, onSelect }: FileListProps) {
  const refreshArtifacts = useAgentStore((s) => s.refreshArtifacts);
  const activeSessionId = useAgentStore((s) => s.activeSessionId);

  const handleOpen = async (artifact: AgentArtifact, e: React.MouseEvent) => {
    e.stopPropagation();
    if (!activeSessionId) return;
    try {
      await openAgentArtifactFile(activeSessionId, artifact.id);
    } catch (err) {
      console.error("Failed to open artifact:", err);
    }
  };

  const handleOpenFolder = async (
    artifact: AgentArtifact,
    e: React.MouseEvent
  ) => {
    e.stopPropagation();
    if (!activeSessionId) return;
    try {
      await openAgentArtifactFolder(activeSessionId, artifact.id);
    } catch (err) {
      console.error("Failed to open folder:", err);
    }
  };

  const handleDelete = async (
    artifact: AgentArtifact,
    e: React.MouseEvent
  ) => {
    e.stopPropagation();
    if (!activeSessionId) return;
    try {
      await deleteAgentArtifact(activeSessionId, artifact.id);
      await refreshArtifacts();
    } catch (err) {
      console.error("Failed to delete artifact:", err);
    }
  };

  const formatBytes = (bytes: number | null) => {
    if (bytes === null || bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
  };

  if (artifacts.length === 0) {
    return (
      <div
        className="flex flex-col items-center justify-center h-full py-8"
        style={{ color: "var(--color-text-muted)" }}
      >
        <FileText className="w-8 h-8 mb-2 opacity-50" />
        <p className="text-sm">暂无产出物</p>
      </div>
    );
  }

  return (
    <div className="p-2 overflow-y-auto h-full">
      {artifacts.map((artifact) => (
        <motion.div
          key={artifact.id}
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          className="p-3 rounded-md mb-2 cursor-pointer transition-colors group"
          style={{
            backgroundColor: "var(--color-surface-subtle)",
            border: "1px solid var(--color-border)",
          }}
          onClick={() => onSelect(artifact.id)}
          whileHover={{ borderColor: "var(--color-primary)" }}
        >
          <div className="flex items-start gap-2">
            <FileText
              className="w-4 h-4 mt-0.5 flex-shrink-0"
              style={{ color: "var(--color-primary)" }}
            />
            <div className="flex-1 min-w-0">
              <p
                className="text-sm font-medium truncate"
                style={{ color: "var(--color-text)" }}
              >
                {artifact.title}
              </p>
              <p
                className="text-xs mt-0.5"
                style={{ color: "var(--color-text-muted)" }}
              >
                {artifact.mime_type || "未知类型"}
                {artifact.byte_size && ` · ${formatBytes(artifact.byte_size)}`}
              </p>
            </div>
          </div>

          {/* Action buttons */}
          <div className="flex gap-1 mt-2 opacity-0 group-hover:opacity-100 transition-opacity">
            <button
              onClick={(e) => handleOpen(artifact, e)}
              className="flex items-center gap-1 px-2 py-1 rounded text-xs transition-colors"
              style={{
                backgroundColor: "var(--color-primary-bg)",
                color: "var(--color-primary-text)",
              }}
            >
              打开
            </button>
            <button
              onClick={(e) => handleOpenFolder(artifact, e)}
              className="flex items-center gap-1 px-2 py-1 rounded text-xs transition-colors"
              style={{
                backgroundColor: "var(--color-surface)",
                color: "var(--color-text-secondary)",
              }}
            >
              <Folder className="w-3 h-3" />
              目录
            </button>
            <button
              onClick={(e) => handleDelete(artifact, e)}
              className="flex items-center gap-1 px-2 py-1 rounded text-xs transition-colors"
              style={{
                backgroundColor: "var(--color-error-bg)",
                color: "var(--color-error-text)",
              }}
            >
              <Trash2 className="w-3 h-3" />
            </button>
          </div>
        </motion.div>
      ))}
    </div>
  );
}
