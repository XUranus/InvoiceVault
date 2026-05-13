import React from "react";
import type { InvoiceDetail } from "../types";
import { getInvoiceDetail } from "../api";

type Props = {
  currentInvoiceId: number;
  candidateInvoiceId: number;
  candidateScore: number;
  onClose: () => void;
  onError: (error: string) => void;
};

const COMPARE_FIELDS: Array<{
  key: string;
  label: string;
  get: (d: InvoiceDetail) => string | null;
}> = [
  { key: "invoice_number", label: "发票号码", get: (d) => d.invoice_number },
  { key: "invoice_code", label: "发票代码", get: (d) => d.invoice_code },
  { key: "total_amount", label: "价税合计", get: (d) => d.total_amount },
  { key: "amount_without_tax", label: "不含税金额", get: (d) => d.amount_without_tax },
  { key: "tax_amount", label: "税额", get: (d) => d.tax_amount },
  { key: "issue_date", label: "开票日期", get: (d) => d.issue_date },
  { key: "seller_name", label: "销售方", get: (d) => d.seller_name },
  { key: "buyer_name", label: "购买方", get: (d) => d.buyer_name },
  { key: "invoice_type", label: "发票类型", get: (d) => d.invoice_type },
  { key: "currency", label: "币种", get: (d) => d.currency },
];

export function DuplicateCompare({
  currentInvoiceId,
  candidateInvoiceId,
  candidateScore,
  onClose,
  onError,
}: Props) {
  const [current, setCurrent] = React.useState<InvoiceDetail | null>(null);
  const [candidate, setCandidate] = React.useState<InvoiceDetail | null>(null);
  const [loading, setLoading] = React.useState(true);

  React.useEffect(() => {
    setLoading(true);
    Promise.all([
      getInvoiceDetail(currentInvoiceId),
      getInvoiceDetail(candidateInvoiceId),
    ])
      .then(([c, o]) => {
        setCurrent(c);
        setCandidate(o);
      })
      .catch((err) => onError(String(err)))
      .finally(() => setLoading(false));
  }, [currentInvoiceId, candidateInvoiceId, onError]);

  if (loading) {
    return <div className="dup-compare-loading">加载对比数据...</div>;
  }

  if (!current || !candidate) {
    return <div className="dup-compare-loading">无法加载发票数据</div>;
  }

  return (
    <div className="dup-compare">
      <div className="dup-compare-header">
        <span className="dup-compare-title">
          发票对比 (匹配分数: {candidateScore})
        </span>
        <button className="dup-compare-close" onClick={onClose}>
          关闭
        </button>
      </div>
      <div className="dup-compare-grid">
        <div className="dup-compare-col-header">当前发票 #{currentInvoiceId}</div>
        <div className="dup-compare-col-header">疑似重复 #{candidateInvoiceId}</div>
        {COMPARE_FIELDS.map(({ key, label, get }) => {
          const v1 = get(current);
          const v2 = get(candidate);
          const match = (v1 ?? "") === (v2 ?? "");
          return (
            <React.Fragment key={key}>
              <div className={`dup-compare-cell ${match ? "" : "dup-compare-diff"}`}>
                <span className="dup-compare-field-label">{label}</span>
                <span className="dup-compare-field-value">{v1 ?? "—"}</span>
                {!match && <span className="dup-compare-diff-mark">不同</span>}
              </div>
              <div className={`dup-compare-cell ${match ? "" : "dup-compare-diff"}`}>
                <span className="dup-compare-field-label">{label}</span>
                <span className="dup-compare-field-value">{v2 ?? "—"}</span>
                {!match && <span className="dup-compare-diff-mark">不同</span>}
              </div>
            </React.Fragment>
          );
        })}
      </div>
    </div>
  );
}
