import React, { useState } from "react";
import * as Tabs from "@radix-ui/react-tabs";
import { motion, AnimatePresence } from "framer-motion";
import { FileText, History, Eye } from "lucide-react";
import { useAgentStore } from "../hooks/useAgentStore";
import { FileList } from "./FileList";
import { TaskList } from "./TaskList";
import { ArtifactPreview } from "./ArtifactPreview";

export function ArtifactPanel() {
  const [activeTab, setActiveTab] = useState("files");
  const artifacts = useAgentStore((s) => s.artifacts);
  const tasks = useAgentStore((s) => s.tasks);
  const [selectedArtifactId, setSelectedArtifactId] = useState<number | null>(
    null
  );

  return (
    <div
      className="w-[280px] flex flex-col border-l h-full"
      style={{
        backgroundColor: "var(--color-surface)",
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
          产出物
        </h3>
        <span
          className="text-xs px-2 py-0.5 rounded-full"
          style={{
            backgroundColor: "var(--color-badge-neutral-bg)",
            color: "var(--color-badge-neutral-text)",
          }}
        >
          {artifacts.length + tasks.length}
        </span>
      </div>

      {/* Tabs */}
      <Tabs.Root
        value={activeTab}
        onValueChange={setActiveTab}
        className="flex-1 flex flex-col overflow-hidden"
      >
        <Tabs.List
          className="flex border-b"
          style={{ borderColor: "var(--color-border)" }}
        >
          <Tabs.Trigger
            value="files"
            className="flex-1 flex items-center justify-center gap-1.5 py-2 text-xs font-medium transition-colors"
            style={{
              color:
                activeTab === "files"
                  ? "var(--color-primary-text)"
                  : "var(--color-text-muted)",
              borderBottom:
                activeTab === "files"
                  ? "2px solid var(--color-primary)"
                  : "2px solid transparent",
            }}
          >
            <FileText className="w-3.5 h-3.5" />
            文件
          </Tabs.Trigger>
          <Tabs.Trigger
            value="tasks"
            className="flex-1 flex items-center justify-center gap-1.5 py-2 text-xs font-medium transition-colors"
            style={{
              color:
                activeTab === "tasks"
                  ? "var(--color-primary-text)"
                  : "var(--color-text-muted)",
              borderBottom:
                activeTab === "tasks"
                  ? "2px solid var(--color-primary)"
                  : "2px solid transparent",
            }}
          >
            <History className="w-3.5 h-3.5" />
            任务
          </Tabs.Trigger>
          <Tabs.Trigger
            value="preview"
            className="flex-1 flex items-center justify-center gap-1.5 py-2 text-xs font-medium transition-colors"
            style={{
              color:
                activeTab === "preview"
                  ? "var(--color-primary-text)"
                  : "var(--color-text-muted)",
              borderBottom:
                activeTab === "preview"
                  ? "2px solid var(--color-primary)"
                  : "2px solid transparent",
            }}
          >
            <Eye className="w-3.5 h-3.5" />
            预览
          </Tabs.Trigger>
        </Tabs.List>

        <div className="flex-1 overflow-hidden">
          <Tabs.Content value="files" className="h-full">
            <FileList
              artifacts={artifacts}
              onSelect={(id) => {
                setSelectedArtifactId(id);
                setActiveTab("preview");
              }}
            />
          </Tabs.Content>

          <Tabs.Content value="tasks" className="h-full">
            <TaskList tasks={tasks} />
          </Tabs.Content>

          <Tabs.Content value="preview" className="h-full">
            <ArtifactPreview artifactId={selectedArtifactId} />
          </Tabs.Content>
        </div>
      </Tabs.Root>
    </div>
  );
}
