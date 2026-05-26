import React, { useState } from "react";
import { motion } from "framer-motion";
import { HelpCircle, Loader2 } from "lucide-react";
import { pickSaveFile } from "../../../api";
import type { PendingConfirmation, ConfirmOption } from "../../../types";

const EXPORT_TOOLS = new Set([
  "export_invoices",
  "export_invoices_with_template",
]);

function ghostStyle(variant?: string) {
  const isCancel = variant === "cancel";
  return {
    backgroundColor: "transparent",
    color: isCancel
      ? "var(--color-text-muted)"
      : "var(--color-text)",
    border: "1px solid var(--color-border)",
  } as React.CSSProperties;
}

const ghostHover: React.CSSProperties = {
  backgroundColor: "var(--color-surface-subtle)",
};

interface PendingConfirmationCardProps {
  pending: PendingConfirmation;
  onConfirm: (extraParams?: Record<string, unknown>) => void;
  onCancel: () => void;
}

export function PendingConfirmationCard({
  pending,
  onConfirm,
  onCancel,
}: PendingConfirmationCardProps) {
  const [loading, setLoading] = useState(false);
  const [showOtherInput, setShowOtherInput] = useState(false);
  const [otherText, setOtherText] = useState("");

  const handleOptionClick = async (option: ConfirmOption) => {
    if (option.value === "cancel") {
      onCancel();
      return;
    }

    if (option.value === "pick_path" && EXPORT_TOOLS.has(pending.tool_name)) {
      const isTemplate = pending.tool_name === "export_invoices_with_template";
      const defaultExt = isTemplate ? "xlsx" : "csv";
      const filterName = isTemplate ? "Excel 文件" : "CSV 文件";

      const filePath = await pickSaveFile(
        `发票导出.${defaultExt}`,
        [[filterName, [defaultExt]]],
      );

      if (!filePath) return;
      setLoading(true);
      onConfirm({ choice: option.value, output_path: filePath });
      return;
    }

    setLoading(true);
    onConfirm({ choice: option.value });
  };

  const handleConfirm = async () => {
    if (EXPORT_TOOLS.has(pending.tool_name)) {
      const existingPath = pending.arguments.output_path as string | undefined;
      if (existingPath) {
        setLoading(true);
        onConfirm();
        return;
      }

      const isTemplate = pending.tool_name === "export_invoices_with_template";
      const defaultExt = isTemplate ? "xlsx" : "csv";
      const filterName = isTemplate ? "Excel 文件" : "CSV 文件";

      const filePath = await pickSaveFile(
        `发票导出.${defaultExt}`,
        [[filterName, [defaultExt]]],
      );

      if (!filePath) return;
      setLoading(true);
      onConfirm({ output_path: filePath });
    } else {
      setLoading(true);
      onConfirm();
    }
  };

  const handleCancel = () => {
    setLoading(true);
    onCancel();
  };

  const handleOtherSubmit = () => {
    if (otherText.trim()) {
      setLoading(true);
      onConfirm({ choice: "__other__", text: otherText.trim() });
    }
  };

  const hasOptions = pending.options && pending.options.length > 0;

  return (
    <div
      className="rounded-xl overflow-hidden mt-2"
      style={{
        backgroundColor: "var(--color-surface-elevated, var(--color-surface))",
        border: "1px solid var(--color-border)",
        boxShadow: "0 1px 3px rgba(0,0,0,0.06)",
        maxWidth: 360,
      }}
    >
      {/* Message */}
      <div className="px-3.5 py-2.5 flex items-start gap-2">
        <HelpCircle
          className="w-3.5 h-3.5 shrink-0 mt-0.5"
          style={{ color: "var(--color-primary)" }}
        />
        <p
          className="text-xs leading-relaxed whitespace-pre-wrap"
          style={{ color: "var(--color-text-secondary)" }}
        >
          {pending.message}
        </p>
      </div>

      {/* Buttons */}
      <div
        className="px-3.5 py-2.5 border-t"
        style={{ borderColor: "var(--color-border)" }}
      >
        {loading ? (
          <div className="flex items-center justify-center gap-1.5 py-0.5">
            <Loader2
              className="w-3 h-3 animate-spin"
              style={{ color: "var(--color-text-muted)" }}
            />
            <span
              className="text-xs"
              style={{ color: "var(--color-text-muted)" }}
            >
              处理中...
            </span>
          </div>
        ) : hasOptions ? (
          <div className="flex flex-wrap gap-1.5 justify-end">
            {pending.options!.map((option) => (
              <GhostButton
                key={option.value}
                label={option.label}
                variant={option.value === "cancel" ? "cancel" : undefined}
                onClick={() => handleOptionClick(option)}
              />
            ))}
            {showOtherInput ? (
              <div className="flex gap-1.5 w-full">
                <input
                  value={otherText}
                  onChange={(e) => setOtherText(e.target.value)}
                  placeholder="输入自定义内容..."
                  className="flex-1 px-2.5 py-1 rounded-lg text-xs min-w-0"
                  style={{
                    border: "1px solid var(--color-border)",
                    backgroundColor: "var(--color-surface)",
                    color: "var(--color-text)",
                    outline: "none",
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleOtherSubmit();
                  }}
                  autoFocus
                />
                <GhostButton label="发送" onClick={handleOtherSubmit} />
              </div>
            ) : (
              <GhostButton
                label="其他..."
                onClick={() => setShowOtherInput(true)}
                dashed
              />
            )}
          </div>
        ) : (
          <div className="flex justify-end gap-1.5">
            <GhostButton label="取消" variant="cancel" onClick={handleCancel} />
            <GhostButton label="确认" onClick={handleConfirm} />
          </div>
        )}
      </div>
    </div>
  );
}

function GhostButton({
  label,
  variant,
  dashed,
  onClick,
}: {
  label: string;
  variant?: "cancel";
  dashed?: boolean;
  onClick: () => void;
}) {
  const [hovered, setHovered] = useState(false);
  const base = ghostStyle(variant);
  return (
    <motion.button
      whileHover={{ scale: 1.02 }}
      whileTap={{ scale: 0.97 }}
      onClick={onClick}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      className="px-3 py-1 rounded-lg text-xs font-medium transition-colors"
      style={{
        ...base,
        ...(hovered ? ghostHover : {}),
        border: dashed ? "1px dashed var(--color-border)" : base.border,
      }}
    >
      {label}
    </motion.button>
  );
}
