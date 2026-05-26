import React from "react";
import {
  Search,
  FileText,
  Download,
  Upload,
  Database,
  Settings,
  Trash2,
  Edit,
  Plus,
  RefreshCw,
  Wrench,
  BarChart3,
  Calendar,
  Paperclip,
  Table,
  FileDown,
  FileJson,
  GitMerge,
  Tag,
  DollarSign,
  Palette,
  FileCode,
  HardDrive,
  Info,
  MessageCircle,
  Monitor,
} from "lucide-react";

interface ToolIconProps {
  name: string;
  className?: string;
  style?: React.CSSProperties;
}

const toolIconMap: Record<string, React.ComponentType<{ className?: string; style?: React.CSSProperties }>> = {
  // 发票查询与详情
  search_invoices: Search,
  get_invoice_detail: FileText,
  get_invoice_field_catalog: FileJson,

  // 仪表盘与统计
  get_dashboard_stats: BarChart3,
  get_current_date_context: Calendar,

  // 附件与表格
  list_message_attachments: Paperclip,
  inspect_spreadsheet: Table,

  // 导出功能
  export_invoices: Download,
  create_export_preview: FileDown,
  export_invoices_with_template: FileDown,
  export_pdf_report: FileDown,

  // 发票编辑
  update_invoice: Edit,
  merge_invoices: GitMerge,

  // 标签管理
  get_badge_config: Tag,
  set_badge_config: Tag,
  set_invoice_badge: Tag,

  // 价格配置
  get_price_config: DollarSign,
  set_price_config: DollarSign,

  // 主题设置
  get_theme: Palette,
  set_theme: Palette,

  // 系统功能
  export_logs: FileCode,
  export_backup: HardDrive,
  cleanup_storage: Trash2,
  get_app_info: Info,
  get_sysinfo: Monitor,

  // 用户交互
  ask_user: MessageCircle,
};

export function ToolIcon({ name, className, style }: ToolIconProps) {
  const IconComponent = toolIconMap[name] || Wrench;

  return <IconComponent className={className} style={style} />;
}
