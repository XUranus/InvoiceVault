import React from "react";
import type { EventListResult, EventRow } from "../types";
import { listEvents } from "../api";

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

  const handleReferenceClick = (ev: EventRow) => {
    if (ev.reference_type === "invoice" && ev.reference_id && onNavigateToInvoice) {
      onNavigateToInvoice(ev.reference_id);
    }
  };

  const typeLabel = (t: string) => EVENT_TYPE_LABELS[t] ?? t;
  const statusLabel = (s: string) => STATUS_LABELS[s] ?? s;
  const statusClass = (s: string) => `event-badge-status event-status-${s}`;

  return (
    <div className="page">
      <h2 className="page-title">事件</h2>

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
    </div>
  );
}
