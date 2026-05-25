use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

use printpdf::{
    BuiltinFont, Color, Mm, Op, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, PdfWarnMsg,
    Point, Pt, RawImage, Rect, Rgb, TextItem, XObjectTransform,
};

#[derive(Debug, Deserialize)]
pub struct ExportInvoicesRequest {
    pub format: String,
    pub output_path: String,
    pub invoice_ids: Option<Vec<i64>>,
    pub columns: Option<Vec<String>>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExportPreviewRequest {
    pub invoice_ids: Option<Vec<i64>>,
    pub columns: Option<Vec<String>>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportResult {
    pub file_path: String,
    pub row_count: usize,
    pub format: String,
    pub byte_size: u64,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportPreviewResult {
    pub row_count: usize,
    pub columns: Vec<String>,
    pub sample_rows: Vec<Vec<String>>,
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
pub(crate) struct ColumnDef {
    pub(crate) key: &'static str,
    pub(crate) label: &'static str,
    pub(crate) numeric: bool,
    pub(crate) aliases: &'static [&'static str],
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportColumnInfo {
    pub key: String,
    pub label: String,
    pub aliases: Vec<String>,
    pub data_type: String,
    pub exportable: bool,
}

pub(crate) const ALL_COLUMNS: &[ColumnDef] = &[
    ColumnDef {
        key: "invoice_type",
        label: "发票类型",
        numeric: false,
        aliases: &["类型", "票种", "发票种类"],
    },
    ColumnDef {
        key: "invoice_code",
        label: "发票代码",
        numeric: false,
        aliases: &["发票编码", "代码"],
    },
    ColumnDef {
        key: "invoice_number",
        label: "发票号码",
        numeric: false,
        aliases: &["发票号", "号码"],
    },
    ColumnDef {
        key: "issue_date",
        label: "开票日期",
        numeric: false,
        aliases: &["日期", "时间", "发票日期", "开具时间"],
    },
    ColumnDef {
        key: "seller_name",
        label: "销售方",
        numeric: false,
        aliases: &["销方", "供应商", "商家", "公司名称"],
    },
    ColumnDef {
        key: "seller_tax_id",
        label: "销售方税号",
        numeric: false,
        aliases: &["销方税号", "供应商税号"],
    },
    ColumnDef {
        key: "buyer_name",
        label: "购买方",
        numeric: false,
        aliases: &["购方", "买方"],
    },
    ColumnDef {
        key: "buyer_tax_id",
        label: "购买方税号",
        numeric: false,
        aliases: &["购方税号", "买方税号"],
    },
    ColumnDef {
        key: "currency",
        label: "币种",
        numeric: false,
        aliases: &["货币"],
    },
    ColumnDef {
        key: "amount_without_tax",
        label: "不含税金额",
        numeric: true,
        aliases: &["未税金额", "金额不含税", "金额"],
    },
    ColumnDef {
        key: "tax_amount",
        label: "税额",
        numeric: true,
        aliases: &["税金"],
    },
    ColumnDef {
        key: "total_amount",
        label: "价税合计",
        numeric: true,
        aliases: &["总金额", "合计", "含税金额"],
    },
    ColumnDef {
        key: "category",
        label: "类别",
        numeric: false,
        aliases: &["分类", "类型", "费用类别"],
    },
    ColumnDef {
        key: "remarks",
        label: "备注",
        numeric: false,
        aliases: &["说明", "发票内容", "内容", "开票内容"],
    },
    ColumnDef {
        key: "source_page_range",
        label: "页码范围",
        numeric: false,
        aliases: &["页码"],
    },
    ColumnDef {
        key: "confidence",
        label: "置信度",
        numeric: true,
        aliases: &["识别置信度"],
    },
    ColumnDef {
        key: "status",
        label: "状态",
        numeric: false,
        aliases: &["识别状态"],
    },
    ColumnDef {
        key: "duplicate_status",
        label: "重复状态",
        numeric: false,
        aliases: &["是否重复"],
    },
    ColumnDef {
        key: "created_at",
        label: "创建时间",
        numeric: false,
        aliases: &["导入时间", "入库时间"],
    },
];

pub fn export_column_catalog() -> Vec<ExportColumnInfo> {
    ALL_COLUMNS
        .iter()
        .map(|column| ExportColumnInfo {
            key: column.key.to_owned(),
            label: column.label.to_owned(),
            aliases: column
                .aliases
                .iter()
                .map(|alias| alias.to_string())
                .collect(),
            data_type: if column.numeric { "number" } else { "string" }.to_owned(),
            exportable: true,
        })
        .collect()
}

pub fn resolve_export_column_keys_from_labels(labels: &[String]) -> Vec<String> {
    let mut keys = Vec::new();
    for label in labels {
        let normalized = label.trim();
        if normalized.is_empty() {
            continue;
        }
        if let Some(column) = ALL_COLUMNS.iter().find(|column| {
            column.key.eq_ignore_ascii_case(normalized)
                || column.label == normalized
                || column.aliases.iter().any(|alias| *alias == normalized)
        }) {
            if !keys.iter().any(|key| key == column.key) {
                keys.push(column.key.to_owned());
            }
        }
    }
    keys
}

/// Resolve template labels to (0-based column index, &ColumnDef) pairs.
/// Returns only columns that matched a known export field.
pub(crate) fn resolve_template_column_map(labels: &[String]) -> Vec<(usize, &'static ColumnDef)> {
    let mut result = Vec::new();
    let mut seen_keys: Vec<&'static str> = Vec::new();
    for (i, label) in labels.iter().enumerate() {
        let normalized = label.trim();
        if normalized.is_empty() {
            continue;
        }
        if let Some(column) = ALL_COLUMNS.iter().find(|column| {
            column.key.eq_ignore_ascii_case(normalized)
                || column.label == normalized
                || column.aliases.iter().any(|alias| *alias == normalized)
        }) {
            if !seen_keys.contains(&column.key) {
                seen_keys.push(column.key);
                result.push((i, column));
            }
        }
    }
    result
}

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

pub(crate) struct InvoiceRow {
    pub(crate) invoice_type: Option<String>,
    pub(crate) invoice_code: Option<String>,
    pub(crate) invoice_number: Option<String>,
    pub(crate) issue_date: Option<String>,
    pub(crate) seller_name: Option<String>,
    pub(crate) seller_tax_id: Option<String>,
    pub(crate) buyer_name: Option<String>,
    pub(crate) buyer_tax_id: Option<String>,
    pub(crate) currency: String,
    pub(crate) amount_without_tax: Option<String>,
    pub(crate) tax_amount: Option<String>,
    pub(crate) total_amount: Option<String>,
    pub(crate) category: Option<String>,
    pub(crate) remarks: Option<String>,
    pub(crate) source_page_range: Option<String>,
    pub(crate) confidence: Option<f64>,
    pub(crate) status: String,
    pub(crate) duplicate_status: String,
    pub(crate) created_at: String,
}

impl InvoiceRow {
    pub(crate) fn field_by_key(&self, key: &str) -> String {
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
            "confidence" => self
                .confidence
                .map(|c| format!("{:.2}", c))
                .unwrap_or_default(),
            "status" => self.status.clone(),
            "duplicate_status" => self.duplicate_status.clone(),
            "created_at" => self.created_at.clone(),
            _ => String::new(),
        }
    }

    pub(crate) fn number_by_key(&self, key: &str) -> Option<f64> {
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

pub fn preview_export(
    conn: &Connection,
    request: ExportPreviewRequest,
) -> Result<ExportPreviewResult, ExportError> {
    let columns = resolve_columns(request.columns.as_deref());
    let rows = load_invoices_for_export(
        conn,
        request.invoice_ids.as_deref(),
        request.date_from.as_deref(),
        request.date_to.as_deref(),
    )?;
    let limit = request.limit.unwrap_or(5).clamp(1, 20);
    let sample_rows = rows
        .iter()
        .take(limit)
        .map(|row| {
            columns
                .iter()
                .map(|column| row.field_by_key(column.key))
                .collect()
        })
        .collect();

    Ok(ExportPreviewResult {
        row_count: rows.len(),
        columns: columns
            .iter()
            .map(|column| column.label.to_owned())
            .collect(),
        sample_rows,
    })
}

pub(crate) fn load_invoices_for_export(
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
        "SELECT invoice_type, invoice_code, invoice_number, issue_date,
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
        invoice_type: row.get(0)?,
        invoice_code: row.get(1)?,
        invoice_number: row.get(2)?,
        issue_date: row.get(3)?,
        seller_name: row.get(4)?,
        seller_tax_id: row.get(5)?,
        buyer_name: row.get(6)?,
        buyer_tax_id: row.get(7)?,
        currency: row
            .get::<_, Option<String>>(8)?
            .unwrap_or_else(|| "CNY".into()),
        amount_without_tax: row.get(9)?,
        tax_amount: row.get(10)?,
        total_amount: row.get(11)?,
        category: row.get(12)?,
        remarks: row.get(13)?,
        source_page_range: row.get(14)?,
        confidence: row.get(15)?,
        status: row
            .get::<_, Option<String>>(16)?
            .unwrap_or_else(|| "unknown".into()),
        duplicate_status: row
            .get::<_, Option<String>>(17)?
            .unwrap_or_else(|| "unknown".into()),
        created_at: row.get(18)?,
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

    let header_format = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0xE0E0E0));
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
            let width = val
                .chars()
                .fold(0.0, |acc, ch| acc + if ch.is_ascii() { 1.0 } else { 2.0 });
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

// --- PDF Report Export ---

const A4_WIDTH_MM: f32 = 297.0;
const A4_HEIGHT_MM: f32 = 210.0;
const MARGIN_MM: f32 = 15.0;

const TABLE_HEADER_FIELDS: &[(&str, f32)] = &[
    ("Invoice No.", 30.0),
    ("Date", 25.0),
    ("Seller", 40.0),
    ("Buyer", 40.0),
    ("Total", 25.0),
    ("Tax", 22.0),
    ("Type", 25.0),
];

#[derive(Debug, Deserialize)]
pub struct PdfReportRequest {
    pub output_path: String,
    pub invoice_ids: Option<Vec<i64>>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub thumbnails_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PdfReportResult {
    pub file_path: String,
    pub invoice_count: usize,
    pub byte_size: u64,
}

pub fn export_pdf_report(
    conn: &Connection,
    request: PdfReportRequest,
) -> Result<PdfReportResult, ExportError> {
    let rows = load_invoices_for_export(
        conn,
        request.invoice_ids.as_deref(),
        request.date_from.as_deref(),
        request.date_to.as_deref(),
    )?;

    if rows.is_empty() {
        return Err(ExportError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no invoices to export",
        )));
    }

    let thumbnails_dir = request
        .thumbnails_dir
        .as_deref()
        .map(Path::new)
        .filter(|p| p.exists());
    let mut warnings: Vec<PdfWarnMsg> = Vec::new();

    let mut doc = PdfDocument::new("InvoiceVault Report");

    // Summary table page
    let summary_ops = build_summary_ops(&rows);
    let summary_page = PdfPage::new(Mm(A4_WIDTH_MM), Mm(A4_HEIGHT_MM), summary_ops);
    doc.pages.push(summary_page);

    // Detail pages — one per invoice
    for row in &rows {
        let mut ops = build_detail_ops(row);
        if let Some(thumb_path) = find_thumbnail_for_invoice(row, thumbnails_dir) {
            if let Some(xobj_op) = load_thumbnail_xobject(&mut doc, &thumb_path) {
                ops.push(xobj_op);
            }
        }
        let page = PdfPage::new(Mm(A4_WIDTH_MM), Mm(A4_HEIGHT_MM), ops);
        doc.pages.push(page);
    }

    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);

    std::fs::write(&request.output_path, &bytes)?;
    let byte_size = bytes.len() as u64;

    Ok(PdfReportResult {
        file_path: request.output_path,
        invoice_count: rows.len(),
        byte_size,
    })
}

fn ascii_filter(s: &str) -> String {
    s.chars()
        .map(|ch| if ch.is_ascii() { ch } else { '?' })
        .collect()
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}

fn text_ops(text: &str, x: Mm, y: Mm, size: Pt) -> Vec<Op> {
    vec![
        Op::StartTextSection,
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
            size,
        },
        Op::SetTextCursor {
            pos: Point {
                x: x.into_pt(),
                y: y.into_pt(),
            },
        },
        Op::ShowText {
            items: vec![TextItem::Text(text.to_string())],
        },
        Op::EndTextSection,
    ]
}

fn mm_rect(x_mm: f32, y_mm: f32, w_mm: f32, h_mm: f32) -> Rect {
    Rect {
        x: Mm(x_mm).into_pt(),
        y: Mm(y_mm).into_pt(),
        width: Mm(w_mm).into_pt(),
        height: Mm(h_mm).into_pt(),
        mode: None,
        winding_order: None,
    }
}

fn build_summary_ops(rows: &[InvoiceRow]) -> Vec<Op> {
    let mut ops = Vec::new();
    let row_height: f32 = 6.0;
    let header_y: f32 = A4_HEIGHT_MM - MARGIN_MM;

    // Title
    ops.extend(text_ops(
        "Invoice Report",
        Mm(MARGIN_MM),
        Mm(A4_HEIGHT_MM - 10.0),
        Pt(14.0),
    ));

    // Table header background
    ops.push(Op::SetFillColor {
        col: Color::Rgb(Rgb {
            r: 0.88,
            g: 0.88,
            b: 0.88,
            icc_profile: None,
        }),
    });
    ops.push(Op::DrawRectangle {
        rectangle: mm_rect(
            MARGIN_MM,
            header_y - row_height,
            A4_WIDTH_MM - 2.0 * MARGIN_MM,
            row_height,
        ),
    });

    // Table header text
    let mut cx = MARGIN_MM + 1.0;
    for (label, width) in TABLE_HEADER_FIELDS {
        let filtered = truncate_str(&ascii_filter(label), (*width / 2.5) as usize);
        ops.extend(text_ops(&filtered, Mm(cx), Mm(header_y - 4.5), Pt(9.0)));
        cx += width;
    }

    // Data rows
    let mut row_y = header_y - row_height - 2.0;
    for (idx, row) in rows.iter().enumerate() {
        if row_y < MARGIN_MM + 10.0 {
            break;
        }

        // Alternating row background
        if idx % 2 == 1 {
            ops.push(Op::SetFillColor {
                col: Color::Rgb(Rgb {
                    r: 0.95,
                    g: 0.95,
                    b: 0.95,
                    icc_profile: None,
                }),
            });
            ops.push(Op::DrawRectangle {
                rectangle: mm_rect(
                    MARGIN_MM,
                    row_y - row_height,
                    A4_WIDTH_MM - 2.0 * MARGIN_MM,
                    row_height,
                ),
            });
        }

        ops.push(Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                icc_profile: None,
            }),
        });

        let values: Vec<String> = vec![
            truncate_str(&ascii_filter(&row.field_by_key("invoice_number")), 12),
            truncate_str(&ascii_filter(&row.field_by_key("issue_date")), 10),
            truncate_str(&ascii_filter(&row.field_by_key("seller_name")), 16),
            truncate_str(&ascii_filter(&row.field_by_key("buyer_name")), 16),
            truncate_str(&ascii_filter(&row.field_by_key("total_amount")), 10),
            truncate_str(&ascii_filter(&row.field_by_key("tax_amount")), 8),
            truncate_str(&ascii_filter(&row.field_by_key("invoice_type")), 10),
        ];

        let mut cx = MARGIN_MM + 1.0;
        for (i, (_, width)) in TABLE_HEADER_FIELDS.iter().enumerate() {
            ops.extend(text_ops(&values[i], Mm(cx), Mm(row_y - 4.5), Pt(8.0)));
            cx += width;
        }

        row_y -= row_height + 1.0;
    }

    ops
}

fn build_detail_ops(row: &InvoiceRow) -> Vec<Op> {
    let mut ops = Vec::new();

    // Title
    ops.extend(text_ops(
        &format!(
            "Invoice {}",
            ascii_filter(&row.field_by_key("invoice_number"))
        ),
        Mm(MARGIN_MM),
        Mm(A4_HEIGHT_MM - 10.0),
        Pt(12.0),
    ));

    // Field list
    let fields: Vec<(&str, String)> = vec![
        ("Type", row.field_by_key("invoice_type")),
        ("Code", row.field_by_key("invoice_code")),
        ("Number", row.field_by_key("invoice_number")),
        ("Date", row.field_by_key("issue_date")),
        ("Seller", row.field_by_key("seller_name")),
        ("Seller Tax ID", row.field_by_key("seller_tax_id")),
        ("Buyer", row.field_by_key("buyer_name")),
        ("Buyer Tax ID", row.field_by_key("buyer_tax_id")),
        ("Currency", row.field_by_key("currency")),
        ("Amount (excl. tax)", row.field_by_key("amount_without_tax")),
        ("Tax Amount", row.field_by_key("tax_amount")),
        ("Total", row.field_by_key("total_amount")),
        ("Category", row.field_by_key("category")),
        ("Remarks", row.field_by_key("remarks")),
        ("Status", row.field_by_key("status")),
        ("Duplicate", row.field_by_key("duplicate_status")),
        ("Confidence", row.field_by_key("confidence")),
        ("Pages", row.field_by_key("source_page_range")),
    ];

    let mut field_y = A4_HEIGHT_MM - 22.0;
    for (label, value) in &fields {
        if field_y < MARGIN_MM + 5.0 {
            break;
        }
        let display = format!("{}: {}", label, truncate_str(&ascii_filter(value), 60));
        ops.extend(text_ops(&display, Mm(MARGIN_MM), Mm(field_y), Pt(9.0)));
        field_y -= 5.5;
    }

    ops
}

fn load_thumbnail_xobject(doc: &mut PdfDocument, thumb_path: &Path) -> Option<Op> {
    let data = std::fs::read(thumb_path).ok()?;
    let mut warnings: Vec<PdfWarnMsg> = Vec::new();
    let image = RawImage::decode_from_bytes(&data, &mut warnings).ok()?;
    let x_object_name = doc.add_image(&image);
    Some(Op::UseXobject {
        id: x_object_name,
        transform: XObjectTransform {
            translate_x: Some(Mm(A4_WIDTH_MM - MARGIN_MM - 50.0).into_pt()),
            translate_y: Some(Mm(A4_HEIGHT_MM - 25.0).into_pt()),
            scale_x: Some(0.5),
            scale_y: Some(0.5),
            ..Default::default()
        },
    })
}

fn find_thumbnail_for_invoice(row: &InvoiceRow, thumbnails_dir: Option<&Path>) -> Option<PathBuf> {
    let thumbnails_dir = thumbnails_dir?;
    let previews_dir = thumbnails_dir.join("previews");
    if !previews_dir.exists() {
        return None;
    }
    let number = row.field_by_key("invoice_number");
    if number.is_empty() {
        return None;
    }
    let extensions = ["jpg", "jpeg", "png"];
    for ext in &extensions {
        if let Ok(entries) = std::fs::read_dir(&previews_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Ok(sub_entries) = std::fs::read_dir(&path) {
                        for sub_entry in sub_entries.flatten() {
                            let sub_path = sub_entry.path();
                            if sub_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .map(|n| n.contains(&number) && n.ends_with(ext))
                                .unwrap_or(false)
                            {
                                return Some(sub_path);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}
