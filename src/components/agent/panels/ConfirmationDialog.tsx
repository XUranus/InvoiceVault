import React from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { motion } from "framer-motion";
import { AlertTriangle, X } from "lucide-react";
import { pickSaveFile } from "../../../api";
import { useAgentStore } from "../hooks/useAgentStore";
import { ToolIcon } from "../shared/ToolIcon";
import type { PendingConfirmation } from "../../../types";

const TOOL_DISPLAY_NAMES: Record<string, string> = {
  search_invoices: "搜索发票",
  get_invoice_detail: "获取发票详情",
  get_invoice_field_catalog: "获取字段字典",
  get_dashboard_stats: "获取统计数据",
  get_current_date_context: "获取日期上下文",
  list_message_attachments: "查看附件列表",
  inspect_spreadsheet: "检查表格内容",
  export_invoices: "导出发票",
  create_export_preview: "创建导出预览",
  export_invoices_with_template: "按模板导出",
  export_pdf_report: "导出PDF报表",
  update_invoice: "更新发票信息",
  merge_invoices: "合并发票",
  get_badge_config: "获取标签配置",
  set_badge_config: "设置标签配置",
  set_invoice_badge: "设置发票标签",
  get_price_config: "获取价格配置",
  set_price_config: "设置价格配置",
  get_theme: "获取主题设置",
  set_theme: "切换主题",
  export_logs: "导出日志",
  export_backup: "导出备份",
  cleanup_storage: "清理存储空间",
  get_app_info: "获取应用信息",
};

const EXPORT_TOOLS = new Set([
  "export_invoices",
  "export_invoices_with_template",
]);

export function ConfirmationDialog() {
  const pendingConfirm = useAgentStore((s) => s.pendingConfirm);
  const confirmAction = useAgentStore((s) => s.confirmAction);
  const setPendingConfirm = useAgentStore((s) => s.setPendingConfirm);

  const handleConfirm = async () => {
    if (!pendingConfirm) return;

    // For export tools, open save dialog to get output path
    if (EXPORT_TOOLS.has(pendingConfirm.tool_name)) {
      const isTemplate = pendingConfirm.tool_name === "export_invoices_with_template";
      const defaultExt = isTemplate ? "xlsx" : "csv";
      const filterName = isTemplate ? "Excel 文件" : "CSV 文件";

      const filePath = await pickSaveFile(
        `发票导出.${defaultExt}`,
        [[filterName, [defaultExt]]],
      );

      if (!filePath) return; // User cancelled save dialog
      await confirmAction(true, { output_path: filePath });
    } else {
      await confirmAction(true);
    }
  };

  const handleCancel = async () => {
    await confirmAction(false);
  };

  const handleOpenChange = (open: boolean) => {
    if (!open) {
      setPendingConfirm(null);
    }
  };

  const displayName = pendingConfirm
    ? TOOL_DISPLAY_NAMES[pendingConfirm.tool_name] || pendingConfirm.tool_name
    : "";

  return (
    <Dialog.Root
      open={pendingConfirm !== null}
      onOpenChange={handleOpenChange}
    >
      <Dialog.Portal>
        <Dialog.Overlay asChild>
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 z-50"
            style={{ backgroundColor: "rgba(0, 0, 0, 0.5)" }}
          />
        </Dialog.Overlay>
        <Dialog.Content asChild>
          <motion.div
            initial={{ opacity: 0, scale: 0.95, y: 20 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.95, y: 20 }}
            className="fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 z-50 w-[90vw] max-w-[440px] rounded-lg shadow-lg p-6"
            style={{
              backgroundColor: "var(--color-surface-solid)",
              border: "1px solid var(--color-border)",
            }}
          >
            <Dialog.Title className="flex items-center gap-3 mb-4">
              <AlertTriangle
                className="w-6 h-6"
                style={{ color: "var(--color-warn)" }}
              />
              <span
                className="text-lg font-semibold"
                style={{ color: "var(--color-text)" }}
              >
                确认操作
              </span>
            </Dialog.Title>

            <Dialog.Description asChild>
              <div className="mb-6">
                {pendingConfirm && (
                  <div>
                    <div
                      className="flex items-center gap-3 p-3 rounded-md mb-3"
                      style={{
                        backgroundColor: "var(--color-confirm-bg)",
                        border: "1px solid var(--color-confirm-border)",
                      }}
                    >
                      <ToolIcon
                        name={pendingConfirm.tool_name}
                        className="w-5 h-5"
                        style={{ color: "var(--color-primary-text)" }}
                      />
                      <span
                        className="text-sm font-medium"
                        style={{ color: "var(--color-text)" }}
                      >
                        {displayName}
                      </span>
                    </div>
                    {pendingConfirm.message && (
                      <p
                        className="text-sm"
                        style={{ color: "var(--color-text-muted)" }}
                      >
                        {pendingConfirm.message}
                      </p>
                    )}
                  </div>
                )}
              </div>
            </Dialog.Description>

            <div className="flex justify-end gap-3">
              <Dialog.Close asChild>
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
              </Dialog.Close>
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

            <Dialog.Close asChild>
              <button
                className="absolute top-4 right-4 p-1 rounded-md hover:opacity-70 transition-opacity"
                style={{ color: "var(--color-text-muted)" }}
              >
                <X className="w-5 h-5" />
              </button>
            </Dialog.Close>
          </motion.div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
