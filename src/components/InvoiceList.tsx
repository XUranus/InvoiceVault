import React from "react";
import type { Invoice, InvoiceSearchParams } from "../types";
import { searchInvoices } from "../api";
import { InvoiceListControls } from "./InvoiceListControls";
import { ExportButton } from "./ExportButton";
import { duplicateStatusMeta, invoiceStatusMeta, toneClass } from "../status";

type Props = {
  invoices: Invoice[];
  onSelectInvoice: (invoiceId: number) => void;
  onRefresh: () => void;
};

export function InvoiceList({ invoices, onSelectInvoice, onRefresh }: Props) {
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
        // fall back to prop data
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

  const handleFilterChange = (newParams: Partial<InvoiceSearchParams>) => {
    setParams((prev) => ({ ...prev, ...newParams, page: 1 }));
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

  const handlePageChange = (page: number) => {
    setParams((prev) => ({ ...prev, page }));
  };

  const selectedIds = new Set<number>();

  return (
    <div className="invoice-list-container">
      <div className="invoice-list-topbar">
        <span className="invoice-count">共 {totalCount} 张发票</span>
        <ExportButton onError={() => {}} onRefresh={onRefresh} />
      </div>
      <InvoiceListControls
        params={params}
        onFilterChange={handleFilterChange}
        onSort={handleSort}
        currentPage={currentPage}
        totalPages={totalPages}
        onPageChange={handlePageChange}
      />
      {loading ? (
        <p className="muted">查询中...</p>
      ) : displayInvoices.length === 0 ? (
        <p className="muted">暂无结构化发票。</p>
      ) : (
        <div className="invoice-list">
          {displayInvoices.map((invoice) => (
            <article
              className="invoice-row"
              key={invoice.id}
              onClick={() => onSelectInvoice(invoice.id)}
              style={{ cursor: "pointer" }}
            >
              <div className="invoice-row-header">
                <strong>
                  {invoice.seller_name ?? invoice.invoice_type ?? "未命名发票"}
                </strong>
                <span className={`mini-tag ${toneClass(duplicateStatusMeta(invoice.duplicate_status).tone)}`}>
                  {duplicateStatusMeta(invoice.duplicate_status).label}
                </span>
                <span className={`mini-tag ${toneClass(invoiceStatusMeta(invoice.status).tone)}`}>
                  {invoiceStatusMeta(invoice.status).label}
                </span>
              </div>
              <span>
                {invoice.issue_date ?? "日期未识别"} ·{" "}
                {invoice.total_amount ?? "金额未识别"} {invoice.currency}
              </span>
              <small>
                {invoice.invoice_number
                  ? `号码 ${invoice.invoice_number}`
                  : "发票号码待确认"}
                {invoice.source_page_range
                  ? ` · 第 ${invoice.source_page_range} 页`
                  : ""}
              </small>
            </article>
          ))}
        </div>
      )}
    </div>
  );
}
