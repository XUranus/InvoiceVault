import React from "react";
import type { EventListResult, EventRow } from "../types";
import { listEvents, deleteAllEvents } from "../api";
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

export function EventsPage({ onNavigateToInvoice }: Props) {
  const [result, setResult] = React.useState<EventListResult | null>(null);
  const [typeFilter, setTypeFilter] = React.useState<string | null>(null);
  const [page, setPage] = React.useState(1);
  const [clearDialogOpen, setClearDialogOpen] = React.useState(false);
  const [clearing, setClearing] = React.useState(false);
  const pageSize = 20;

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

  const handleReferenceClick = (ev: EventRow) => {
    if (ev.reference_type === "invoice" && ev.reference_id && onNavigateToInvoice) {
      onNavigateToInvoice(ev.reference_id);
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

  const parseSourcePaths = (metadataJson: string | null): string[] => {
    if (!metadataJson) return [];
    try {
      const meta = JSON.parse(metadataJson);
      return meta.source_paths ?? [];
    } catch {
      return [];
    }
  };

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

      {result === null ? (
        <p className="muted">加载中...</p>
      ) : result.events.length === 0 ? (
        <div className="empty-state">
          <span className="empty-icon">📋</span>
          <p>暂无事件</p>
        </div>
      ) : (
        <>
          <div className="event-list">
            {result.events.map((ev) => (
              <div
                key={ev.id}
                className={`event-card ${ev.reference_type === "invoice" ? "event-card-clickable" : ""}`}
                onClick={() => handleReferenceClick(ev)}
              >
                <div className="event-card-header">
                  <span className={`event-type-badge event-type-${ev.event_type}`}>
                    {typeLabel(ev.event_type)}
                  </span>
                  <span className={statusClass(ev.status)}>
                    {statusLabel(ev.status)}
                  </span>
                  <span className="event-time">{ev.created_at}</span>
                </div>
                <div className="event-card-body">
                  <strong>{ev.title}</strong>
                  {ev.description ? <span className="event-desc">{ev.description}</span> : null}
                </div>
                {ev.reference_type === "invoice" && ev.reference_id ? (
                  <div className="event-card-footer">
                    <span className="event-ref">
                      发票 #{ev.reference_id} — 点击查看详情
                    </span>
                  </div>
                ) : null}
                {ev.event_type === "import" && ev.metadata_json ? (
                  <SourcePaths paths={parseSourcePaths(ev.metadata_json)} onOpen={openFolder} />
                ) : null}
              </div>
            ))}
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
    <div className="event-card-footer">
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
    </div>
  );
}
