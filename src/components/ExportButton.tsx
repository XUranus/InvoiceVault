import React from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { exportInvoices } from "../api";
import type { ExportResult } from "../types";

type Props = {
  onError: (error: string) => void;
  onRefresh: () => void;
};

export function ExportButton({ onError }: Props) {
  const [exporting, setExporting] = React.useState(false);
  const [lastResult, setLastResult] = React.useState<ExportResult | null>(null);

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
      });
      setLastResult(result);
    } catch (err) {
      onError(String(err));
    } finally {
      setExporting(false);
    }
  };

  return (
    <div className="export-group">
      <button
        className="small-button"
        onClick={() => handleExport("csv")}
        disabled={exporting}
      >
        {exporting ? "导出中..." : "导出 CSV"}
      </button>
      <button
        className="small-button"
        onClick={() => handleExport("xlsx")}
        disabled={exporting}
      >
        {exporting ? "导出中..." : "导出 Excel"}
      </button>
      {lastResult ? (
        <small>
          已导出 {lastResult.row_count} 条到 {lastResult.file_path}
        </small>
      ) : null}
    </div>
  );
}
