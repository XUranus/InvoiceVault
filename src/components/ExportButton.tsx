import React from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { exportInvoices, exportPdfReport } from "../api";
import type { ExportResult } from "../types";

type Props = {
  onError: (error: string) => void;
  onRefresh: () => void;
  invoiceIds?: number[];
};

const ALL_COLUMN_KEYS = [
  "invoice_type", "invoice_code", "invoice_number", "issue_date",
  "seller_name", "seller_tax_id", "buyer_name", "buyer_tax_id",
  "currency", "amount_without_tax", "tax_amount", "total_amount",
  "category", "remarks", "source_page_range", "confidence",
  "status", "duplicate_status", "created_at",
];

const COLUMN_LABELS: Record<string, string> = {
  invoice_type: "发票类型",
  invoice_code: "发票代码",
  invoice_number: "发票号码",
  issue_date: "开票日期",
  seller_name: "销售方",
  seller_tax_id: "销售方税号",
  buyer_name: "购买方",
  buyer_tax_id: "购买方税号",
  currency: "币种",
  amount_without_tax: "不含税金额",
  tax_amount: "税额",
  total_amount: "价税合计",
  category: "类别",
  remarks: "备注",
  source_page_range: "页码范围",
  confidence: "置信度",
  status: "状态",
  duplicate_status: "重复状态",
  created_at: "创建时间",
};

const DEFAULT_COLUMNS = [
  "invoice_type", "invoice_number", "issue_date", "seller_name",
  "total_amount", "category", "status", "created_at",
];

export function ExportButton({ onError, invoiceIds }: Props) {
  const [exporting, setExporting] = React.useState(false);
  const [lastResult, setLastResult] = React.useState<ExportResult | null>(null);
  const [open, setOpen] = React.useState(false);
  const [columns, setColumns] = React.useState<string[]>(DEFAULT_COLUMNS);
  const [dateFrom, setDateFrom] = React.useState("");
  const [dateTo, setDateTo] = React.useState("");

  const toggleColumn = (key: string) => {
    setColumns((prev) =>
      prev.includes(key) ? prev.filter((k) => k !== key) : [...prev, key],
    );
  };

  const selectAllColumns = () => setColumns([...ALL_COLUMN_KEYS]);
  const selectDefaults = () => setColumns([...DEFAULT_COLUMNS]);
  const clearColumns = () => setColumns([]);

  const handleExport = async (format: "csv" | "xlsx") => {
    setExporting(true);
    try {
      const ext = format === "csv" ? "csv" : "xlsx";
      const filePath = await save({
        defaultPath: `invoices_export.${ext}`,
        filters: [
          {
            name: format === "csv" ? "CSV 文件" : "Excel 文件",
            extensions: [ext],
          },
        ],
      });

      if (!filePath) {
        setExporting(false);
        return;
      }

      const result = await exportInvoices({
        format,
        output_path: filePath,
        invoice_ids: invoiceIds,
        columns: columns.length === ALL_COLUMN_KEYS.length ? undefined : columns,
        date_from: dateFrom || undefined,
        date_to: dateTo || undefined,
      });
      setLastResult(result);
      setOpen(false);
    } catch (err) {
      onError(String(err));
    } finally {
      setExporting(false);
    }
  };

  const handleExportPdf = async () => {
    setExporting(true);
    try {
      const filePath = await save({
        defaultPath: "invoice-report.pdf",
        filters: [{ name: "PDF 文件", extensions: ["pdf"] }],
      });
      if (!filePath) {
        setExporting(false);
        return;
      }
      await exportPdfReport({
        output_path: filePath,
        invoice_ids: invoiceIds,
        date_from: dateFrom || undefined,
        date_to: dateTo || undefined,
      });
      setOpen(false);
    } catch (err) {
      onError(String(err));
    } finally {
      setExporting(false);
    }
  };

  return (
    <>
      <button className="btn-small" onClick={() => setOpen(true)}>
        导出...
      </button>

      {lastResult ? (
        <small className="export-result">
          已导出 {lastResult.row_count} 条 ({lastResult.columns.length} 列)
        </small>
      ) : null}

      {open && (
        <div className="modal-overlay" onClick={() => setOpen(false)}>
          <div className="modal-card" onClick={(e) => e.stopPropagation()}>
            <h3 className="modal-title">导出发票数据</h3>

            {/* Format buttons */}
            <div className="export-modal-section">
              <span className="export-modal-label">导出格式</span>
              <div className="export-modal-actions">
                <button
                  className="btn-small"
                  onClick={() => handleExport("csv")}
                  disabled={exporting || columns.length === 0}
                >
                  {exporting ? "导出中..." : "CSV"}
                </button>
                <button
                  className="btn-small"
                  onClick={() => handleExport("xlsx")}
                  disabled={exporting || columns.length === 0}
                >
                  {exporting ? "导出中..." : "Excel"}
                </button>
                <button
                  className="btn-small"
                  onClick={handleExportPdf}
                  disabled={exporting}
                >
                  {exporting ? "导出中..." : "PDF 报表"}
                </button>
              </div>
            </div>

            {/* Date range */}
            <div className="export-modal-section">
              <span className="export-modal-label">日期范围</span>
              <div className="export-modal-actions">
                <input
                  type="date"
                  className="control-input"
                  value={dateFrom}
                  onChange={(e) => setDateFrom(e.target.value)}
                  style={{ width: 140 }}
                />
                <span className="muted" style={{ fontSize: 12 }}>至</span>
                <input
                  type="date"
                  className="control-input"
                  value={dateTo}
                  onChange={(e) => setDateTo(e.target.value)}
                  style={{ width: 140 }}
                />
              </div>
            </div>

            {/* Column selection */}
            <div className="export-modal-section">
              <div className="export-modal-label-row">
                <span className="export-modal-label">导出列</span>
                <div className="export-modal-actions" style={{ gap: 4 }}>
                  <button className="btn-small" onClick={selectAllColumns}>全选</button>
                  <button className="btn-small" onClick={selectDefaults}>默认</button>
                  <button className="btn-small" onClick={clearColumns}>清空</button>
                </div>
              </div>
              <div className="export-column-grid">
                {ALL_COLUMN_KEYS.map((key) => (
                  <label key={key} className="export-column-check">
                    <input
                      type="checkbox"
                      checked={columns.includes(key)}
                      onChange={() => toggleColumn(key)}
                    />
                    <span>{COLUMN_LABELS[key] ?? key}</span>
                  </label>
                ))}
              </div>
            </div>

            <div className="modal-actions">
              <button
                className="btn-small"
                onClick={() => setOpen(false)}
                disabled={exporting}
              >
                取消
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
