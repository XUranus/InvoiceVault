import React from "react";
import type { Invoice, InvoiceSearchParams, SimilarResult } from "../types";
import { searchInvoices, getInvoiceDetail, searchInvoicesSemantic } from "../api";
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

  // Semantic search state
  const [semanticQuery, setSemanticQuery] = React.useState("");
  const [semanticResults, setSemanticResults] = React.useState<SimilarResult[] | null>(null);
  const [semanticInvoices, setSemanticInvoices] = React.useState<Invoice[] | null>(null);
  const [semanticLoading, setSemanticLoading] = React.useState(false);
  const [searchMode, setSearchMode] = React.useState<"keyword" | "semantic">("keyword");

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
      // Load full invoice data for each result
      const invoiceData: Invoice[] = [];
      for (const r of results) {
        try {
          const detail = await getInvoiceDetail(r.invoice_id);
          invoiceData.push({
            id: detail.id,
            raw_file_id: detail.raw_file_id,
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
          });
        } catch {
          // Skip invoices we can't load
        }
      }
      setSemanticInvoices(invoiceData);
    } catch (err) {
      setSemanticResults(null);
      setSemanticInvoices(null);
      onError(String(err));
    } finally {
      setSemanticLoading(false);
    }
  };

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

      {searchMode === "semantic" && semanticResults !== null && semanticInvoices !== null ? (
        semanticInvoices.length === 0 ? (
          <p className="muted" style={{ marginTop: 16 }}>未找到语义相似结果。</p>
        ) : (
          <div className="invoice-cards" style={{ marginTop: 16 }}>
            {semanticInvoices.map((invoice, i) => (
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
                    <span className="similarity-badge">
                      相似度: {(semanticResults[i]?.similarity * 100).toFixed(0)}%
                    </span>
                  </div>
                </div>
                <div className="invoice-card-tags">
                  <span className={`mini-tag tag-${invoice.status}`}>
                    {statusLabel(invoice.status)}
                  </span>
                </div>
              </article>
            ))}
          </div>
        )
      ) : loading || semanticLoading ? (
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
