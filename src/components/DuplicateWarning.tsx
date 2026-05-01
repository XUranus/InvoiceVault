import React from "react";
import type { DedupeCheckResult } from "../types";
import { checkInvoiceDuplicates, resolveDuplicate } from "../api";

type Props = {
  invoiceId: number;
  onError: (error: string) => void;
};

export function DuplicateWarning({ invoiceId, onError }: Props) {
  const [result, setResult] = React.useState<DedupeCheckResult | null>(null);

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

  const handleResolve = async (dedupeId: number, action: string) => {
    try {
      await resolveDuplicate(dedupeId, action);
      // refresh
      const updated = await checkInvoiceDuplicates(invoiceId);
      setResult(updated);
    } catch (err) {
      onError(String(err));
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
          <div>
            <strong>
              发票号码: {c.invoice_number ?? "未知"} | 销售方:{" "}
              {c.seller_name ?? "未知"}
            </strong>
            <span>
              日期: {c.issue_date ?? "未知"} | 金额:{" "}
              {c.total_amount ?? "未知"} | 匹配分数: {c.score}
            </span>
          </div>
          <div className="dup-actions">
            <button
              className="small-button"
              onClick={() => handleResolve(c.id, "confirm")}
            >
              确认重复
            </button>
            <button
              className="small-button"
              onClick={() => handleResolve(c.id, "ignore")}
            >
              忽略
            </button>
          </div>
        </div>
      ))}
    </div>
  );
}
