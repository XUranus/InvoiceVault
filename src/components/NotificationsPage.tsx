import React from "react";
import type { NotificationRow } from "../types";
import {
  listNotifications,
  markNotificationRead,
  markAllNotificationsRead,
  dismissNotification,
  deleteAllNotifications,
} from "../api";
import { ConfirmDialog } from "./ConfirmDialog";

type Props = {
  onNavigateToInvoice?: (id: number) => void;
  onUnreadCountChange?: (count: number) => void;
};

const LEVEL_LABELS: Record<string, string> = {
  info: "通知",
  warning: "告警",
  error: "错误",
};

export function NotificationsPage({ onNavigateToInvoice, onUnreadCountChange }: Props) {
  const [notifications, setNotifications] = React.useState<NotificationRow[]>([]);
  const [page, setPage] = React.useState(1);
  const [clearDialogOpen, setClearDialogOpen] = React.useState(false);
  const [clearing, setClearing] = React.useState(false);
  const pageSize = 10;

  const fetch = React.useCallback(() => {
    listNotifications().then((list) => {
      setNotifications(list);
      const unread = list.filter((n) => !n.is_read).length;
      onUnreadCountChange?.(unread);
    }).catch(() => {});
  }, [onUnreadCountChange]);

  React.useEffect(() => {
    fetch();
  }, [fetch]);

  React.useEffect(() => {
    setPage(1);
  }, [notifications.length]);

  const handleMarkRead = async (id: number) => {
    await markNotificationRead(id).catch(() => {});
    fetch();
  };

  const handleMarkAllRead = async () => {
    await markAllNotificationsRead().catch(() => {});
    fetch();
  };

  const handleDismiss = async (id: number) => {
    await dismissNotification(id).catch(() => {});
    fetch();
  };

  const handleClearAll = async () => {
    setClearing(true);
    try {
      await deleteAllNotifications();
      setClearDialogOpen(false);
      fetch();
    } catch {
      // ignore
    } finally {
      setClearing(false);
    }
  };

  const handleReferenceClick = (n: NotificationRow) => {
    if (!n.is_read) handleMarkRead(n.id);
    if (n.reference_type === "invoice" && n.reference_id && onNavigateToInvoice) {
      onNavigateToInvoice(n.reference_id);
    }
  };

  const unreadCount = notifications.filter((n) => !n.is_read).length;
  const totalPages = Math.max(1, Math.ceil(notifications.length / pageSize));
  const currentPage = Math.min(page, totalPages);
  const pagedNotifications = notifications.slice(
    (currentPage - 1) * pageSize,
    currentPage * pageSize,
  );

  return (
    <div className="page">
      <div className="page-header">
        <h2 className="page-title">通知</h2>
        <div className="page-header-actions">
          {unreadCount > 0 ? (
            <span className="count-badge">{unreadCount} 条未读</span>
          ) : null}
          {unreadCount > 0 ? (
            <button className="btn-small" onClick={handleMarkAllRead}>
              全部已读
            </button>
          ) : null}
          {notifications.length > 0 ? (
            <button className="btn-danger" onClick={() => setClearDialogOpen(true)}>
              清空全部通知
            </button>
          ) : null}
        </div>
      </div>

      {notifications.length === 0 ? (
        <div className="empty-state">
          <span className="empty-icon">🔔</span>
          <p>暂无通知</p>
        </div>
      ) : (
        <div className="notification-table-wrap">
          <table className="notification-table">
            <thead>
              <tr>
                <th>级别</th>
                <th>标题</th>
                <th>内容</th>
                <th>时间</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {pagedNotifications.map((n) => (
                <tr
                  key={n.id}
                  className={`notification-row notification-level-${n.level} ${!n.is_read ? "notification-unread" : ""}`}
                  onClick={() => handleReferenceClick(n)}
                >
                  <td>
                  <span className={`notification-level-tag notification-tag-${n.level}`}>
                    {LEVEL_LABELS[n.level] ?? n.level}
                  </span>
                    {!n.is_read ? <span className="notification-unread-dot" title="未读" /> : null}
                  </td>
                  <td className="notification-title">{n.title}</td>
                  <td className="notification-message">{n.message || "-"}</td>
                  <td className="notification-time">{n.created_at}</td>
                  <td className="notification-actions" onClick={(e) => e.stopPropagation()}>
                  {n.reference_type === "invoice" && n.reference_id ? (
                    <button
                      className="btn-small"
                      onClick={() => handleReferenceClick(n)}
                    >
                      查看发票 #{n.reference_id}
                    </button>
                  ) : null}
                  {!n.is_read ? (
                    <button
                      className="btn-small"
                      onClick={() => handleMarkRead(n.id)}
                    >
                      标记已读
                    </button>
                  ) : null}
                  <button
                    className="btn-small btn-danger"
                    onClick={() => handleDismiss(n.id)}
                  >
                    忽略
                  </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {totalPages > 1 ? (
            <div className="pagination table-pagination">
              <button
                className="page-btn"
                disabled={currentPage <= 1}
                onClick={() => setPage((p) => Math.max(1, p - 1))}
              >
                上一页
              </button>
              <span className="page-info">
                {currentPage} / {totalPages}（共 {notifications.length} 条）
              </span>
              <button
                className="page-btn"
                disabled={currentPage >= totalPages}
                onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
              >
                下一页
              </button>
            </div>
          ) : null}
        </div>
      )}

      <ConfirmDialog
        open={clearDialogOpen}
        title="清空全部通知"
        message="确定要删除所有通知吗？此操作不可撤销。"
        confirmLabel="清空"
        danger
        loading={clearing}
        onConfirm={handleClearAll}
        onCancel={() => setClearDialogOpen(false)}
      />
    </div>
  );
}
