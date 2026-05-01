import React from "react";
import type { Invoice, InvoiceSearchParams } from "../types";
import { searchInvoices, getInvoiceDetail } from "../api";
import { InvoiceListControls } from "./InvoiceListControls";
import { ExportButton } from "./ExportButton";
import { InvoiceDetail } from "./InvoiceDetail";

type Props = {
  invoices: Invoice[];
  onInvoicesChanged: () => void;
  onError: (error: string) => void;
};

export function InvoicesPage({ invoices, onInvoicesChanged, onError }: Props) {
  const [view, setView] = React.useState<"list" | "detail">("list");
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
  }, [params, doSearch]);

  const displayInvoices = searchResult?.invoices ?? invoices;
  const totalCount = searchResult?.total_count ?? invoices.length;
  const currentPage = searchResult?.page ?? 1;
  const totalPages = searchResult?.total_pages ?? 1;

  const handleSelectInvoice = (id: number) => {
    setSelectedId(id);
    setView("detail");
  };

  const handleBack = () => {
    setView("list");
    setSelectedId(null);
    doSearch(params);
    onInvoicesChanged();
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
          onError={onError}
        />
      </div>
    );
  }

  return (
    <div className="page">
      <div className="page-header">
        <h2 className="page-title">发票库</h2>
        <div className="page-header-actions">
          <span className="count-badge">{totalCount} 张</span>
          <ExportButton onError={onError} onRefresh={onInvoicesChanged} />
        </div>
      </div>

      <InvoiceListControls
        params={params}
        onFilterChange={handleFilterChange}
        onSort={handleSort}
        currentPage={currentPage}
        totalPages={totalPages}
        onPageChange={(page) => setParams((prev) => ({ ...prev, page }))}
      />

      {loading ? (
        <p className="muted">查询中...</p>
      ) : displayInvoices.length === 0 ? (
        <div className="empty-state">
          <span className="empty-icon">🧾</span>
          <p>暂无发票记录</p>
          <span className="muted">导入发票文件并点击"识别"后，结构化数据将出现在这里</span>
        </div>
      ) : (
        <div className="invoice-cards">
          {displayInvoices.map((invoice) => (
            <article
              className="invoice-card"
              key={invoice.id}
              onClick={() => handleSelectInvoice(invoice.id)}
            >
              <div className="invoice-card-body">
                <div className="invoice-card-main">
                  <strong>
                    {invoice.seller_name ?? invoice.invoice_type ?? "未命名发票"}
                  </strong>
                  <span className="invoice-card-amount">
                    {invoice.total_amount ? `¥ ${invoice.total_amount}` : "金额未识别"}
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
                </div>
              </div>
              <div className="invoice-card-tags">
                <span className={`mini-tag tag-${invoice.status}`}>
                  {statusLabel(invoice.status)}
                </span>
                {invoice.duplicate_status !== "unique" &&
                invoice.duplicate_status !== "unknown" ? (
                  <span className="mini-tag tag-warn">{dupLabel(invoice.duplicate_status)}</span>
                ) : null}
              </div>
            </article>
          ))}
        </div>
      )}
    </div>
  );
}

function statusLabel(status: string) {
  const labels: Record<string, string> = {
    pending_confirmation: "待确认",
    recognized: "已识别",
    reviewed: "已复核",
    flagged: "已标记",
  };
  return labels[status] ?? status;
}

function dupLabel(status: string) {
  const labels: Record<string, string> = {
    exact_duplicate: "完全重复",
    probable_duplicate: "疑似重复",
    possible_duplicate: "可能重复",
  };
  return labels[status] ?? status;
}
