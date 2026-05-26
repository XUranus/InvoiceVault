import React, { useState } from "react";
import { motion } from "framer-motion";
import { AlertTriangle, Loader2 } from "lucide-react";
import { pickSaveFile } from "../../../api";
import type { PendingConfirmation, ConfirmOption } from "../../../types";

const EXPORT_TOOLS = new Set([
  "export_invoices",
  "export_invoices_with_template",
]);

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

  const hasOptions = pending.options && pending.options.length > 0;

  return (
    <div
      className="rounded-lg overflow-hidden mt-2"
      style={{
        backgroundColor: "var(--color-confirm-bg, var(--color-surface))",
        border: "1px solid var(--color-confirm-border, var(--color-border))",
      }}
    >
      {/* Message */}
      <div className="px-4 py-3 flex items-start gap-2">
        <AlertTriangle
          className="w-4 h-4 shrink-0 mt-0.5"
          style={{ color: "var(--color-warn)" }}
        />
        <p
          className="text-sm whitespace-pre-wrap"
          style={{ color: "var(--color-text-muted)" }}
        >
          {pending.message}
        </p>
      </div>

      {/* Buttons */}
      <div
        className="px-4 py-3 border-t"
        style={{ borderColor: "var(--color-confirm-border, var(--color-border))" }}
      >
        {loading ? (
          <div className="flex items-center justify-center gap-2 py-1">
            <Loader2
              className="w-4 h-4 animate-spin"
              style={{ color: "var(--color-text-muted)" }}
            />
            <span
              className="text-sm"
              style={{ color: "var(--color-text-muted)" }}
            >
              处理中...
            </span>
          </div>
        ) : hasOptions ? (
          <div className="flex flex-col gap-2">
            {pending.options!.map((option) => {
              const isPrimary = option.style === "primary";
              const isDanger = option.style === "danger";
              return (
                <motion.button
                  key={option.value}
                  whileHover={{ scale: 1.02 }}
                  whileTap={{ scale: 0.98 }}
                  onClick={() => handleOptionClick(option)}
                  className="px-4 py-2 rounded-md text-sm font-medium transition-colors w-full"
                  style={{
                    backgroundColor: isPrimary
                      ? "var(--color-primary)"
                      : isDanger
                        ? "var(--color-danger, #ef4444)"
                        : "var(--color-surface-subtle)",
                    color: isPrimary
                      ? "var(--color-on-primary)"
                      : isDanger
                        ? "#fff"
                        : "var(--color-text)",
                    border:
                      isPrimary || isDanger
                        ? "none"
                        : "1px solid var(--color-border)",
                  }}
                >
                  {option.label}
                </motion.button>
              );
            })}
          </div>
        ) : (
          <div className="flex justify-end gap-3">
            <motion.button
              whileHover={{ scale: 1.02 }}
              whileTap={{ scale: 0.98 }}
              onClick={handleCancel}
              className="px-4 py-2 rounded-md text-sm font-medium transition-colors"
              style={{
                backgroundColor: "var(--color-surface-subtle)",
                color: "var(--color-text-secondary)",
                border: "1px solid var(--color-border)",
              }}
            >
              取消
            </motion.button>
            <motion.button
              whileHover={{ scale: 1.02 }}
              whileTap={{ scale: 0.98 }}
              onClick={handleConfirm}
              className="px-4 py-2 rounded-md text-sm font-medium transition-colors"
              style={{
                backgroundColor: "var(--color-primary)",
                color: "var(--color-on-primary)",
              }}
            >
              确认
            </motion.button>
          </div>
        )}
      </div>
    </div>
  );
}
