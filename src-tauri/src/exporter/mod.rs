use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::io::Write;

#[derive(Debug, Deserialize)]
pub struct ExportInvoicesRequest {
    pub format: String,
    pub output_path: String,
    pub invoice_ids: Option<Vec<i64>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportResult {
    pub file_path: String,
    pub row_count: usize,
    pub format: String,
    pub byte_size: u64,
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

pub fn export_invoices(
    conn: &Connection,
    request: ExportInvoicesRequest,
) -> Result<ExportResult, ExportError> {
    let rows = load_invoices_for_export(conn, request.invoice_ids.as_deref())?;

    match request.format.as_str() {
        "csv" => export_csv(&request.output_path, &rows),
        "xlsx" => export_xlsx(&request.output_path, &rows),
        other => Err(ExportError::UnsupportedFormat(other.into())),
    }
}

fn load_invoices_for_export(
    conn: &Connection,
    invoice_ids: Option<&[i64]>,
) -> Result<Vec<InvoiceRow>, ExportError> {
    let sql = if let Some(ids) = invoice_ids {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders: Vec<String> = ids.iter().map(|_| "?".into()).collect();
        format!(
            "SELECT id, invoice_type, invoice_code, invoice_number, issue_date,
                seller_name, seller_tax_id, buyer_name, buyer_tax_id, currency,
                amount_without_tax, tax_amount, total_amount, category, remarks,
                source_page_range, confidence, status, duplicate_status, created_at
            FROM invoices
            WHERE id IN ({})
            ORDER BY id DESC",
            placeholders.join(",")
        )
    } else {
        String::from(
            "SELECT id, invoice_type, invoice_code, invoice_number, issue_date,
                seller_name, seller_tax_id, buyer_name, buyer_tax_id, currency,
                amount_without_tax, tax_amount, total_amount, category, remarks,
                source_page_range, confidence, status, duplicate_status, created_at
            FROM invoices
            ORDER BY id DESC",
        )
    };

    let mut stmt = conn.prepare(&sql)?;

    let rows = if let Some(ids) = invoice_ids {
        let param_refs: Vec<Box<dyn rusqlite::types::ToSql>> =
            ids.iter().map(|id| Box::new(*id) as Box<dyn rusqlite::types::ToSql>).collect();
        let refs: Vec<&dyn rusqlite::types::ToSql> = param_refs.iter().map(|v| v.as_ref()).collect();
        stmt.query_map(refs.as_slice(), map_invoice_row)?
            .collect::<Result<Vec<_>, _>>()?
    } else {
        stmt.query_map([], map_invoice_row)?
            .collect::<Result<Vec<_>, _>>()?
    };

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
        currency: row.get::<_, Option<String>>(9)?.unwrap_or_else(|| "CNY".into()),
        amount_without_tax: row.get(10)?,
        tax_amount: row.get(11)?,
        total_amount: row.get(12)?,
        category: row.get(13)?,
        remarks: row.get(14)?,
        source_page_range: row.get(15)?,
        confidence: row.get(16)?,
        status: row.get::<_, Option<String>>(17)?.unwrap_or_else(|| "unknown".into()),
        duplicate_status: row.get::<_, Option<String>>(18)?.unwrap_or_else(|| "unknown".into()),
        created_at: row.get(19)?,
    })
}

fn export_csv(path: &str, rows: &[InvoiceRow]) -> Result<ExportResult, ExportError> {
    let mut file = std::fs::File::create(path)?;

    // UTF-8 BOM for Excel compatibility
    file.write_all(b"\xEF\xBB\xBF")?;

    let mut wtr = csv_writer(&mut file);

    // Header
    wtr.write_record(&[
        "ID", "发票类型", "发票代码", "发票号码", "开票日期",
        "销售方", "销售方税号", "购买方", "购买方税号", "币种",
        "不含税金额", "税额", "价税合计", "类别", "备注",
        "页码范围", "置信度", "状态", "重复状态", "创建时间",
    ])?;

    for row in rows {
        wtr.write_record(&[
            row.id.to_string(),
            row.invoice_type.clone().unwrap_or_default(),
            row.invoice_code.clone().unwrap_or_default(),
            row.invoice_number.clone().unwrap_or_default(),
            row.issue_date.clone().unwrap_or_default(),
            row.seller_name.clone().unwrap_or_default(),
            row.seller_tax_id.clone().unwrap_or_default(),
            row.buyer_name.clone().unwrap_or_default(),
            row.buyer_tax_id.clone().unwrap_or_default(),
            row.currency.clone(),
            row.amount_without_tax.clone().unwrap_or_default(),
            row.tax_amount.clone().unwrap_or_default(),
            row.total_amount.clone().unwrap_or_default(),
            row.category.clone().unwrap_or_default(),
            row.remarks.clone().unwrap_or_default(),
            row.source_page_range.clone().unwrap_or_default(),
            row.confidence.map(|c| format!("{:.2}", c)).unwrap_or_default(),
            row.status.clone(),
            row.duplicate_status.clone(),
            row.created_at.clone(),
        ])?;
    }

    wtr.flush()?;
    drop(wtr);

    let byte_size = file.metadata()?.len();
    Ok(ExportResult {
        file_path: path.into(),
        row_count: rows.len(),
        format: "csv".into(),
        byte_size,
    })
}

// Write CSV with proper quoting — manual to avoid adding csv crate
fn csv_writer<W: Write>(w: W) -> csv::Writer<W> {
    csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(w)
}

fn export_xlsx(path: &str, rows: &[InvoiceRow]) -> Result<ExportResult, ExportError> {
    use rust_xlsxwriter::*;

    let mut workbook = Workbook::new();

    // Invoices sheet
    let sheet = workbook.add_worksheet();
    sheet.set_name("Invoices").map_err(|e| ExportError::Xlsx(e.to_string()))?;

    let headers = [
        "ID", "发票类型", "发票代码", "发票号码", "开票日期",
        "销售方", "销售方税号", "购买方", "购买方税号", "币种",
        "不含税金额", "税额", "价税合计", "类别", "备注",
        "页码范围", "置信度", "状态", "重复状态", "创建时间",
    ];

    let header_format = Format::new().set_bold();
    for (col, header) in headers.iter().enumerate() {
        sheet
            .write_string_with_format(0, col as u16, *header, &header_format)
            .map_err(|e| ExportError::Xlsx(e.to_string()))?;
    }

    for (row_idx, row) in rows.iter().enumerate() {
        let r = (row_idx + 1) as u32;
        let mut write = |col: u16, val: &str| -> Result<(), ExportError> {
            sheet
                .write_string(r, col, val)
                .map_err(|e| ExportError::Xlsx(e.to_string()))?;
            Ok(())
        };
        write(0, &row.id.to_string())?;
        write(1, row.invoice_type.as_deref().unwrap_or(""))?;
        write(2, row.invoice_code.as_deref().unwrap_or(""))?;
        write(3, row.invoice_number.as_deref().unwrap_or(""))?;
        write(4, row.issue_date.as_deref().unwrap_or(""))?;
        write(5, row.seller_name.as_deref().unwrap_or(""))?;
        write(6, row.seller_tax_id.as_deref().unwrap_or(""))?;
        write(7, row.buyer_name.as_deref().unwrap_or(""))?;
        write(8, row.buyer_tax_id.as_deref().unwrap_or(""))?;
        write(9, &row.currency)?;
        write(10, row.amount_without_tax.as_deref().unwrap_or(""))?;
        write(11, row.tax_amount.as_deref().unwrap_or(""))?;
        write(12, row.total_amount.as_deref().unwrap_or(""))?;
        write(13, row.category.as_deref().unwrap_or(""))?;
        write(14, row.remarks.as_deref().unwrap_or(""))?;
        write(15, row.source_page_range.as_deref().unwrap_or(""))?;
        write(16, &row.confidence.map(|c| format!("{:.2}", c)).unwrap_or_default())?;
        write(17, &row.status)?;
        write(18, &row.duplicate_status)?;
        write(19, &row.created_at)?;
    }

    // Auto-fit columns
    for col in 0..headers.len() as u16 {
        sheet
            .set_column_width(col, 14)
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
    })
}
