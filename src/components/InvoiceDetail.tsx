import React from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { InvoiceDetail as InvoiceDetailType, InvoiceItemRow, InvoiceItemChange } from "../types";
import { getInvoiceDetail, openInvoiceRawFileInBrowser, updateInvoiceItems } from "../api";
import { InvoiceEditForm } from "./InvoiceEditForm";
import { DuplicateWarning } from "./DuplicateWarning";
import { duplicateStatusMeta, invoiceStatusMeta, toneClass } from "../status";

type Props = {
  invoiceId: number;
  onBack: () => void;
  onError: (error: string) => void;
};

export function InvoiceDetail({ invoiceId, onBack, onError }: Props) {
  const [detail, setDetail] = React.useState<InvoiceDetailType | null>(null);
  const [isEditing, setIsEditing] = React.useState(false);
  const [loading, setLoading] = React.useState(true);
  const [editingItems, setEditingItems] = React.useState<InvoiceItemRow[]>([]);
  const [itemsDirty, setItemsDirty] = React.useState(false);
  const [savingItems, setSavingItems] = React.useState(false);

  const loadDetail = React.useCallback(async () => {
    setLoading(true);
    try {
      const result = await getInvoiceDetail(invoiceId);
      setDetail(result);
      setEditingItems(result.items.map((i) => ({ ...i })));
      setItemsDirty(false);
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

  const handleItemChange = (index: number, field: string, value: string) => {
    setEditingItems((prev) => {
      const next = [...prev];
      next[index] = { ...next[index], [field]: value };
      return next;
    });
    setItemsDirty(true);
  };

  const handleSaveItems = async () => {
    if (!itemsDirty) return;
    setSavingItems(true);
    try {
      const changes: InvoiceItemChange[] = editingItems.map((item) => ({
        action: "update" as const,
        id: item.id,
        name: item.name,
        specification: item.specification,
        unit: item.unit,
        quantity: item.quantity,
        unit_price: item.unit_price,
        amount: item.amount,
        tax_rate: item.tax_rate,
        tax_amount: item.tax_amount,
      }));
      await updateInvoiceItems({ invoice_id: invoiceId, items: changes });
      setItemsDirty(false);
      loadDetail();
    } catch (err) {
      onError(String(err));
    } finally {
      setSavingItems(false);
    }
  };

  const handleOpenRawFile = async () => {
    if (!detail?.raw_file_path) return;
    try {
      await openInvoiceRawFileInBrowser(invoiceId);
    } catch (err) {
      onError(String(err));
    }
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

  const previewSrc = detail.thumbnail_path ? convertFileSrc(detail.thumbnail_path) : null;

  return (
    <div className="invoice-detail">
      <div className="detail-header">
        <button className="back-btn" onClick={onBack}>
          ← 返回列表
        </button>
        <div className="detail-header-actions">
          <button
            className="edit-btn"
            onClick={() => setIsEditing((prev) => !prev)}
          >
            {isEditing ? "取消编辑" : "编辑"}
          </button>
        </div>
      </div>

      <div className="detail-title-row">
        <h2>
          {detail.seller_name ?? detail.invoice_type ?? "发票详情"}
        </h2>
        <div className="detail-badges">
          <span className={`badge ${toneClass(invoiceStatusMeta(detail.status).tone)}`}>
            {statusLabel(detail.status)}
          </span>
          <span className={`badge ${toneClass(duplicateStatusMeta(detail.duplicate_status).tone)}`}>
            {duplicateLabel(detail.duplicate_status)}
          </span>
          {detail.confidence != null && (
            <span className="badge badge-confidence">
              置信度 {(detail.confidence * 100).toFixed(0)}%
            </span>
          )}
          {detail.source_page_range && (
            <span className="badge badge-page">
              {detail.source_page_range}
            </span>
          )}
        </div>
      </div>

      <div className="detail-split">
        <aside className="detail-preview-pane">
          {previewSrc ? (
            <div className="thumbnail-preview">
              <button
                className="preview-open-btn"
                type="button"
                onClick={handleOpenRawFile}
                disabled={!detail.raw_file_path}
                title={detail.raw_file_path ? "打开原文件" : "原文件路径不可用"}
              >
                <img
                  src={previewSrc}
                  alt="发票预览"
                  className="preview-img"
                  onError={(e) => {
                    (e.target as HTMLImageElement).style.display = "none";
                  }}
                />
              </button>
            </div>
          ) : (
            <div className="thumbnail-empty">
              <span>暂无预览</span>
            </div>
          )}
        </aside>

        <div className="detail-content-pane">
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
            </div>
          )}

          {detail.items.length > 0 || editingItems.length > 0 ? (
            <div className="items-section">
              <div className="items-section-header">
                <h3>明细行</h3>
                {itemsDirty && (
                  <button
                    className="btn-primary btn-small"
                    onClick={handleSaveItems}
                    disabled={savingItems}
                  >
                    {savingItems ? "保存中..." : "保存明细"}
                  </button>
                )}
              </div>
              <div className="items-table-wrap">
                <table className="items-table items-table-editable">
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
                    {editingItems.map((item, idx) => (
                      <tr key={item.id}>
                        <td>
                          <input
                            value={item.name}
                            onChange={(e) => handleItemChange(idx, "name", e.target.value)}
                          />
                        </td>
                        <td>
                          <input
                            value={item.specification ?? ""}
                            onChange={(e) => handleItemChange(idx, "specification", e.target.value)}
                          />
                        </td>
                        <td>
                          <input
                            value={item.unit ?? ""}
                            onChange={(e) => handleItemChange(idx, "unit", e.target.value)}
                          />
                        </td>
                        <td>
                          <input
                            value={item.quantity ?? ""}
                            onChange={(e) => handleItemChange(idx, "quantity", e.target.value)}
                          />
                        </td>
                        <td>
                          <input
                            value={item.unit_price ?? ""}
                            onChange={(e) => handleItemChange(idx, "unit_price", e.target.value)}
                          />
                        </td>
                        <td>
                          <input
                            value={item.amount ?? ""}
                            onChange={(e) => handleItemChange(idx, "amount", e.target.value)}
                          />
                        </td>
                        <td>
                          <input
                            value={item.tax_rate ?? ""}
                            onChange={(e) => handleItemChange(idx, "tax_rate", e.target.value)}
                          />
                        </td>
                        <td>
                          <input
                            value={item.tax_amount ?? ""}
                            onChange={(e) => handleItemChange(idx, "tax_amount", e.target.value)}
                          />
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          ) : null}
        </div>
      </div>
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

function statusLabel(status: string): string {
  return invoiceStatusMeta(status).label;
}

function duplicateLabel(status: string): string {
  return duplicateStatusMeta(status).label;
}
