import React from "react";
import { motion } from "framer-motion";
import { FileText, Download, Folder } from "lucide-react";
import { useAgentStore } from "../hooks/useAgentStore";
import {
  openAgentArtifactFile,
  openAgentArtifactFolder,
} from "../../../api";

interface ArtifactPreviewProps {
  artifactId: number | null;
}

export function ArtifactPreview({ artifactId }: ArtifactPreviewProps) {
  const artifacts = useAgentStore((s) => s.artifacts);
  const activeSessionId = useAgentStore((s) => s.activeSessionId);
  const artifact = artifacts.find((a) => a.id === artifactId);

  if (!artifact) {
    return (
      <div
        className="flex flex-col items-center justify-center h-full py-8"
        style={{ color: "var(--color-text-muted)" }}
      >
        <FileText className="w-8 h-8 mb-2 opacity-50" />
        <p className="text-sm">选择一个产出物进行预览</p>
      </div>
    );
  }

  const formatBytes = (bytes: number | null) => {
    if (bytes === null || bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
  };

  const handleOpen = async () => {
    if (!activeSessionId) return;
    try {
      await openAgentArtifactFile(activeSessionId, artifact.id);
    } catch (err) {
      console.error("Failed to open artifact:", err);
    }
  };

  const handleOpenFolder = async () => {
    if (!activeSessionId) return;
    try {
      await openAgentArtifactFolder(activeSessionId, artifact.id);
    } catch (err) {
      console.error("Failed to open folder:", err);
    }
  };

  return (
    <div className="p-4 h-full overflow-y-auto">
      <div className="mb-4">
        <div className="flex items-center gap-2 mb-2">
          <FileText
            className="w-6 h-6"
            style={{ color: "var(--color-primary)" }}
          />
          <h4
            className="text-sm font-medium"
            style={{ color: "var(--color-text)" }}
          >
            {artifact.title}
          </h4>
        </div>
        <div className="space-y-1 text-xs" style={{ color: "var(--color-text-muted)" }}>
          <p>类型: {artifact.mime_type || "未知"}</p>
          <p>大小: {formatBytes(artifact.byte_size)}</p>
          <p>创建时间: {new Date(artifact.created_at).toLocaleString()}</p>
          {artifact.artifact_type && <p>分类: {artifact.artifact_type}</p>}
        </div>
      </div>

      {/* Action buttons */}
      <div className="flex gap-2">
        <motion.button
          whileHover={{ scale: 1.02 }}
          whileTap={{ scale: 0.98 }}
          onClick={handleOpen}
          className="flex items-center gap-2 px-3 py-2 rounded-md text-xs font-medium transition-colors"
          style={{
            backgroundColor: "var(--color-primary)",
            color: "var(--color-on-primary)",
          }}
        >
          <Download className="w-3.5 h-3.5" />
          打开文件
        </motion.button>
        <motion.button
          whileHover={{ scale: 1.02 }}
          whileTap={{ scale: 0.98 }}
          onClick={handleOpenFolder}
          className="flex items-center gap-2 px-3 py-2 rounded-md text-xs font-medium transition-colors"
          style={{
            backgroundColor: "var(--color-surface-subtle)",
            color: "var(--color-text-secondary)",
            border: "1px solid var(--color-border)",
          }}
        >
          <Folder className="w-3.5 h-3.5" />
          打开目录
        </motion.button>
      </div>

      {/* Metadata preview */}
      {artifact.metadata_json && (
        <div className="mt-4">
          <h5
            className="text-xs font-medium mb-2"
            style={{ color: "var(--color-text-muted)" }}
          >
            元数据
          </h5>
          <pre
            className="p-3 rounded-md text-xs overflow-auto"
            style={{
              backgroundColor: "var(--color-surface-subtle)",
              border: "1px solid var(--color-border)",
              maxHeight: "200px",
              color: "var(--color-text-secondary)",
            }}
          >
            {JSON.stringify(JSON.parse(artifact.metadata_json), null, 2)}
          </pre>
        </div>
      )}
    </div>
  );
}
