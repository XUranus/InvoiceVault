import type { InvoiceSearchParams } from "../types";

type Props = {
  params: InvoiceSearchParams;
  onFilterChange: (params: Partial<InvoiceSearchParams>) => void;
  onSort: (sortBy: string) => void;
  currentPage: number;
  totalPages: number;
  onPageChange: (page: number) => void;
};

const SORT_OPTIONS: { value: string; label: string }[] = [
  { value: "issue_date", label: "日期" },
  { value: "total_amount", label: "金额" },
  { value: "seller_name", label: "销售方" },
  { value: "confidence", label: "置信度" },
  { value: "created_at", label: "入库时间" },
];

const STATUS_OPTIONS: { value: string; label: string }[] = [
  { value: "", label: "全部状态" },
  { value: "pending_confirmation", label: "待确认" },
  { value: "recognized", label: "已识别" },
  { value: "reviewed", label: "已复核" },
  { value: "flagged", label: "已标记" },
];

const DUPLICATE_OPTIONS: { value: string; label: string }[] = [
  { value: "", label: "全部重复状态" },
  { value: "possible_duplicate", label: "可能重复" },
  { value: "probable_duplicate", label: "高度疑似" },
  { value: "exact_duplicate", label: "完全重复" },
  { value: "not_duplicate", label: "已排除" },
  { value: "unique", label: "唯一" },
];

export function InvoiceListControls({
  params,
  onFilterChange,
  onSort,
  currentPage,
  totalPages,
  onPageChange,
}: Props) {
  return (
    <div className="list-controls">
      <div className="controls-row">
        <input
          className="control-input search-input"
          type="search"
          placeholder="搜索发票号码、销售方、购买方..."
          value={params.query ?? ""}
          onChange={(e) => onFilterChange({ query: e.target.value || undefined })}
        />
      </div>
      <div className="controls-row">
        <input
          className="control-input"
          type="date"
          value={params.date_from ?? ""}
          onChange={(e) =>
            onFilterChange({ date_from: e.target.value || undefined })
          }
          placeholder="开始日期"
        />
        <input
          className="control-input"
          type="date"
          value={params.date_to ?? ""}
          onChange={(e) =>
            onFilterChange({ date_to: e.target.value || undefined })
          }
          placeholder="结束日期"
        />
        <select
          className="control-input"
          value={params.status ?? ""}
          onChange={(e) =>
            onFilterChange({ status: e.target.value || undefined })
          }
        >
          {STATUS_OPTIONS.map((opt) => (
            <option key={opt.value} value={opt.value}>
              {opt.label}
            </option>
          ))}
        </select>
        <select
          className="control-input"
          value={params.duplicate_status ?? ""}
          onChange={(e) =>
            onFilterChange({ duplicate_status: e.target.value || undefined })
          }
        >
          {DUPLICATE_OPTIONS.map((opt) => (
            <option key={opt.value} value={opt.value}>
              {opt.label}
            </option>
          ))}
        </select>
      </div>
      <div className="controls-row">
        <div className="sort-group">
          <span className="sort-label">排序:</span>
          {SORT_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              className={`sort-btn ${params.sort_by === opt.value ? "sort-btn-active" : ""}`}
              onClick={() => onSort(opt.value)}
            >
              {opt.label}
              {params.sort_by === opt.value
                ? params.sort_order === "asc"
                  ? " ↑"
                  : " ↓"
                : ""}
            </button>
          ))}
        </div>
        {totalPages > 1 ? (
          <div className="pagination">
            <button
              className="page-btn"
              disabled={currentPage <= 1}
              onClick={() => onPageChange(currentPage - 1)}
            >
              上一页
            </button>
            <span className="page-info">
              {currentPage} / {totalPages}
            </span>
            <button
              className="page-btn"
              disabled={currentPage >= totalPages}
              onClick={() => onPageChange(currentPage + 1)}
            >
              下一页
            </button>
          </div>
        ) : null}
      </div>
    </div>
  );
}
