import React from "react";
import type { InvoiceDetail as InvoiceDetailType } from "../types";
import { getInvoiceDetail } from "../api";
import { InvoiceEditForm } from "./InvoiceEditForm";
import { LineItemsEditor } from "./LineItemsEditor";
import { DuplicateWarning } from "./DuplicateWarning";

type Props = {
  invoiceId: number;
  onBack: () => void;
  onError: (error: string) => void;
};

export function InvoiceDetail({ invoiceId, onBack, onError }: Props) {
  const [detail, setDetail] = React.useState<InvoiceDetailType | null>(null);
  const [isEditing, setIsEditing] = React.useState(false);
  const [loading, setLoading] = React.useState(true);

  const loadDetail = React.useCallback(async () => {
    setLoading(true);
    try {
      const result = await getInvoiceDetail(invoiceId);
      setDetail(result);
    } catch (err) {
      onError(String(err));
    } finally {
      setLoading(false);
    }
  }, [invoiceId, onError]);

  React.useEffect(() => {
    loadDetail();
  }, [loadDetail]);

  const handleSaved = () => {
    setIsEditing(false);
    loadDetail();
  };

  if (loading) {
    return (
      <div>
        <button className="back-btn" onClick={onBack}>
          ← 返回列表
        </button>
        <p className="muted">加载中...</p>
      </div>
    );
  }

  if (!detail) {
    return (
      <div>
        <button className="back-btn" onClick={onBack}>
          ← 返回列表
        </button>
        <p className="muted">未找到发票。</p>
      </div>
    );
  }

  return (
    <div className="invoice-detail">
      <div className="detail-header">
        <button className="back-btn" onClick={onBack}>
          ← 返回列表
        </button>
        <button
          className="edit-btn"
          onClick={() => setIsEditing((prev) => !prev)}
        >
          {isEditing ? "取消编辑" : "编辑"}
        </button>
      </div>

      <h2>
        {detail.seller_name ?? detail.invoice_type ?? "发票详情"}
      </h2>

      {detail.thumbnail_path ? (
        <div className="thumbnail-preview">
          <img
            src={`https://asset.localhost/${detail.thumbnail_path}`}
            alt="发票预览"
            className="preview-img"
            onError={(e) => {
              (e.target as HTMLImageElement).style.display = "none";
            }}
          />
        </div>
      ) : null}

      <DuplicateWarning invoiceId={invoiceId} onError={onError} />

      {isEditing ? (
        <InvoiceEditForm
          detail={detail}
          onSaved={handleSaved}
          onError={onError}
        />
      ) : (
        <div className="detail-fields">
          <Field label="发票类型" value={detail.invoice_type} />
          <Field label="发票代码" value={detail.invoice_code} />
          <Field label="发票号码" value={detail.invoice_number} />
          <Field label="开票日期" value={detail.issue_date} />
          <Field label="销售方" value={detail.seller_name} />
          <Field label="销售方税号" value={detail.seller_tax_id} />
          <Field label="购买方" value={detail.buyer_name} />
          <Field label="购买方税号" value={detail.buyer_tax_id} />
          <Field label="币种" value={detail.currency} />
          <Field label="不含税金额" value={detail.amount_without_tax} />
          <Field label="税额" value={detail.tax_amount} />
          <Field label="价税合计" value={detail.total_amount} />
          <Field label="类别" value={detail.category} />
          <Field label="备注" value={detail.remarks} />
          <Field label="页码范围" value={detail.source_page_range} />
          <Field
            label="置信度"
            value={
              detail.confidence != null
                ? `${(detail.confidence * 100).toFixed(0)}%`
                : null
            }
          />
          <Field label="状态" value={detail.status} />
          <Field label="重复状态" value={detail.duplicate_status} />
        </div>
      )}

      {!isEditing && detail.items.length > 0 ? (
        <div className="items-section">
          <h3>明细行</h3>
          <table className="items-table">
            <thead>
              <tr>
                <th>名称</th>
                <th>规格</th>
                <th>单位</th>
                <th>数量</th>
                <th>单价</th>
                <th>金额</th>
                <th>税率</th>
                <th>税额</th>
              </tr>
            </thead>
            <tbody>
              {detail.items.map((item) => (
                <tr key={item.id}>
                  <td>{item.name}</td>
                  <td>{item.specification ?? ""}</td>
                  <td>{item.unit ?? ""}</td>
                  <td>{item.quantity ?? ""}</td>
                  <td>{item.unit_price ?? ""}</td>
                  <td>{item.amount ?? ""}</td>
                  <td>{item.tax_rate ?? ""}</td>
                  <td>{item.tax_amount ?? ""}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
    </div>
  );
}

function Field({ label, value }: { label: string; value: string | null }) {
  if (!value) return null;
  return (
    <div className="detail-field">
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}
