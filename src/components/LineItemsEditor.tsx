import React from "react";
import type { InvoiceItemRow, InvoiceItemChange } from "../types";
import { updateInvoiceItems } from "../api";

type Props = {
  invoiceId: number;
  items: InvoiceItemRow[];
  onSaved: () => void;
  onError: (error: string) => void;
};

export function LineItemsEditor({ invoiceId, items, onSaved, onError }: Props) {
  const [localItems, setLocalItems] = React.useState<InvoiceItemRow[]>(items);
  const [saving, setSaving] = React.useState(false);
  const [changes, setChanges] = React.useState<InvoiceItemChange[]>([]);

  const handleAdd = () => {
    const newItem: InvoiceItemRow = {
      id: -Date.now(),
      name: "",
      specification: null,
      unit: null,
      quantity: null,
      unit_price: null,
      amount: null,
      tax_rate: null,
      tax_amount: null,
    };
    setLocalItems((prev) => [...prev, newItem]);
    setChanges((prev) => [
      ...prev,
      {
        action: "add",
        name: "",
        specification: null,
        unit: null,
        quantity: null,
        unit_price: null,
        amount: null,
        tax_rate: null,
        tax_amount: null,
      },
    ]);
  };

  const handleUpdate = (index: number, field: string, value: string) => {
    const updated = [...localItems];
    updated[index] = { ...updated[index], [field]: value || null };
    setLocalItems(updated);

    const item = updated[index];
    const change: InvoiceItemChange = {
      action: item.id < 0 ? "add" : "update",
      id: item.id > 0 ? item.id : undefined,
      name: item.name,
      specification: item.specification,
      unit: item.unit,
      quantity: item.quantity,
      unit_price: item.unit_price,
      amount: item.amount,
      tax_rate: item.tax_rate,
      tax_amount: item.tax_amount,
    };
    setChanges((prev) => {
      const existing = prev.findIndex(
        (c) => c.id === item.id || (item.id < 0 && index === prev.length - 1),
      );
      if (existing >= 0) {
        const updated = [...prev];
        updated[existing] = change;
        return updated;
      }
      return [...prev, change];
    });
  };

  const handleDelete = (index: number) => {
    const item = localItems[index];
    if (item.id > 0) {
      setChanges((prev) => [
        ...prev,
        { action: "delete", id: item.id, name: item.name },
      ]);
    }
    setLocalItems((prev) => prev.filter((_, i) => i !== index));
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      await updateInvoiceItems({ invoice_id: invoiceId, items: changes });
      onSaved();
    } catch (err) {
      onError(String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="items-editor">
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
            <th></th>
          </tr>
        </thead>
        <tbody>
          {localItems.map((item, i) => (
            <tr key={item.id}>
              <td>
                <input
                  value={item.name}
                  onChange={(e) => handleUpdate(i, "name", e.target.value)}
                />
              </td>
              <td>
                <input
                  value={item.specification ?? ""}
                  onChange={(e) =>
                    handleUpdate(i, "specification", e.target.value)
                  }
                />
              </td>
              <td>
                <input
                  value={item.unit ?? ""}
                  onChange={(e) => handleUpdate(i, "unit", e.target.value)}
                />
              </td>
              <td>
                <input
                  value={item.quantity ?? ""}
                  onChange={(e) => handleUpdate(i, "quantity", e.target.value)}
                />
              </td>
              <td>
                <input
                  value={item.unit_price ?? ""}
                  onChange={(e) =>
                    handleUpdate(i, "unit_price", e.target.value)
                  }
                />
              </td>
              <td>
                <input
                  value={item.amount ?? ""}
                  onChange={(e) => handleUpdate(i, "amount", e.target.value)}
                />
              </td>
              <td>
                <input
                  value={item.tax_rate ?? ""}
                  onChange={(e) => handleUpdate(i, "tax_rate", e.target.value)}
                />
              </td>
              <td>
                <input
                  value={item.tax_amount ?? ""}
                  onChange={(e) =>
                    handleUpdate(i, "tax_amount", e.target.value)
                  }
                />
              </td>
              <td>
                <button
                  className="small-button"
                  onClick={() => handleDelete(i)}
                >
                  删除
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="items-actions">
        <button onClick={handleAdd}>添加明细行</button>
        <button onClick={handleSave} disabled={saving}>
          {saving ? "保存明细..." : "保存明细"}
        </button>
      </div>
    </div>
  );
}
