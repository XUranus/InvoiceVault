import React from "react";
import type { Invoice, InvoiceSearchParams, SimilarResult } from "../types";
import {
  searchInvoices,
  getInvoiceDetail,
  searchInvoicesSemantic,
  batchUpdateInvoices,
  batchDeleteInvoices,
} from "../api";
import { InvoiceListControls } from "./InvoiceListControls";
import { ExportButton } from "./ExportButton";
import { InvoiceDetail } from "./InvoiceDetail";
import { ConfirmDialog } from "./ConfirmDialog";
import { useAppStore } from "../stores/appStore";
import { useRefreshStore } from "../stores/refreshStore";
import {
  duplicateStatusMeta,
  invoiceStatusMeta,
  shouldShowDuplicateStatus,
  toneClass,
} from "../status";

const BATCH_STATUS_OPTIONS = [
  { value: "", label: "不修改状态" },
  { value: "pending_confirmation", label: "待确认" },
  { value: "recognized", label: "已识别" },
  { value: "reviewed", label: "已复核" },
  { value: "flagged", label: "已标记" },
];

const BATCH_CATEGORY_OPTIONS = [
  { value: "", label: "不修改类别" },
  { value: "办公用品", label: "办公用品" },
  { value: "差旅费", label: "差旅费" },
  { value: "餐饮", label: "餐饮" },
  { value: "交通", label: "交通" },
  { value: "通讯", label: "通讯" },
  { value: "房租", label: "房租" },
  { value: "水电", label: "水电" },
  { value: "物流", label: "物流" },
  { value: "广告", label: "广告" },
  { value: "其他", label: "其他" },
];

export function InvoicesPage() {
  const invoices = useAppStore((s) => s.invoices);
  const refreshInvoices = useAppStore((s) => s.refreshInvoices);
  const setError = useAppStore((s) => s.setError);
  const refreshKey = useRefreshStore((s) => s.invoicesKey);
  const decrementInvoiceBadgeCount = useAppStore((s) => s.decrementInvoiceBadgeCount);
  const [view, setView] = React.useState<"list" | "detail">("list");
  const [viewMode, setViewMode] = React.useState<"cards" | "table">("table");
  const [selectedId, setSelectedId] = React.useState<number | null>(null);
  const [searchResult, setSearchResult] = React.useState<{
    invoices: Invoice[];
    total_count: number;
    page: number;
    page_size: number;
    total_pages: number;
  } | null>(null);
  const [params, setParams] = React.useState<InvoiceSearchParams>({
    page: 1,
    page_size: 20,
  });
  const [loading, setLoading] = React.useState(false);

  // Semantic search state
  const [semanticQuery, setSemanticQuery] = React.useState("");
  const [semanticResults, setSemanticResults] = React.useState<SimilarResult[] | null>(null);
  const [semanticInvoices, setSemanticInvoices] = React.useState<Invoice[] | null>(null);
  const [semanticLoading, setSemanticLoading] = React.useState(false);
  const [searchMode, setSearchMode] = React.useState<"keyword" | "semantic">("keyword");

  // Batch selection state
  const [selected, setSelected] = React.useState<Set<number>>(new Set());
  const [batchStatus, setBatchStatus] = React.useState("");
  const [batchCategory, setBatchCategory] = React.useState("");
  const [batchApplying, setBatchApplying] = React.useState(false);
  const [deleteDialogOpen, setDeleteDialogOpen] = React.useState(false);
  const [batchDeleting, setBatchDeleting] = React.useState(false);
  const [localError, setLocalError] = React.useState<string | null>(null);

  const showError = (err: string) => {
    setLocalError(err);
    setError(err);
  };

  const doSearch = React.useCallback(
    async (p: InvoiceSearchParams) => {
      setLoading(true);
      try {
        const result = await searchInvoices(p);
        setSearchResult(result);
      } catch {
        setSearchResult(null);
      } finally {
        setLoading(false);
      }
    },
    [],
  );

  React.useEffect(() => {
    doSearch(params);
  }, [params, doSearch, refreshKey]);

  const handleSemanticSearch = async () => {
    if (!semanticQuery.trim()) {
      setSemanticResults(null);
      setSemanticInvoices(null);
      return;
    }
    setSemanticLoading(true);
    try {
      const results = await searchInvoicesSemantic(semanticQuery.trim(), 20);
      setSemanticResults(results);
      const invoiceData: Invoice[] = [];
      for (const r of results) {
        try {
          const detail = await getInvoiceDetail(r.invoice_id);
          invoiceData.push({
            id: detail.id,
            raw_file_id: detail.raw_file_id,
            raw_file_mime: detail.raw_file_mime,
            invoice_type: detail.invoice_type,
            invoice_code: detail.invoice_code,
            invoice_number: detail.invoice_number,
            issue_date: detail.issue_date,
            seller_name: detail.seller_name,
            buyer_name: detail.buyer_name,
            currency: detail.currency,
            total_amount: detail.total_amount,
            category: detail.category,
            source_page_range: detail.source_page_range,
            confidence: detail.confidence,
            status: detail.status,
            duplicate_status: detail.duplicate_status,
            created_at: detail.created_at,
            updated_at: detail.updated_at,
            badges: detail.badges,
          });
        } catch {
          // Skip invoices we can't load
        }
      }
      setSemanticInvoices(invoiceData);
    } catch (err) {
      setSemanticResults(null);
      setSemanticInvoices(null);
      showError(String(err));
    } finally {
      setSemanticLoading(false);
    }
  };

  const displayInvoices = searchResult?.invoices ?? invoices;
  const totalCount = searchResult?.total_count ?? invoices.length;
  const currentPage = searchResult?.page ?? 1;
  const totalPages = searchResult?.total_pages ?? 1;
  const hasSemanticResults =
    searchMode === "semantic" && semanticResults !== null && semanticInvoices !== null;
  const activeInvoices = hasSemanticResults && semanticInvoices ? semanticInvoices : displayInvoices;
  const activeSimilarities = hasSemanticResults && semanticResults ? semanticResults : null;

  const handleViewModeChange = (mode: "cards" | "table") => {
    setViewMode(mode);
    localStorage.setItem("invoiceViewMode", mode);
  };

  // Clear selection when invoices change
  React.useEffect(() => {
    setSelected(new Set());
  }, [displayInvoices, semanticInvoices, searchMode]);

  const toggleSelect = (id: number) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const toggleSelectAll = () => {
    if (selected.size === activeInvoices.length) {
      setSelected(new Set());
    } else {
      setSelected(new Set(activeInvoices.map((inv) => inv.id)));
    }
  };

  const handleBatchApply = async () => {
    if (!batchStatus && !batchCategory) return;
    setBatchApplying(true);
    try {
      await batchUpdateInvoices({
        ids: Array.from(selected),
        status: batchStatus || null,
        category: batchCategory || null,
      });
      setSelected(new Set());
      setBatchStatus("");
      setBatchCategory("");
      doSearch(params);
      refreshInvoices();
    } catch (err) {
      showError(String(err));
    } finally {
      setBatchApplying(false);
    }
  };

  const handleBatchDelete = async () => {
    setBatchDeleting(true);
    try {
      await batchDeleteInvoices(Array.from(selected));
      setSelected(new Set());
      setDeleteDialogOpen(false);
      doSearch(params);
      refreshInvoices();
    } catch (err) {
      showError(String(err));
    } finally {
      setBatchDeleting(false);
    }
  };

  const handleSelectInvoice = (id: number) => {
    setSelectedId(id);
    setView("detail");
    decrementInvoiceBadgeCount();
  };

  React.useEffect(() => {
    const targetInvoiceId = sessionStorage.getItem("focusInvoiceId");
    if (!targetInvoiceId) return;

    const id = Number(targetInvoiceId);
    sessionStorage.removeItem("focusInvoiceId");
    if (Number.isFinite(id) && id > 0) {
      handleSelectInvoice(id);
    }
  }, [refreshKey]);

  const handleBack = () => {
    setView("list");
    setSelectedId(null);
    doSearch(params);
    refreshInvoices();
  };

  const handleFilterChange = (p: Partial<InvoiceSearchParams>) => {
    setParams((prev) => ({ ...prev, ...p, page: 1 }));
  };

  const handleSort = (sortBy: string) => {
    setParams((prev) => ({
      ...prev,
      sort_by: sortBy,
      sort_order:
        prev.sort_by === sortBy && prev.sort_order === "asc" ? "desc" : "asc",
      page: 1,
    }));
  };

  if (view === "detail" && selectedId !== null) {
    return (
      <div className="page">
        <InvoiceDetail
          invoiceId={selectedId}
          onBack={handleBack}
          onError={setError}
        />
      </div>
    );
  }

  return (
    <div className="page">
      {localError ? (
        <div className="alert alert-error" style={{ marginBottom: 12 }}>
          {localError}
          <button className="alert-dismiss" onClick={() => setLocalError(null)}>×</button>
        </div>
      ) : null}

      <div className="page-header">
        <h2 className="page-title">发票库</h2>
        <div className="page-header-actions">
          <span className="count-badge">{totalCount} 张</span>
          <div className="view-toggle" aria-label="发票库视图切换">
            <button
              className={`view-toggle-btn ${viewMode === "cards" ? "active" : ""}`}
              onClick={() => handleViewModeChange("cards")}
              type="button"
            >
              卡片
            </button>
            <button
              className={`view-toggle-btn ${viewMode === "table" ? "active" : ""}`}
              onClick={() => handleViewModeChange("table")}
              type="button"
            >
              表格
            </button>
          </div>
          <ExportButton
            onError={showError}
            onRefresh={refreshInvoices}
            invoiceIds={selected.size > 0 ? Array.from(selected) : undefined}
          />
        </div>
      </div>

      {/* Batch action bar */}
      {selected.size > 0 && (
        <div className="batch-bar">
          <span className="batch-bar-count">已选 {selected.size} 张</span>
          <select
            value={batchStatus}
            onChange={(e) => setBatchStatus(e.target.value)}
          >
            {BATCH_STATUS_OPTIONS.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
          <select
            value={batchCategory}
            onChange={(e) => setBatchCategory(e.target.value)}
          >
            {BATCH_CATEGORY_OPTIONS.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
          <button
            className="btn-primary"
            onClick={handleBatchApply}
            disabled={batchApplying || (!batchStatus && !batchCategory)}
          >
            {batchApplying ? "应用中..." : "批量应用"}
          </button>
          <button
            className="btn-danger"
            onClick={() => setDeleteDialogOpen(true)}
          >
            批量删除
          </button>
          <button
            className="btn-small"
            onClick={() => {
              setSelected(new Set());
              setDeleteDialogOpen(false);
            }}
          >
            取消选择
          </button>
        </div>
      )}

      {/* Search mode tabs */}
      <div className="search-mode-tabs">
        <button
          className={`search-mode-tab ${searchMode === "keyword" ? "active" : ""}`}
          onClick={() => setSearchMode("keyword")}
        >
          关键词搜索
        </button>
        <button
          className={`search-mode-tab ${searchMode === "semantic" ? "active" : ""}`}
          onClick={() => setSearchMode("semantic")}
        >
          语义搜索
        </button>
      </div>

      {searchMode === "semantic" ? (
        <div className="semantic-search-bar">
          <input
            value={semanticQuery}
            onChange={(e) => setSemanticQuery(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleSemanticSearch()}
            placeholder="输入自然语言描述，如：办公用品 纸张 2026年"
            spellCheck={false}
          />
          <button
            className="btn-primary"
            onClick={handleSemanticSearch}
            disabled={semanticLoading}
          >
            {semanticLoading ? "搜索中..." : "语义搜索"}
          </button>
        </div>
      ) : (
        <InvoiceListControls
          params={params}
          onFilterChange={handleFilterChange}
          onSort={handleSort}
          currentPage={currentPage}
          totalPages={totalPages}
          onPageChange={(page) => setParams((prev) => ({ ...prev, page }))}
        />
      )}

      {/* Select all */}
      {activeInvoices.length > 0 && (
        <label className="select-all-row">
          <input
            type="checkbox"
            checked={selected.size === activeInvoices.length && activeInvoices.length > 0}
            onChange={toggleSelectAll}
          />
          <span>全选</span>
        </label>
      )}

      {hasSemanticResults && activeInvoices.length === 0 ? (
          <p className="muted" style={{ marginTop: 16 }}>未找到语义相似结果。</p>
      ) : loading || semanticLoading ? (
        <p className="muted">查询中...</p>
      ) : activeInvoices.length === 0 ? (
        <div className="empty-state">
          <span className="empty-icon">🧾</span>
          <p>暂无发票记录</p>
          <span className="muted">导入发票文件并点击"识别"后，结构化数据将出现在这里</span>
        </div>
      ) : viewMode === "table" ? (
        <InvoiceTableView
          invoices={activeInvoices}
          similarities={activeSimilarities}
          selected={selected}
          onToggleSelect={toggleSelect}
          onSelectInvoice={handleSelectInvoice}
          onSort={hasSemanticResults ? undefined : handleSort}
        />
      ) : (
        <InvoiceCardView
          invoices={activeInvoices}
          similarities={activeSimilarities}
          selected={selected}
          onToggleSelect={toggleSelect}
          onSelectInvoice={handleSelectInvoice}
        />
      )}

      {/* Delete confirmation dialog */}
      <ConfirmDialog
        open={deleteDialogOpen}
        title="确认删除"
        message={`确定要删除选中的 ${selected.size} 张发票吗？此操作不可撤销。`}
        detail={getSelectedSummary(activeInvoices, selected)}
        confirmLabel="删除"
        danger
        loading={batchDeleting}
        onConfirm={handleBatchDelete}
        onCancel={() => setDeleteDialogOpen(false)}
      />
    </div>
  );
}

function InvoiceCardView({
  invoices,
  similarities,
  selected,
  onToggleSelect,
  onSelectInvoice,
}: {
  invoices: Invoice[];
  similarities: SimilarResult[] | null;
  selected: Set<number>;
  onToggleSelect: (id: number) => void;
  onSelectInvoice: (id: number) => void;
}) {
  return (
    <div className="invoice-cards">
      {invoices.map((invoice, i) => (
        <article
          className={`invoice-card ${selected.has(invoice.id) ? "invoice-card-selected" : ""}`}
          key={invoice.id}
          onClick={() => onSelectInvoice(invoice.id)}
        >
          <label className="invoice-card-check" onClick={(e) => e.stopPropagation()}>
            <input
              type="checkbox"
              checked={selected.has(invoice.id)}
              onChange={() => onToggleSelect(invoice.id)}
            />
          </label>
          <div className="invoice-card-body">
            <div className="invoice-card-main">
              <strong>
                {invoice.seller_name ?? invoice.invoice_type ?? "未命名发票"}
              </strong>
              <span className="invoice-card-amount">
                {formatInvoiceAmount(invoice)}
              </span>
            </div>
            <div className="invoice-card-meta">
              <span>{invoice.issue_date ?? "日期未识别"}</span>
              <span>
                {invoice.invoice_number
                  ? `No. ${invoice.invoice_number}`
                  : "号码待确认"}
              </span>
              {invoice.source_page_range ? (
                <span>第 {invoice.source_page_range} 页</span>
              ) : null}
              {similarities ? (
                <span className="similarity-badge">
                  相似度: {formatSimilarity(similarities[i]?.similarity)}
                </span>
              ) : null}
            </div>
          </div>
          <div className="invoice-card-tags">
            <span className={`mini-tag ${toneClass(invoiceStatusMeta(invoice.status).tone)}`}>
              {invoiceStatusMeta(invoice.status).label}
            </span>
            {shouldShowDuplicateStatus(invoice.duplicate_status) ? (
              <span className={`mini-tag ${toneClass(duplicateStatusMeta(invoice.duplicate_status).tone)}`}>
                {duplicateStatusMeta(invoice.duplicate_status).label}
              </span>
            ) : null}
          </div>
        </article>
      ))}
    </div>
  );
}

function InvoiceTableView({
  invoices,
  similarities,
  selected,
  onToggleSelect,
  onSelectInvoice,
  onSort,
}: {
  invoices: Invoice[];
  similarities: SimilarResult[] | null;
  selected: Set<number>;
  onToggleSelect: (id: number) => void;
  onSelectInvoice: (id: number) => void;
  onSort?: (sortBy: string) => void;
}) {
  const sortableHeader = (label: string, sortBy: string) => (
    <button
      className="invoice-table-sort"
      type="button"
      onClick={() => onSort?.(sortBy)}
      disabled={!onSort}
    >
      {label}
    </button>
  );

  return (
    <div className="invoice-table-wrap">
      <table className={`invoice-table ${similarities ? "invoice-table-with-similarity" : ""}`}>
        <colgroup>
          <col className="invoice-table-col-check" />
          <col className="invoice-table-col-company" />
          <col className="invoice-table-col-date" />
          <col className="invoice-table-col-code" />
          <col className="invoice-table-col-amount" />
          <col className="invoice-table-col-tags" />
          {similarities ? <col className="invoice-table-col-similarity" /> : null}
        </colgroup>
        <thead>
          <tr>
            <th className="invoice-table-check-col" aria-label="选择" />
            <th>{sortableHeader("公司名称", "seller_name")}</th>
            <th>{sortableHeader("时间", "issue_date")}</th>
            <th>{sortableHeader("发票编码", "invoice_number")}</th>
            <th>{sortableHeader("金额", "total_amount")}</th>
            <th>标签</th>
            {similarities ? <th>相似度</th> : null}
          </tr>
        </thead>
        <tbody>
          {invoices.map((invoice, i) => (
            <tr
              key={invoice.id}
              className={selected.has(invoice.id) ? "invoice-table-row-selected" : ""}
              onClick={() => onSelectInvoice(invoice.id)}
            >
              <td onClick={(e) => e.stopPropagation()}>
                <input
                  type="checkbox"
                  checked={selected.has(invoice.id)}
                  onChange={() => onToggleSelect(invoice.id)}
                />
              </td>
              <td className="invoice-table-company">
                <strong>{invoice.seller_name ?? invoice.buyer_name ?? "未命名发票"}</strong>
                {invoice.buyer_name ? (
                  <span className="invoice-table-subtext">购买方：{invoice.buyer_name}</span>
                ) : null}
              </td>
              <td className="invoice-table-date">{invoice.issue_date ?? "未识别"}</td>
              <td className="invoice-table-code">
                <InvoiceCodeCell invoice={invoice} />
              </td>
              <td className="invoice-table-amount">{formatInvoiceAmount(invoice)}</td>
              <td>
                <InvoiceTagBadges invoice={invoice} />
              </td>
              {similarities ? (
                <td className="similarity-badge">
                  {formatSimilarity(similarities[i]?.similarity)}
                </td>
              ) : null}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function InvoiceCodeCell({ invoice }: { invoice: Invoice }) {
  if (!invoice.invoice_code && !invoice.invoice_number) {
    return <span className="invoice-code-empty">待确认</span>;
  }

  return (
    <div className="invoice-code-cell" title={formatInvoiceCode(invoice)}>
      {invoice.invoice_code ? (
        <div className="invoice-code-line">
          <span className="invoice-code-label">代码</span>
          <span className="invoice-code-value">{invoice.invoice_code}</span>
        </div>
      ) : null}
      {invoice.invoice_number ? (
        <div className="invoice-code-line">
          <span className="invoice-code-label">号码</span>
          <span className="invoice-code-value">{invoice.invoice_number}</span>
        </div>
      ) : null}
    </div>
  );
}

function formatInvoiceAmount(invoice: Invoice): string {
  if (!invoice.total_amount) return "金额未识别";
  return `${invoice.currency || "¥"} ${invoice.total_amount}`;
}

function formatInvoiceCode(invoice: Invoice): string {
  if (invoice.invoice_code && invoice.invoice_number) {
    return `${invoice.invoice_code} / ${invoice.invoice_number}`;
  }
  return invoice.invoice_code ?? invoice.invoice_number ?? "待确认";
}

function InvoiceTagBadges({ invoice }: { invoice: Invoice }) {
  const tags = buildInvoiceTags(invoice);
  if (tags.length === 0) {
    return <span className="muted">-</span>;
  }
  return (
    <div className="invoice-table-tags">
      {tags.map((tag) => (
        <span key={`${tag.label}-${tag.tone}`} className={`mini-tag ${toneClass(tag.tone)}`}>
          {tag.label}
        </span>
      ))}
    </div>
  );
}

function buildInvoiceTags(invoice: Invoice): Array<{ label: string; tone: "success" | "warning" | "danger" | "info" | "neutral" }> {
  const tags: Array<{ label: string; tone: "success" | "warning" | "danger" | "info" | "neutral" }> = [];
  const fileTag = fileTypeTag(invoice.raw_file_mime);
  if (fileTag) tags.push(fileTag);

  const invoiceType = invoice.invoice_type ?? "";
  if (invoiceType.includes("专用")) {
    tags.push({ label: "专票", tone: "info" });
  } else if (invoiceType.includes("普通")) {
    tags.push({ label: "普票", tone: "info" });
  } else if (invoiceType) {
    tags.push({ label: invoiceType, tone: "neutral" });
  }

  if (invoice.category) {
    tags.push({ label: invoice.category, tone: "neutral" });
  }

  for (const badge of invoice.badges ?? []) {
    tags.push({ label: `${badge.group_name}: ${badge.value}`, tone: "neutral" });
  }

  return tags;
}

function fileTypeTag(mimeType: string | null): { label: string; tone: "neutral" | "info" } | null {
  if (!mimeType) return null;
  if (mimeType === "application/pdf") return { label: "PDF", tone: "neutral" };
  if (mimeType.startsWith("image/")) return { label: "图片", tone: "neutral" };
  return { label: "文件", tone: "neutral" };
}

function formatSimilarity(value: number | undefined): string {
  if (typeof value !== "number" || Number.isNaN(value)) return "-";
  return `${(value * 100).toFixed(0)}%`;
}

function getSelectedSummary(
  invoices: Invoice[],
  selected: Set<number>,
): string {
  const selectedInvoices = invoices.filter((inv) => selected.has(inv.id));
  const preview = selectedInvoices
    .slice(0, 5)
    .map((inv) => inv.seller_name ?? inv.invoice_type ?? "未命名")
    .join("、");
  if (selectedInvoices.length <= 5) return preview;
  return preview + ` 等 ${selectedInvoices.length} 张`;
}

export default InvoicesPage;
