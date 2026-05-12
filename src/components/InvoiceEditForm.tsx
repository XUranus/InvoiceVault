import React from "react";
import type { InvoiceDetail, UpdateInvoiceRequest, FieldError } from "../types";
import { updateInvoice, updateInvoiceItems } from "../api";
import { LineItemsEditor } from "./LineItemsEditor";

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
  });
  const [errors, setErrors] = React.useState<FieldError[]>([]);
  const [saving, setSaving] = React.useState(false);
  const formRef = React.useRef(form);
  formRef.current = form;

  const setField = (field: keyof UpdateInvoiceRequest, value: string) => {
    setForm((prev) => ({ ...prev, [field]: value || null }));
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

  return (
    <div className="edit-form">
      <TextInput
        label="发票类型"
        value={form.invoice_type ?? ""}
        onChange={(v) => setField("invoice_type", v)}
        error={fieldError("invoice_type")}
      />
      <TextInput
        label="发票代码"
        value={form.invoice_code ?? ""}
        onChange={(v) => setField("invoice_code", v)}
        error={fieldError("invoice_code")}
      />
      <TextInput
        label="发票号码"
        value={form.invoice_number ?? ""}
        onChange={(v) => setField("invoice_number", v)}
        error={fieldError("invoice_number")}
      />
      <TextInput
        label="开票日期 (YYYY-MM-DD)"
        value={form.issue_date ?? ""}
        onChange={(v) => setField("issue_date", v)}
        error={fieldError("issue_date")}
      />
      <TextInput
        label="销售方名称"
        value={form.seller_name ?? ""}
        onChange={(v) => setField("seller_name", v)}
        error={fieldError("seller_name")}
      />
      <TextInput
        label="销售方税号"
        value={form.seller_tax_id ?? ""}
        onChange={(v) => setField("seller_tax_id", v)}
        error={fieldError("seller_tax_id")}
      />
      <TextInput
        label="购买方名称"
        value={form.buyer_name ?? ""}
        onChange={(v) => setField("buyer_name", v)}
        error={fieldError("buyer_name")}
      />
      <TextInput
        label="购买方税号"
        value={form.buyer_tax_id ?? ""}
        onChange={(v) => setField("buyer_tax_id", v)}
        error={fieldError("buyer_tax_id")}
      />
      <TextInput
        label="币种"
        value={form.currency ?? ""}
        onChange={(v) => setField("currency", v)}
        error={fieldError("currency")}
      />
      <TextInput
        label="不含税金额"
        value={form.amount_without_tax ?? ""}
        onChange={(v) => setField("amount_without_tax", v)}
        error={fieldError("amount_without_tax")}
      />
      <TextInput
        label="税额"
        value={form.tax_amount ?? ""}
        onChange={(v) => setField("tax_amount", v)}
        error={fieldError("tax_amount")}
      />
      <TextInput
        label="价税合计"
        value={form.total_amount ?? ""}
        onChange={(v) => setField("total_amount", v)}
        error={fieldError("total_amount")}
      />
      <TextInput
        label="类别"
        value={form.category ?? ""}
        onChange={(v) => setField("category", v)}
        error={fieldError("category")}
      />
      <TextInput
        label="备注"
        value={form.remarks ?? ""}
        onChange={(v) => setField("remarks", v)}
        error={fieldError("remarks")}
      />
    </div>
  );
}

function TextInput({
  label,
  value,
  onChange,
  error,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  error?: string;
}) {
  return (
    <label className="edit-field">
      <span>{label}</span>
      <input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className={error ? "input-error" : ""}
      />
      {error ? <span className="field-error">{error}</span> : null}
    </label>
  );
}
