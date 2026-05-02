import React from "react";

type Props = {
  open: boolean;
  title: string;
  message: string;
  detail?: string;
  confirmLabel?: string;
  danger?: boolean;
  loading?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
};

export function ConfirmDialog({
  open,
  title,
  message,
  detail,
  confirmLabel = "确认",
  danger = false,
  loading = false,
  onConfirm,
  onCancel,
}: Props) {
  if (!open) return null;

  return (
    <div className="modal-overlay" onClick={loading ? undefined : onCancel}>
      <div className="modal-card" onClick={(e) => e.stopPropagation()}>
        <h3 className="modal-title">{title}</h3>
        <p className="modal-message">{message}</p>
        {detail ? (
          <p className="modal-detail">{detail}</p>
        ) : null}
        <div className="modal-actions">
          <button
            className={danger ? "btn-danger" : "btn-primary"}
            onClick={onConfirm}
            disabled={loading}
          >
            {loading ? "处理中..." : confirmLabel}
          </button>
          <button
            className="btn-small"
            onClick={onCancel}
            disabled={loading}
          >
            取消
          </button>
        </div>
      </div>
    </div>
  );
}
