export type StatusTone = "success" | "warning" | "danger" | "info" | "neutral";

type StatusMeta = {
  label: string;
  tone: StatusTone;
};

const INVOICE_STATUS_META: Record<string, StatusMeta> = {
  pending_confirmation: { label: "待确认", tone: "warning" },
  pending: { label: "待处理", tone: "warning" },
  recognized: { label: "已识别", tone: "success" },
  reviewed: { label: "已复核", tone: "info" },
  flagged: { label: "已标记", tone: "danger" },
  exported: { label: "已导出", tone: "success" },
  failed: { label: "失败", tone: "danger" },
};

const DUPLICATE_STATUS_META: Record<string, StatusMeta> = {
  unique: { label: "唯一", tone: "success" },
  exact_duplicate: { label: "完全重复", tone: "danger" },
  probable_duplicate: { label: "高度疑似重复", tone: "danger" },
  possible_duplicate: { label: "可能重复", tone: "warning" },
  not_duplicate: { label: "已排除", tone: "info" },
  unknown: { label: "未检测", tone: "neutral" },
};

const IMPORT_STATUS_META: Record<string, StatusMeta> = {
  pending: { label: "等待中", tone: "warning" },
  processing: { label: "处理中", tone: "info" },
  completed: { label: "已完成", tone: "success" },
  duplicate: { label: "重复", tone: "warning" },
  failed: { label: "失败", tone: "danger" },
  recognized: { label: "已识别", tone: "success" },
};

export function invoiceStatusMeta(status: string): StatusMeta {
  return INVOICE_STATUS_META[status] ?? { label: status, tone: "neutral" };
}

export function duplicateStatusMeta(status: string): StatusMeta {
  return DUPLICATE_STATUS_META[status] ?? { label: status, tone: "neutral" };
}

export function importStatusMeta(status: string): StatusMeta {
  return IMPORT_STATUS_META[status] ?? { label: status, tone: "neutral" };
}

export function toneClass(tone: StatusTone): string {
  return `tag-tone-${tone}`;
}

export function shouldShowDuplicateStatus(status: string): boolean {
  return status !== "unique" && status !== "unknown";
}
