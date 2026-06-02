import React, { useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import * as Collapsible from "@radix-ui/react-collapsible";
import type { AgentMessage } from "../../../types";
import { ToolIcon } from "../shared/ToolIcon";
import { JsonHighlight } from "../shared/JsonHighlight";

interface ToolCallCardProps {
  toolCall: { name: string; args: Record<string, unknown> };
  message?: AgentMessage;
}

export function ToolCallCard({ toolCall, message }: ToolCallCardProps) {
  const [isOpen, setIsOpen] = useState(false);

  // Parse result from message content
  let result: unknown = null;
  if (message?.content) {
    try {
      result = JSON.parse(message.content);
    } catch {
      result = message.content;
    }
  }

  const hasResult = message !== undefined && result !== null && result !== undefined;

  // Get display name for the tool
  const getToolDisplayName = (name: string) => {
    const nameMap: Record<string, string> = {
      // 发票查询与详情
      search_invoices: "搜索发票",
      get_invoice_detail: "获取发票详情",
      get_invoice_field_catalog: "获取字段字典",

      // 仪表盘与统计
      get_dashboard_stats: "获取统计数据",
      get_current_date_context: "获取日期上下文",

      // 附件与表格
      list_message_attachments: "查看附件列表",
      inspect_spreadsheet: "检查表格内容",

      // 导出功能
      export_invoices: "导出发票",
      create_export_preview: "创建导出预览",
      generate_template_plan : "生成模板计划",
      export_invoices_with_template: "按模板导出",
      export_pdf_report: "导出PDF报表",
      validate_xlsx : "验证XLSX文件",

      // 发票编辑
      update_invoice: "更新发票信息",
      merge_invoices: "合并发票",

      // 标签管理
      get_badge_config: "获取标签配置",
      set_badge_config: "设置标签配置",
      set_invoice_badge: "设置发票标签",

      // 价格配置
      get_price_config: "获取价格配置",
      set_price_config: "设置价格配置",

      // 主题设置
      get_theme: "获取主题设置",
      set_theme: "切换主题",

      // 系统功能
      export_logs: "导出日志",
      export_backup: "导出备份",
      cleanup_storage: "清理存储空间",
      get_app_info: "获取应用信息",
      get_sysinfo: "获取系统信息",

      // 用户交互
      ask_user: "询问用户",
    };
    return nameMap[name] || name;
  };

  return (
    <Collapsible.Root open={isOpen} onOpenChange={setIsOpen}>
      <div
        className="rounded-lg overflow-hidden"
        style={{
          backgroundColor: "var(--color-surface)",
          border: "1px solid var(--color-border)",
        }}
      >
        {/* Header - always visible */}
        <Collapsible.Trigger asChild>
          <button
            className="w-full flex items-center gap-3 px-4 py-2.5 hover:opacity-80 transition-opacity"
            style={{ backgroundColor: "var(--color-surface-subtle)" }}
          >
            <ToolIcon
              name={toolCall.name}
              className="w-4 h-4"
              style={{ color: "var(--color-primary)" }}
            />
            <span
              className="text-sm font-medium flex-1 text-left"
              style={{ color: "var(--color-text)" }}
            >
              {getToolDisplayName(toolCall.name)}
            </span>
            {message === undefined ? (
              <span
                className="text-xs px-2 py-0.5 rounded"
                style={{
                  backgroundColor: "var(--color-warn-bg, #fef3c7)",
                  color: "var(--color-warn, #d97706)",
                }}
              >
                等待确认
              </span>
            ) : (
              <span
                className="text-xs px-2 py-0.5 rounded"
                style={{
                  backgroundColor: "var(--color-success-bg)",
                  color: "var(--color-success)",
                }}
              >
                成功
              </span>
            )}
            <svg
              className="w-4 h-4 transition-transform"
              style={{
                color: "var(--color-text-muted)",
                transform: isOpen ? "rotate(180deg)" : "rotate(0deg)",
              }}
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M19 9l-7 7-7-7"
              />
            </svg>
          </button>
        </Collapsible.Trigger>

        {/* Expandable content */}
        <AnimatePresence>
          {isOpen && (
            <Collapsible.Content forceMount>
              <motion.div
                initial={{ height: 0, opacity: 0 }}
                animate={{ height: "auto", opacity: 1 }}
                exit={{ height: 0, opacity: 0 }}
                transition={{ duration: 0.2 }}
                className="overflow-hidden"
              >
                {/* Arguments - only show if there are actual arguments */}
                {Object.keys(toolCall.args).length > 0 &&
                  Object.values(toolCall.args).some(
                    (v) => v !== undefined && v !== null && v !== ""
                  ) && (
                    <div
                      className="px-4 py-3 border-t"
                      style={{ borderColor: "var(--color-border)" }}
                    >
                      <div
                        className="text-xs font-medium mb-2"
                        style={{ color: "var(--color-text-muted)" }}
                      >
                        参数
                      </div>
                      <div className="space-y-1">
                        {Object.entries(toolCall.args)
                          .filter(
                            ([, value]) =>
                              value !== undefined && value !== null && value !== ""
                          )
                          .map(([key, value]) => (
                            <div key={key} className="flex gap-2 text-xs">
                              <span
                                className="font-mono"
                                style={{ color: "var(--color-primary-text)" }}
                              >
                                {key}:
                              </span>
                              <span
                                className="font-mono"
                                style={{ color: "var(--color-text-secondary)" }}
                              >
                                {JSON.stringify(value)}
                              </span>
                            </div>
                          ))}
                      </div>
                    </div>
                  )}

                {/* Result */}
                {hasResult && (
                  <div
                    className="px-4 py-3 border-t"
                    style={{ borderColor: "var(--color-border)" }}
                  >
                    <div
                      className="text-xs font-medium mb-2"
                      style={{ color: "var(--color-text-muted)" }}
                    >
                      结果
                    </div>
                    <JsonHighlight data={result} maxHeight={400} />
                  </div>
                )}
              </motion.div>
            </Collapsible.Content>
          )}
        </AnimatePresence>
      </div>
    </Collapsible.Root>
  );
}
