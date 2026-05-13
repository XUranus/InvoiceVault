import React from "react";
import type { DedupeCheckResult } from "../types";
import { checkInvoiceDuplicates, resolveDuplicate } from "../api";
import { DuplicateCompare } from "./DuplicateCompare";

type Props = {
  invoiceId: number;
  onError: (error: string) => void;
  onDeleted: (deletedId: number) => void;
};

export function DuplicateWarning({ invoiceId, onError, onDeleted }: Props) {
  const [result, setResult] = React.useState<DedupeCheckResult | null>(null);
  const [confirmingId, setConfirmingId] = React.useState<number | null>(null);
  const [comparingId, setComparingId] = React.useState<number | null>(null);
  const [resolving, setResolving] = React.useState(false);

  React.useEffect(() => {
    checkInvoiceDuplicates(invoiceId)
      .then(setResult)
      .catch(() => {
        /* dedupe not yet available */
      });
  }, [invoiceId]);

  if (!result || result.candidates.length === 0) return null;

  const openCandidates = result.candidates.filter(
    (c) => c.status === "open",
  );

  if (openCandidates.length === 0) return null;

  const refresh = async () => {
    const updated = await checkInvoiceDuplicates(invoiceId);
    setResult(updated);
    setConfirmingId(null);
  };

  const handleIgnore = async (dedupeId: number) => {
    setResolving(true);
    try {
      await resolveDuplicate(dedupeId, "ignore");
      await refresh();
    } catch (err) {
      onError(String(err));
    } finally {
      setResolving(false);
    }
  };

  const handleAction = async (dedupeId: number, action: string) => {
    setResolving(true);
    try {
      const res = await resolveDuplicate(dedupeId, action);
      if (res.deleted_invoice_id != null) {
        onDeleted(res.deleted_invoice_id);
        return;
      }
      await refresh();
    } catch (err) {
      onError(String(err));
    } finally {
      setResolving(false);
    }
  };

  return (
    <div className="duplicate-warning">
      <h3>
        {result.has_exact_duplicate
          ? "发现完全重复发票"
          : "发现疑似重复发票"}
      </h3>
      {openCandidates.map((c) => (
        <div className="dup-candidate" key={c.id}>
          <div className="dup-candidate-info">
            <strong>
              发票号码: {c.invoice_number ?? "未知"} | 销售方:{" "}
              {c.seller_name ?? "未知"}
            </strong>
            <span>
              日期: {c.issue_date ?? "未知"} | 金额:{" "}
              {c.total_amount ?? "未知"} | 匹配分数: {c.score}
            </span>
            <button
              className="btn-ghost btn-small"
              style={{ marginTop: 6 }}
              onClick={() => setComparingId(comparingId === c.id ? null : c.id)}
            >
              {comparingId === c.id ? "收起对比" : "查看对比"}
            </button>
          </div>
          {comparingId === c.id && (
            <DuplicateCompare
              currentInvoiceId={invoiceId}
              candidateInvoiceId={c.candidate_invoice_id}
              candidateScore={c.score}
              onClose={() => setComparingId(null)}
              onError={onError}
            />
          )}
          {confirmingId === c.id ? (
            <div className="dup-actions-expanded">
              <span className="dup-actions-label">选择处理方式：</span>
              <div className="dup-actions-grid">
                <button
                  className="btn-small dup-action-btn"
                  disabled={resolving}
                  onClick={() => handleAction(c.id, "keep_current")}
                >
                  保留当前，删除重复
                </button>
                <button
                  className="btn-small dup-action-btn"
                  disabled={resolving}
                  onClick={() => handleAction(c.id, "keep_other")}
                >
                  保留重复，删除当前
                </button>
                <button
                  className="btn-small dup-action-btn"
                  disabled={resolving}
                  onClick={() => handleAction(c.id, "keep_both")}
                >
                  两份都保留
                </button>
              </div>
              <button
                className="btn-ghost dup-actions-cancel"
                disabled={resolving}
                onClick={() => setConfirmingId(null)}
              >
                取消
              </button>
            </div>
          ) : (
            <div className="dup-actions">
              <button
                className="btn-small"
                disabled={resolving}
                onClick={() => setConfirmingId(c.id)}
              >
                确认重复
              </button>
              <button
                className="btn-small"
                disabled={resolving}
                onClick={() => handleIgnore(c.id)}
              >
                忽略
              </button>
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
