import React from "react";
import type { InvoiceDetail, UpdateInvoiceRequest, FieldError } from "../types";
import { updateInvoice, updateInvoiceItems } from "../api";
import { LineItemsEditor } from "./LineItemsEditor";

const CATEGORY_OPTIONS = [
  "办公用品", "差旅费", "餐饮", "交通", "通讯", "房租", "水电", "物流", "广告",
  "转账", "报销", "其他",
];

type Props = {
  detail: InvoiceDetail;
  onSaved: () => void;
  onError: (error: string) => void;
  onStateChange?: (state: { save: () => void; saving: boolean }) => void;
};

export function InvoiceEditForm({ detail, onSaved, onError, onStateChange }: Props) {
  const [form, setForm] = React.useState<UpdateInvoiceRequest>({
    id: detail.id,
    invoice_type: detail.invoice_type,
    invoice_code: detail.invoice_code,
    invoice_number: detail.invoice_number,
    issue_date: detail.issue_date,
    seller_name: detail.seller_name,
    seller_tax_id: detail.seller_tax_id,
    buyer_name: detail.buyer_name,
    buyer_tax_id: detail.buyer_tax_id,
    currency: detail.currency,
    amount_without_tax: detail.amount_without_tax,
    tax_amount: detail.tax_amount,
    total_amount: detail.total_amount,
    category: detail.category,
    remarks: detail.remarks,
    confidence: detail.confidence,
    status: detail.status,
    extra_fields: parseExtraFields(detail.extra_fields),
  });
  const [errors, setErrors] = React.useState<FieldError[]>([]);
  const [saving, setSaving] = React.useState(false);
  const [extraExpanded, setExtraExpanded] = React.useState(false);
  const formRef = React.useRef(form);
  formRef.current = form;

  const setField = (field: keyof UpdateInvoiceRequest, value: string) => {
    setForm((prev) => ({ ...prev, [field]: value || null }));
  };

  const setExtraField = (key: string, value: string) => {
    setForm((prev) => {
      const current = { ...(prev.extra_fields as Record<string, unknown> ?? {}) };
      if (value === "") {
        delete current[key];
      } else {
        current[key] = value;
      }
      return { ...prev, extra_fields: Object.keys(current).length > 0 ? current : null };
    });
  };

  const addExtraField = () => {
    const key = prompt("输入字段名称：");
    if (!key) return;
    setForm((prev) => ({
      ...prev,
      extra_fields: { ...(prev.extra_fields as Record<string, unknown> ?? {}), [key]: "" },
    }));
  };

  const handleSave = React.useCallback(async () => {
    setSaving(true);
    setErrors([]);
    try {
      const result = await updateInvoice(formRef.current);
      if (result.errors.length > 0) {
        setErrors(result.errors);
      } else {
        onSaved();
      }
    } catch (err) {
      onError(String(err));
    } finally {
      setSaving(false);
    }
  }, [onSaved, onError]);

  React.useEffect(() => {
    onStateChange?.({ save: handleSave, saving });
  }, [saving, handleSave, onStateChange]);

  const fieldError = (field: string) =>
    errors.find((e) => e.field === field)?.message;

  const extraFields = (form.extra_fields as Record<string, unknown>) ?? {};

  return (
    <div className="edit-form">
      <TextInput label="发票类型" value={form.invoice_type ?? ""} onChange={(v) => setField("invoice_type", v)} error={fieldError("invoice_type")} />
      <TextInput label="开票日期" value={form.issue_date ?? ""} onChange={(v) => setField("issue_date", v)} error={fieldError("issue_date")} placeholder="YYYY-MM-DD" />
      <TextInput label="发票号码" value={form.invoice_number ?? ""} onChange={(v) => setField("invoice_number", v)} error={fieldError("invoice_number")} />
      <TextInput label="发票代码" value={form.invoice_code ?? ""} onChange={(v) => setField("invoice_code", v)} error={fieldError("invoice_code")} />
      <TextInput label="销售方" value={form.seller_name ?? ""} onChange={(v) => setField("seller_name", v)} error={fieldError("seller_name")} />
      <TextInput label="销售方税号" value={form.seller_tax_id ?? ""} onChange={(v) => setField("seller_tax_id", v)} error={fieldError("seller_tax_id")} />
      <TextInput label="购买方" value={form.buyer_name ?? ""} onChange={(v) => setField("buyer_name", v)} error={fieldError("buyer_name")} />
      <TextInput label="购买方税号" value={form.buyer_tax_id ?? ""} onChange={(v) => setField("buyer_tax_id", v)} error={fieldError("buyer_tax_id")} />
      <TextInput label="不含税金额" value={form.amount_without_tax ?? ""} onChange={(v) => setField("amount_without_tax", v)} error={fieldError("amount_without_tax")} />
      <TextInput label="税额" value={form.tax_amount ?? ""} onChange={(v) => setField("tax_amount", v)} error={fieldError("tax_amount")} />
      <TextInput label="价税合计" value={form.total_amount ?? ""} onChange={(v) => setField("total_amount", v)} error={fieldError("total_amount")} />
      <TextInput label="币种" value={form.currency ?? ""} onChange={(v) => setField("currency", v)} error={fieldError("currency")} />
      <label className="edit-field">
        <span>类别</span>
        <select
          value={form.category ?? ""}
          onChange={(e) => setField("category", e.target.value)}
        >
          <option value="">未分类</option>
          {CATEGORY_OPTIONS.map((c) => (
            <option key={c} value={c}>{c}</option>
          ))}
        </select>
      </label>
      <TextInput label="备注" value={form.remarks ?? ""} onChange={(v) => setField("remarks", v)} error={fieldError("remarks")} fullWidth />

      <div className="edit-field field-full" style={{ flexDirection: "column", alignItems: "stretch" }}>
        <button
          type="button"
          className="extra-fields-toggle"
          onClick={() => setExtraExpanded(!extraExpanded)}
          style={{ background: "none", border: "none", cursor: "pointer", textAlign: "left", padding: "4px 0", fontWeight: 600, fontSize: "0.85rem", color: "var(--text-secondary)" }}
        >
          {extraExpanded ? "▼" : "▶"} 扩展字段 ({Object.keys(extraFields).length})
        </button>
        {extraExpanded && (
          <div style={{ display: "flex", flexDirection: "column", gap: "6px", paddingTop: "4px" }}>
            {Object.entries(extraFields).map(([key, val]) => (
              <div key={key} style={{ display: "flex", gap: "8px", alignItems: "center" }}>
                <span style={{ minWidth: "100px", fontSize: "0.8rem", color: "var(--text-secondary)" }}>{formatExtraLabel(key)}</span>
                <input
                  value={String(val ?? "")}
                  onChange={(e) => setExtraField(key, e.target.value)}
                  style={{ flex: 1 }}
                />
                <button type="button" onClick={() => setExtraField(key, "")} style={{ background: "none", border: "none", cursor: "pointer", color: "var(--danger)", fontSize: "1rem" }}>×</button>
              </div>
            ))}
            <button type="button" onClick={addExtraField} style={{ background: "none", border: "1px dashed var(--border)", borderRadius: "4px", padding: "4px 8px", cursor: "pointer", fontSize: "0.8rem", color: "var(--text-secondary)", alignSelf: "flex-start" }}>
              + 添加字段
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

function TextInput({
  label,
  value,
  onChange,
  error,
  placeholder,
  fullWidth,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  error?: string;
  placeholder?: string;
  fullWidth?: boolean;
}) {
  return (
    <label className={`edit-field${fullWidth ? " field-full" : ""}`}>
      <span>{label}</span>
      <div style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column", alignItems: "flex-end" }}>
        <input
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className={error ? "input-error" : ""}
          placeholder={placeholder}
        />
        {error ? <span className="field-error">{error}</span> : null}
      </div>
    </label>
  );
}

function parseExtraFields(value: string | null): Record<string, unknown> | null {
  if (!value) return null;
  try {
    const parsed = JSON.parse(value);
    return typeof parsed === "object" && parsed !== null ? parsed : null;
  } catch {
    return null;
  }
}

const EXTRA_LABEL_MAP: Record<string, string> = {
  passenger_name: "乘客姓名", train_number: "车次", flight_number: "航班号",
  departure: "出发站", arrival: "到达站", departure_time: "出发时间",
  arrival_time: "到达时间", seat_class: "座位等级", boarding_gate: "登机口",
  baggage: "行李额", toll_entry: "入口", toll_exit: "出口",
  license_plate: "车牌号", vehicle_type: "车辆类型", vehicle_model: "车辆型号",
  vin: "VIN码", engine_number: "发动机号", payment_platform: "支付平台",
  transaction_id: "交易单号", counterparty: "收付方", transaction_type: "交易类型",
};

function formatExtraLabel(key: string): string {
  return EXTRA_LABEL_MAP[key] ?? key;
}
