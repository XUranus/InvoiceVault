import React from "react";
import type { EventListResult, EventRow } from "../types";
import { listEvents, deleteAllEvents, getInvoiceIdByRawFile } from "../api";
import { ConfirmDialog } from "./ConfirmDialog";

type Props = {
  onNavigateToInvoice?: (id: number) => void;
};

const EVENT_TYPE_LABELS: Record<string, string> = {
  import: "导入",
  recognition: "识别",
  config_change: "配置变更",
  agent: "Agent",
  export: "导出",
};

const STATUS_LABELS: Record<string, string> = {
  completed: "完成",
  failed: "失败",
  pending: "进行中",
  running: "进行中",
};

type ImportEventMetadata = {
  source_paths?: string[];
  raw_file_ids?: number[];
  invoice_ids?: number[];
};

export function EventsPage({ onNavigateToInvoice }: Props) {
  const [result, setResult] = React.useState<EventListResult | null>(null);
  const [typeFilter, setTypeFilter] = React.useState<string | null>(null);
  const [page, setPage] = React.useState(1);
  const [eventError, setEventError] = React.useState<string | null>(null);
  const [clearDialogOpen, setClearDialogOpen] = React.useState(false);
  const [clearing, setClearing] = React.useState(false);
  const pageSize = 10;

  const fetchEvents = React.useCallback(() => {
    listEvents(page, pageSize, typeFilter ?? undefined)
      .then(setResult)
      .catch(() => {});
  }, [page, typeFilter]);

  React.useEffect(() => {
    fetchEvents();
  }, [fetchEvents]);

  const handleFilter = (t: string | null) => {
    setTypeFilter(t);
    setPage(1);
  };

  const handleClearAll = async () => {
    setClearing(true);
    try {
      await deleteAllEvents();
      setClearDialogOpen(false);
      fetchEvents();
    } catch {
      // ignore
    } finally {
      setClearing(false);
    }
  };

  const handleReferenceClick = async (ev: EventRow, metadata: ImportEventMetadata) => {
    setEventError(null);
    if (ev.reference_type === "invoice" && ev.reference_id && onNavigateToInvoice) {
      onNavigateToInvoice(ev.reference_id);
      return;
    }

    const metadataInvoiceId = metadata.invoice_ids?.[0];
    if (metadataInvoiceId && onNavigateToInvoice) {
      onNavigateToInvoice(metadataInvoiceId);
      return;
    }

    const rawFileId = metadata.raw_file_ids?.[0];
    if (!rawFileId || !onNavigateToInvoice) {
      return;
    }

    try {
      const invoiceId = await getInvoiceIdByRawFile(rawFileId);
      if (invoiceId) {
        onNavigateToInvoice(invoiceId);
      } else {
        setEventError("该导入事件还没有可打开的发票详情，请等待识别完成后再试。");
      }
    } catch (err) {
      setEventError(String(err));
    }
  };

  const openFolder = async (filePath: string) => {
    try {
      const { open } = await import("@tauri-apps/plugin-shell");
      // Open the parent directory of the file
      const parentDir = filePath.replace(/[/\\][^/\\]*$/, "") || filePath;
      await open(parentDir);
    } catch {
      // ignore
    }
  };

  const typeLabel = (t: string) => EVENT_TYPE_LABELS[t] ?? t;
  const statusLabel = (s: string) => STATUS_LABELS[s] ?? s;
  const statusClass = (s: string) => `event-badge-status event-status-${s}`;

  const parseImportMetadata = (metadataJson: string | null): ImportEventMetadata => {
    if (!metadataJson) return {};
    try {
      const meta = JSON.parse(metadataJson);
      return {
        source_paths: Array.isArray(meta.source_paths) ? meta.source_paths : [],
        raw_file_ids: Array.isArray(meta.raw_file_ids) ? meta.raw_file_ids : [],
        invoice_ids: Array.isArray(meta.invoice_ids) ? meta.invoice_ids : [],
      };
    } catch {
      return {};
    }
  };

  const canNavigateToInvoice = (ev: EventRow, metadata: ImportEventMetadata): boolean =>
    (ev.reference_type === "invoice" && Boolean(ev.reference_id)) ||
    Boolean(metadata.invoice_ids?.length) ||
    Boolean(metadata.raw_file_ids?.length);

  return (
    <div className="page">
      <div className="page-header">
        <h2 className="page-title">事件</h2>
        <div className="page-header-actions">
          {result && result.total_count > 0 ? (
            <button className="btn-danger" onClick={() => setClearDialogOpen(true)}>
              清空全部事件
            </button>
          ) : null}
        </div>
      </div>

      <div className="event-filters">
        <button
          className={`btn-small ${!typeFilter ? "sort-btn-active" : ""}`}
          onClick={() => handleFilter(null)}
        >
          全部
        </button>
        {Object.entries(EVENT_TYPE_LABELS).map(([key, label]) => (
          <button
            key={key}
            className={`btn-small ${typeFilter === key ? "sort-btn-active" : ""}`}
            onClick={() => handleFilter(key)}
          >
            {label}
          </button>
        ))}
      </div>

      {eventError ? (
        <div className="alert alert-warn">
          {eventError}
          <button className="alert-dismiss" onClick={() => setEventError(null)}>×</button>
        </div>
      ) : null}

      {result === null ? (
        <p className="muted">加载中...</p>
      ) : result.events.length === 0 ? (
        <div className="empty-state">
          <span className="empty-icon">📋</span>
          <p>暂无事件</p>
        </div>
      ) : (
        <>
          <div className="event-table-wrap">
            <table className="event-table">
              <thead>
                <tr>
                  <th>类型</th>
                  <th>状态</th>
                  <th>标题</th>
                  <th>详情</th>
                  <th>关联</th>
                  <th>时间</th>
                </tr>
              </thead>
              <tbody>
            {result.events.map((ev) => {
              const metadata = parseImportMetadata(ev.metadata_json);
              const navigable = canNavigateToInvoice(ev, metadata);
              const referenceId = ev.reference_type === "invoice"
                ? ev.reference_id
                : metadata.invoice_ids?.[0] ?? null;
              return (
                <tr
                  key={ev.id}
                  className={navigable ? "event-row-clickable" : ""}
                  onClick={() => handleReferenceClick(ev, metadata)}
                >
                  <td>
                    <span className={`event-type-badge event-type-${ev.event_type}`}>
                      {typeLabel(ev.event_type)}
                    </span>
                  </td>
                  <td>
                    <span className={statusClass(ev.status)}>
                      {statusLabel(ev.status)}
                    </span>
                  </td>
                  <td className="event-table-title">{ev.title}</td>
                  <td className="event-table-desc">
                    {ev.description ? <span>{ev.description}</span> : <span className="muted">-</span>}
                    {ev.event_type === "import" && ev.metadata_json ? (
                      <SourcePaths paths={metadata.source_paths ?? []} onOpen={openFolder} />
                    ) : null}
                  </td>
                  <td>
                  {navigable ? (
                      <button
                        className="event-ref-button"
                        type="button"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleReferenceClick(ev, metadata);
                        }}
                      >
                        {referenceId ? `发票 #${referenceId}` : "导入发票"}
                      </button>
                    ) : (
                      <span className="muted">-</span>
                    )}
                  </td>
                  <td className="event-time">{ev.created_at}</td>
                </tr>
              );
            })}
              </tbody>
            </table>
          </div>

          {result.total_pages > 1 ? (
            <div className="pagination" style={{ marginTop: 16 }}>
              <button
                className="page-btn"
                disabled={page <= 1}
                onClick={() => setPage((p) => Math.max(1, p - 1))}
              >
                上一页
              </button>
              <span className="page-info">
                {result.page} / {result.total_pages}（共 {result.total_count} 条）
              </span>
              <button
                className="page-btn"
                disabled={page >= result.total_pages}
                onClick={() => setPage((p) => p + 1)}
              >
                下一页
              </button>
            </div>
          ) : null}
        </>
      )}

      <ConfirmDialog
        open={clearDialogOpen}
        title="清空全部事件"
        message="确定要删除所有事件记录吗？此操作不可撤销。"
        confirmLabel="清空"
        danger
        loading={clearing}
        onConfirm={handleClearAll}
        onCancel={() => setClearDialogOpen(false)}
      />
    </div>
  );
}

function SourcePaths({ paths, onOpen }: { paths: string[]; onOpen: (path: string) => void }) {
  if (paths.length === 0) return null;
  return (
      <div className="event-source-paths">
        {paths.map((p, i) => (
          <span
            key={i}
            className="event-source-path"
            onClick={(e) => {
              e.stopPropagation();
              onOpen(p);
            }}
            title={p}
          >
            {p}
          </span>
        ))}
      </div>
  );
}
