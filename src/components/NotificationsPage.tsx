import React from "react";
import type { NotificationRow } from "../types";
import {
  listNotifications,
  markNotificationRead,
  markAllNotificationsRead,
  dismissNotification,
} from "../api";

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

  const handleReferenceClick = (n: NotificationRow) => {
    if (!n.is_read) handleMarkRead(n.id);
    if (n.reference_type === "invoice" && n.reference_id && onNavigateToInvoice) {
      onNavigateToInvoice(n.reference_id);
    }
  };

  const unreadCount = notifications.filter((n) => !n.is_read).length;

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
        </div>
      </div>

      {notifications.length === 0 ? (
        <div className="empty-state">
          <span className="empty-icon">🔔</span>
          <p>暂无通知</p>
        </div>
      ) : (
        <div className="notification-list">
          {notifications.map((n) => (
            <div
              key={n.id}
              className={`notification-card notification-level-${n.level} ${!n.is_read ? "notification-unread" : ""}`}
            >
              <div className="notification-indicator" />
              <div className="notification-body">
                <div className="notification-header">
                  <span className={`notification-level-tag notification-tag-${n.level}`}>
                    {LEVEL_LABELS[n.level] ?? n.level}
                  </span>
                  <span className="notification-time">{n.created_at}</span>
                </div>
                <strong className="notification-title">{n.title}</strong>
                {n.message ? (
                  <p className="notification-message">{n.message}</p>
                ) : null}
                <div className="notification-actions">
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
                </div>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
