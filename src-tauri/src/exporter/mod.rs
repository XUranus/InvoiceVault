use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::io::Write;

#[derive(Debug, Deserialize)]
pub struct ExportInvoicesRequest {
    pub format: String,
    pub output_path: String,
    pub invoice_ids: Option<Vec<i64>>,
    pub columns: Option<Vec<String>>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportResult {
    pub file_path: String,
    pub row_count: usize,
    pub format: String,
    pub byte_size: u64,
    pub columns: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("csv error: {0}")]
    Csv(#[from] csv::Error),
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
    #[error("xlsx error: {0}")]
    Xlsx(String),
}

#[derive(Debug, Clone)]
struct ColumnDef {
    key: &'static str,
    label: &'static str,
    numeric: bool,
}

const ALL_COLUMNS: &[ColumnDef] = &[
    ColumnDef { key: "invoice_type", label: "发票类型", numeric: false },
    ColumnDef { key: "invoice_code", label: "发票代码", numeric: false },
    ColumnDef { key: "invoice_number", label: "发票号码", numeric: false },
    ColumnDef { key: "issue_date", label: "开票日期", numeric: false },
    ColumnDef { key: "seller_name", label: "销售方", numeric: false },
    ColumnDef { key: "seller_tax_id", label: "销售方税号", numeric: false },
    ColumnDef { key: "buyer_name", label: "购买方", numeric: false },
    ColumnDef { key: "buyer_tax_id", label: "购买方税号", numeric: false },
    ColumnDef { key: "currency", label: "币种", numeric: false },
    ColumnDef { key: "amount_without_tax", label: "不含税金额", numeric: true },
    ColumnDef { key: "tax_amount", label: "税额", numeric: true },
    ColumnDef { key: "total_amount", label: "价税合计", numeric: true },
    ColumnDef { key: "category", label: "类别", numeric: false },
    ColumnDef { key: "remarks", label: "备注", numeric: false },
    ColumnDef { key: "source_page_range", label: "页码范围", numeric: false },
    ColumnDef { key: "confidence", label: "置信度", numeric: true },
    ColumnDef { key: "status", label: "状态", numeric: false },
    ColumnDef { key: "duplicate_status", label: "重复状态", numeric: false },
    ColumnDef { key: "created_at", label: "创建时间", numeric: false },
];

fn resolve_columns(requested: Option<&[String]>) -> Vec<ColumnDef> {
    if let Some(cols) = requested {
        let mut selected: Vec<ColumnDef> = Vec::new();
        for key in cols {
            if let Some(def) = ALL_COLUMNS.iter().find(|c| c.key == key) {
                selected.push(def.clone());
            }
        }
        if selected.is_empty() {
            ALL_COLUMNS.to_vec()
        } else {
            selected
        }
    } else {
        ALL_COLUMNS.to_vec()
    }
}

struct InvoiceRow {
    id: i64,
    invoice_type: Option<String>,
    invoice_code: Option<String>,
    invoice_number: Option<String>,
    issue_date: Option<String>,
    seller_name: Option<String>,
    seller_tax_id: Option<String>,
    buyer_name: Option<String>,
    buyer_tax_id: Option<String>,
    currency: String,
    amount_without_tax: Option<String>,
    tax_amount: Option<String>,
    total_amount: Option<String>,
    category: Option<String>,
    remarks: Option<String>,
    source_page_range: Option<String>,
    confidence: Option<f64>,
    status: String,
    duplicate_status: String,
    created_at: String,
}

impl InvoiceRow {
    fn field_by_key(&self, key: &str) -> String {
        match key {
            "invoice_type" => self.invoice_type.clone().unwrap_or_default(),
            "invoice_code" => self.invoice_code.clone().unwrap_or_default(),
            "invoice_number" => self.invoice_number.clone().unwrap_or_default(),
            "issue_date" => self.issue_date.clone().unwrap_or_default(),
            "seller_name" => self.seller_name.clone().unwrap_or_default(),
            "seller_tax_id" => self.seller_tax_id.clone().unwrap_or_default(),
            "buyer_name" => self.buyer_name.clone().unwrap_or_default(),
            "buyer_tax_id" => self.buyer_tax_id.clone().unwrap_or_default(),
            "currency" => self.currency.clone(),
            "amount_without_tax" => self.amount_without_tax.clone().unwrap_or_default(),
            "tax_amount" => self.tax_amount.clone().unwrap_or_default(),
            "total_amount" => self.total_amount.clone().unwrap_or_default(),
            "category" => self.category.clone().unwrap_or_default(),
            "remarks" => self.remarks.clone().unwrap_or_default(),
            "source_page_range" => self.source_page_range.clone().unwrap_or_default(),
            "confidence" => self.confidence.map(|c| format!("{:.2}", c)).unwrap_or_default(),
            "status" => self.status.clone(),
            "duplicate_status" => self.duplicate_status.clone(),
            "created_at" => self.created_at.clone(),
            _ => String::new(),
        }
    }

    fn number_by_key(&self, key: &str) -> Option<f64> {
        match key {
            "amount_without_tax" => self.amount_without_tax.as_deref()?.parse().ok(),
            "tax_amount" => self.tax_amount.as_deref()?.parse().ok(),
            "total_amount" => self.total_amount.as_deref()?.parse().ok(),
            "confidence" => self.confidence,
            _ => None,
        }
    }
}

pub fn export_invoices(
    conn: &Connection,
    request: ExportInvoicesRequest,
) -> Result<ExportResult, ExportError> {
    let columns = resolve_columns(request.columns.as_deref());
    let column_labels: Vec<String> = columns.iter().map(|c| c.label.to_string()).collect();
    let rows = load_invoices_for_export(
        conn,
        request.invoice_ids.as_deref(),
        request.date_from.as_deref(),
        request.date_to.as_deref(),
    )?;

    let result = match request.format.as_str() {
        "csv" => export_csv(&request.output_path, &rows, &columns)?,
        "xlsx" => export_xlsx(&request.output_path, &rows, &columns)?,
        other => return Err(ExportError::UnsupportedFormat(other.into())),
    };

    Ok(ExportResult {
        columns: column_labels,
        ..result
    })
}

fn load_invoices_for_export(
    conn: &Connection,
    invoice_ids: Option<&[i64]>,
    date_from: Option<&str>,
    date_to: Option<&str>,
) -> Result<Vec<InvoiceRow>, ExportError> {
    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ids) = invoice_ids {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders: Vec<String> = ids.iter().map(|_| "?".into()).collect();
        conditions.push(format!("id IN ({})", placeholders.join(",")));
        for id in ids {
            params.push(Box::new(*id));
        }
    }

    if let Some(from) = date_from {
        conditions.push("issue_date >= ?".into());
        params.push(Box::new(from.to_string()));
    }
    if let Some(to) = date_to {
        conditions.push("issue_date <= ?".into());
        params.push(Box::new(to.to_string()));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let sql = format!(
        "SELECT id, invoice_type, invoice_code, invoice_number, issue_date,
            seller_name, seller_tax_id, buyer_name, buyer_tax_id, currency,
            amount_without_tax, tax_amount, total_amount, category, remarks,
            source_page_range, confidence, status, duplicate_status, created_at
        FROM invoices
        {}
        ORDER BY id DESC",
        where_clause
    );

    let mut stmt = conn.prepare(&sql)?;

    let refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|v| v.as_ref()).collect();
    let rows = stmt
        .query_map(refs.as_slice(), map_invoice_row)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

fn map_invoice_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<InvoiceRow> {
    Ok(InvoiceRow {
        id: row.get(0)?,
        invoice_type: row.get(1)?,
        invoice_code: row.get(2)?,
        invoice_number: row.get(3)?,
        issue_date: row.get(4)?,
        seller_name: row.get(5)?,
        seller_tax_id: row.get(6)?,
        buyer_name: row.get(7)?,
        buyer_tax_id: row.get(8)?,
        currency: row
            .get::<_, Option<String>>(9)?
            .unwrap_or_else(|| "CNY".into()),
        amount_without_tax: row.get(10)?,
        tax_amount: row.get(11)?,
        total_amount: row.get(12)?,
        category: row.get(13)?,
        remarks: row.get(14)?,
        source_page_range: row.get(15)?,
        confidence: row.get(16)?,
        status: row
            .get::<_, Option<String>>(17)?
            .unwrap_or_else(|| "unknown".into()),
        duplicate_status: row
            .get::<_, Option<String>>(18)?
            .unwrap_or_else(|| "unknown".into()),
        created_at: row.get(19)?,
    })
}

fn export_csv(
    path: &str,
    rows: &[InvoiceRow],
    columns: &[ColumnDef],
) -> Result<ExportResult, ExportError> {
    let mut file = std::fs::File::create(path)?;

    // UTF-8 BOM for Excel compatibility
    file.write_all(b"\xEF\xBB\xBF")?;

    let mut wtr = csv_writer(&mut file);

    // Header
    let headers: Vec<&str> = columns.iter().map(|c| c.label).collect();
    wtr.write_record(&headers)?;

    for row in rows {
        let values: Vec<String> = columns.iter().map(|c| row.field_by_key(c.key)).collect();
        let refs: Vec<&str> = values.iter().map(|s| s.as_str()).collect();
        wtr.write_record(&refs)?;
    }

    wtr.flush()?;
    drop(wtr);

    let byte_size = file.metadata()?.len();
    Ok(ExportResult {
        file_path: path.into(),
        row_count: rows.len(),
        format: "csv".into(),
        byte_size,
        columns: columns.iter().map(|c| c.label.into()).collect(),
    })
}

fn csv_writer<W: Write>(w: W) -> csv::Writer<W> {
    csv::WriterBuilder::new().has_headers(false).from_writer(w)
}

fn export_xlsx(
    path: &str,
    rows: &[InvoiceRow],
    columns: &[ColumnDef],
) -> Result<ExportResult, ExportError> {
    use rust_xlsxwriter::*;

    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet
        .set_name("Invoices")
        .map_err(|e| ExportError::Xlsx(e.to_string()))?;

    let header_format = Format::new().set_bold().set_background_color(Color::RGB(0xE0E0E0));
    let number_format = Format::new().set_num_format("0.00");

    // Write headers
    for (col, col_def) in columns.iter().enumerate() {
        sheet
            .write_string_with_format(0, col as u16, col_def.label, &header_format)
            .map_err(|e| ExportError::Xlsx(e.to_string()))?;
    }

    // Write data rows
    for (row_idx, row) in rows.iter().enumerate() {
        let r = (row_idx + 1) as u32;
        for (col, col_def) in columns.iter().enumerate() {
            let c = col as u16;
            if col_def.numeric {
                if let Some(num) = row.number_by_key(col_def.key) {
                    sheet
                        .write_number_with_format(r, c, num, &number_format)
                        .map_err(|e| ExportError::Xlsx(e.to_string()))?;
                } else {
                    sheet
                        .write_string(r, c, "")
                        .map_err(|e| ExportError::Xlsx(e.to_string()))?;
                }
            } else {
                let val = row.field_by_key(col_def.key);
                sheet
                    .write_string(r, c, &val)
                    .map_err(|e| ExportError::Xlsx(e.to_string()))?;
            }
        }
    }

    // Auto-fit columns based on content
    for (col, col_def) in columns.iter().enumerate() {
        let mut max_width = col_def.label.len() as f64 * 1.2;
        for row in rows {
            let val = row.field_by_key(col_def.key);
            let width = val.chars().fold(0.0, |acc, ch| {
                acc + if ch.is_ascii() { 1.0 } else { 2.0 }
            });
            if width > max_width {
                max_width = width;
            }
        }
        let width = (max_width + 2.0).min(50.0).max(8.0);
        sheet
            .set_column_width(col as u16, width)
            .map_err(|e| ExportError::Xlsx(e.to_string()))?;
    }

    workbook
        .save(path)
        .map_err(|e| ExportError::Xlsx(e.to_string()))?;

    let byte_size = std::fs::metadata(path)?.len();
    Ok(ExportResult {
        file_path: path.into(),
        row_count: rows.len(),
        format: "xlsx".into(),
        byte_size,
        columns: columns.iter().map(|c| c.label.into()).collect(),
    })
}
