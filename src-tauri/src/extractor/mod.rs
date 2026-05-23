use std::collections::HashMap;

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Deserialize)]
pub struct SaveInvoiceExtractionRequest {
    pub raw_file_id: i64,
    #[serde(default)]
    pub source_page_range: Option<String>,
    pub provider_name: Option<String>,
    pub model: Option<String>,
    pub response_json: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InvoiceSummary {
    pub id: i64,
    pub raw_file_id: i64,
    pub raw_file_mime: Option<String>,
    pub invoice_type: Option<String>,
    pub invoice_code: Option<String>,
    pub invoice_number: Option<String>,
    pub issue_date: Option<String>,
    pub seller_name: Option<String>,
    pub buyer_name: Option<String>,
    pub currency: String,
    pub total_amount: Option<String>,
    pub category: Option<String>,
    pub source_page_range: Option<String>,
    pub confidence: Option<f64>,
    pub status: String,
    pub duplicate_status: String,
    pub created_at: String,
    pub updated_at: String,
    pub viewed_at: Option<String>,
    pub item_names: Option<String>,
    pub badges: Vec<InvoiceBadgeSelection>,
}

#[derive(Debug, Deserialize)]
pub struct InvoiceSearchParams {
    pub query: Option<String>,
    pub invoice_type: Option<String>,
    pub seller_name: Option<String>,
    pub buyer_name: Option<String>,
    pub invoice_number: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub amount_min: Option<String>,
    pub amount_max: Option<String>,
    pub category: Option<String>,
    pub tag: Option<String>,
    pub status: Option<String>,
    pub duplicate_status: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InvoiceSearchResult {
    pub invoices: Vec<InvoiceSummary>,
    pub total_count: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

pub fn search_invoices(
    conn: &Connection,
    params: InvoiceSearchParams,
) -> Result<InvoiceSearchResult, ExtractorError> {
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(50).min(500).max(1);
    let offset = (page - 1) * page_size;

    let (where_clause, mut bind_values) = build_search_where(&params);
    let sort_clause = build_sort_clause(params.sort_by.as_deref(), params.sort_order.as_deref());

    let total_count: i64 = {
        let count_sql = format!("SELECT COUNT(*) FROM invoices WHERE 1=1 {where_clause}");
        let mut stmt = conn.prepare(&count_sql)?;
        let refs: Vec<&dyn rusqlite::types::ToSql> =
            bind_values.iter().map(|v| v.as_ref()).collect();
        stmt.query_row(refs.as_slice(), |row| row.get(0))?
    };

    let query_sql = format!(
        "SELECT
            id, raw_file_id,
            (SELECT mime_type FROM raw_files rf WHERE rf.id = invoices.raw_file_id) AS raw_file_mime,
            invoice_type, invoice_code, invoice_number,
            issue_date, seller_name, buyer_name, currency, total_amount,
            category, source_page_range, confidence, status, duplicate_status,
            created_at, updated_at, viewed_at,
            (SELECT GROUP_CONCAT(DISTINCT name) FROM invoice_items WHERE invoice_id = invoices.id) AS item_names
        FROM invoices
        WHERE 1=1 {where_clause}
        {sort_clause}
        LIMIT ?{} OFFSET ?{}",
        bind_values.len() + 1,
        bind_values.len() + 2,
    );

    bind_values.push(Box::new(page_size));
    bind_values.push(Box::new(offset));

    let mut stmt = conn.prepare(&query_sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        bind_values.iter().map(|v| v.as_ref()).collect();

    let mut invoices = stmt
        .query_map(param_refs.as_slice(), row_to_invoice_summary)?
        .collect::<Result<Vec<_>, _>>()?;
    attach_badges_to_summaries(conn, &mut invoices)?;

    let total_pages = ((total_count as f64) / (page_size as f64)).ceil() as i64;

    Ok(InvoiceSearchResult {
        invoices,
        total_count,
        page,
        page_size,
        total_pages: total_pages.max(1),
    })
}

fn build_search_where(
    params: &InvoiceSearchParams,
) -> (String, Vec<Box<dyn rusqlite::types::ToSql>>) {
    let mut clauses = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref query) = params.query {
        let q = format!("%{query}%");
        let n = values.len() + 1;
        clauses.push(format!(
            "(seller_name LIKE ?{n}
                OR buyer_name LIKE ?{n}
                OR invoice_number LIKE ?{n}
                OR invoice_code LIKE ?{n}
                OR invoice_type LIKE ?{n}
                OR category LIKE ?{n}
                OR remarks LIKE ?{n}
                OR EXISTS (
                    SELECT 1 FROM invoice_items item
                    WHERE item.invoice_id = invoices.id
                        AND (item.name LIKE ?{n} OR item.specification LIKE ?{n})
                ))"
        ));
        values.push(Box::new(q));
    }

    if let Some(ref t) = params.invoice_type {
        clauses.push(format!("invoice_type = ?{}", values.len() + 1));
        values.push(Box::new(t.clone()));
    }

    if let Some(ref s) = params.seller_name {
        clauses.push(format!("seller_name LIKE ?{}", values.len() + 1));
        values.push(Box::new(format!("%{s}%")));
    }

    if let Some(ref b) = params.buyer_name {
        clauses.push(format!("buyer_name LIKE ?{}", values.len() + 1));
        values.push(Box::new(format!("%{b}%")));
    }

    if let Some(ref n) = params.invoice_number {
        clauses.push(format!("invoice_number LIKE ?{}", values.len() + 1));
        values.push(Box::new(format!("%{n}%")));
    }

    if let Some(ref d) = params.date_from {
        clauses.push(format!("issue_date >= ?{}", values.len() + 1));
        values.push(Box::new(d.clone()));
    }

    if let Some(ref d) = params.date_to {
        clauses.push(format!("issue_date <= ?{}", values.len() + 1));
        values.push(Box::new(d.clone()));
    }

    if let Some(ref a) = params.amount_min {
        clauses.push(format!(
            "CAST(total_amount AS REAL) >= CAST(?{} AS REAL)",
            values.len() + 1
        ));
        values.push(Box::new(a.clone()));
    }

    if let Some(ref a) = params.amount_max {
        clauses.push(format!(
            "CAST(total_amount AS REAL) <= CAST(?{} AS REAL)",
            values.len() + 1
        ));
        values.push(Box::new(a.clone()));
    }

    if let Some(ref c) = params.category {
        clauses.push(format!("category LIKE ?{}", values.len() + 1));
        values.push(Box::new(format!("%{c}%")));
    }

    if let Some(ref tag) = params.tag {
        let trimmed = tag.trim();
        if !trimmed.is_empty() {
            let n = values.len() + 1;
            clauses.push(format!(
                "(category = ?{n}
                    OR invoice_type = ?{n}
                    OR (?{n} = '专票' AND invoice_type LIKE '%专用%')
                    OR (?{n} = '普票' AND invoice_type LIKE '%普通%')
                    OR (?{n} = 'PDF' AND EXISTS (
                        SELECT 1 FROM raw_files rf
                        WHERE rf.id = invoices.raw_file_id
                            AND rf.mime_type = 'application/pdf'
                    ))
                    OR (?{n} = '图片' AND EXISTS (
                        SELECT 1 FROM raw_files rf
                        WHERE rf.id = invoices.raw_file_id
                            AND rf.mime_type LIKE 'image/%'
                    ))
                    OR (?{n} = '文件' AND EXISTS (
                        SELECT 1 FROM raw_files rf
                        WHERE rf.id = invoices.raw_file_id
                            AND rf.mime_type IS NOT NULL
                            AND rf.mime_type != 'application/pdf'
                            AND rf.mime_type NOT LIKE 'image/%'
                    ))
                    OR EXISTS (
                        SELECT 1 FROM invoice_badges badge
                        WHERE badge.invoice_id = invoices.id
                            AND (badge.value = ?{n}
                                OR badge.group_name || ': ' || badge.value = ?{n})
                    ))"
            ));
            values.push(Box::new(trimmed.to_owned()));
        }
    }

    if let Some(ref s) = params.status {
        clauses.push(format!("status = ?{}", values.len() + 1));
        values.push(Box::new(s.clone()));
    }

    if let Some(ref d) = params.duplicate_status {
        clauses.push(format!("duplicate_status = ?{}", values.len() + 1));
        values.push(Box::new(d.clone()));
    }

    let where_clause = if clauses.is_empty() {
        String::new()
    } else {
        format!(" AND {}", clauses.join(" AND "))
    };

    (where_clause, values)
}

fn build_sort_clause(sort_by: Option<&str>, sort_order: Option<&str>) -> String {
    let valid_columns: &[&str] = &[
        "issue_date",
        "total_amount",
        "seller_name",
        "confidence",
        "created_at",
    ];

    let column = sort_by
        .filter(|s| valid_columns.contains(s))
        .unwrap_or("id");

    let direction = match sort_order {
        Some("asc") => "ASC",
        Some("desc") => "DESC",
        _ => "DESC",
    };

    format!("ORDER BY {column} {direction}")
}

#[derive(Debug, Clone, Serialize)]
pub struct InvoiceDetail {
    pub id: i64,
    pub raw_file_id: i64,
    pub invoice_type: Option<String>,
    pub invoice_code: Option<String>,
    pub invoice_number: Option<String>,
    pub issue_date: Option<String>,
    pub seller_name: Option<String>,
    pub seller_tax_id: Option<String>,
    pub buyer_name: Option<String>,
    pub buyer_tax_id: Option<String>,
    pub currency: String,
    pub amount_without_tax: Option<String>,
    pub tax_amount: Option<String>,
    pub total_amount: Option<String>,
    pub category: Option<String>,
    pub remarks: Option<String>,
    pub extra_fields: Option<String>,
    pub source_page_range: Option<String>,
    pub confidence: Option<f64>,
    pub status: String,
    pub duplicate_status: String,
    pub created_at: String,
    pub updated_at: String,
    pub viewed_at: Option<String>,
    pub items: Vec<InvoiceItemRow>,
    pub raw_file_name: Option<String>,
    pub raw_file_mime: Option<String>,
    pub raw_file_path: Option<String>,
    pub thumbnail_path: Option<String>,
    pub extraction_model: Option<String>,
    pub extraction_provider: Option<String>,
    pub badges: Vec<InvoiceBadgeSelection>,
    pub source_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InvoiceBadgeSelection {
    pub group_name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BadgeGroupConfig {
    pub name: String,
    pub options: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BadgeConfig {
    pub groups: Vec<BadgeGroupConfig>,
}

impl Default for BadgeConfig {
    fn default() -> Self {
        Self {
            groups: vec![
                BadgeGroupConfig {
                    name: "电商".to_owned(),
                    options: vec!["拼多多".to_owned(), "京东".to_owned(), "淘宝".to_owned()],
                },
                BadgeGroupConfig {
                    name: "类型".to_owned(),
                    options: vec![
                        "成品油".to_owned(),
                        "办公用品".to_owned(),
                        "食品".to_owned(),
                    ],
                },
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct InvoiceItemRow {
    pub id: i64,
    pub name: String,
    pub specification: Option<String>,
    pub unit: Option<String>,
    pub quantity: Option<String>,
    pub unit_price: Option<String>,
    pub amount: Option<String>,
    pub tax_rate: Option<String>,
    pub tax_amount: Option<String>,
}

pub fn get_invoice_detail(
    conn: &Connection,
    thumbnails_dir: &std::path::Path,
    invoice_id: i64,
) -> Result<InvoiceDetail, ExtractorError> {
    let invoice = conn.query_row(
        "SELECT
            id, raw_file_id, invoice_type, invoice_code, invoice_number,
            issue_date, seller_name, seller_tax_id, buyer_name, buyer_tax_id,
            currency, amount_without_tax, tax_amount, total_amount,
            category, remarks, extra_fields, source_page_range, confidence, status,
            duplicate_status, created_at, updated_at, viewed_at
        FROM invoices WHERE id = ?1",
        [invoice_id],
        |row| {
            Ok(InvoiceDetail {
                id: row.get(0)?,
                raw_file_id: row.get(1)?,
                invoice_type: row.get(2)?,
                invoice_code: row.get(3)?,
                invoice_number: row.get(4)?,
                issue_date: row.get(5)?,
                seller_name: row.get(6)?,
                seller_tax_id: row.get(7)?,
                buyer_name: row.get(8)?,
                buyer_tax_id: row.get(9)?,
                currency: row
                    .get::<_, Option<String>>(10)?
                    .unwrap_or_else(|| "CNY".into()),
                amount_without_tax: row.get(11)?,
                tax_amount: row.get(12)?,
                total_amount: row.get(13)?,
                category: row.get(14)?,
                remarks: row.get(15)?,
                extra_fields: row.get(16)?,
                source_page_range: row.get(17)?,
                confidence: row.get(18)?,
                status: row
                    .get::<_, Option<String>>(19)?
                    .unwrap_or_else(|| "pending_confirmation".into()),
                duplicate_status: row
                    .get::<_, Option<String>>(20)?
                    .unwrap_or_else(|| "unknown".into()),
                created_at: row.get(21)?,
                updated_at: row.get(22)?,
                viewed_at: row.get(23)?,
                items: Vec::new(),
                raw_file_name: None,
                raw_file_mime: None,
                raw_file_path: None,
                thumbnail_path: None,
                extraction_model: None,
                extraction_provider: None,
                badges: Vec::new(),
                source_type: None,
            })
        },
    )?;

    // Load items
    let mut stmt = conn.prepare(
        "SELECT id, name, specification, unit, quantity, unit_price, amount, tax_rate, tax_amount
        FROM invoice_items WHERE invoice_id = ?1 ORDER BY id",
    )?;
    let items = stmt
        .query_map([invoice_id], |row| {
            Ok(InvoiceItemRow {
                id: row.get(0)?,
                name: row.get(1)?,
                specification: row.get(2)?,
                unit: row.get(3)?,
                quantity: row.get(4)?,
                unit_price: row.get(5)?,
                amount: row.get(6)?,
                tax_rate: row.get(7)?,
                tax_amount: row.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // Load raw_file info and thumbnail path
    let (raw_name, raw_mime, raw_path, source_page_range): (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = conn.query_row(
        "SELECT rf.original_name, rf.mime_type, rf.storage_path, inv.source_page_range
            FROM invoices inv JOIN raw_files rf ON rf.id = inv.raw_file_id
            WHERE inv.id = ?1",
        [invoice_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;

    let preview_dir = thumbnails_dir
        .join("previews")
        .join(invoice.raw_file_id.to_string());
    let page_thumbnail = source_page_range.as_ref().and_then(|page_range| {
        let page = page_range
            .split('-')
            .next()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(1);
        let path = preview_dir.join(format!("page-{page}.jpg"));
        path.exists().then(|| path.to_string_lossy().into_owned())
    });
    let image_thumbnail = preview_dir.join("image.jpg");
    let thumbnail_path = page_thumbnail.or_else(|| {
        image_thumbnail
            .exists()
            .then(|| image_thumbnail.to_string_lossy().into_owned())
    });

    // Load extraction info
    let (model, provider): (Option<String>, Option<String>) = conn
        .query_row(
            "SELECT model, provider_name FROM extraction_runs WHERE invoice_id = ?1 ORDER BY id DESC LIMIT 1",
            [invoice_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .unwrap_or((None, None));

    let badges = list_invoice_badges(conn, invoice_id)?;

    let source_type: Option<String> = conn
        .query_row(
            "SELECT ij.source_type FROM import_jobs ij WHERE ij.raw_file_id = ?1 ORDER BY ij.id DESC LIMIT 1",
            [invoice.raw_file_id],
            |row| row.get(0),
        )
        .optional()?;

    Ok(InvoiceDetail {
        items,
        raw_file_name: raw_name,
        raw_file_mime: raw_mime,
        raw_file_path: raw_path,
        thumbnail_path,
        extraction_model: model,
        extraction_provider: provider,
        badges,
        source_type,
        ..invoice
    })
}

pub fn list_invoice_badges(
    conn: &Connection,
    invoice_id: i64,
) -> Result<Vec<InvoiceBadgeSelection>, ExtractorError> {
    let mut stmt = conn.prepare(
        "SELECT group_name, value
        FROM invoice_badges
        WHERE invoice_id = ?1
        ORDER BY group_name",
    )?;

    let badges = stmt
        .query_map([invoice_id], |row| {
            Ok(InvoiceBadgeSelection {
                group_name: row.get(0)?,
                value: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(badges)
}

pub fn set_invoice_badge(
    conn: &mut Connection,
    invoice_id: i64,
    group_name: String,
    value: Option<String>,
) -> Result<Vec<InvoiceBadgeSelection>, ExtractorError> {
    let group_name = group_name.trim().to_owned();
    let value = value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());

    if group_name.is_empty() {
        return Ok(list_invoice_badges(conn, invoice_id)?);
    }

    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM invoices WHERE id = ?1)",
        [invoice_id],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(ExtractorError::NotFound(invoice_id));
    }

    let tx = conn.transaction()?;
    if let Some(value) = value {
        tx.execute(
            "INSERT INTO invoice_badges (invoice_id, group_name, value, updated_at)
            VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)
            ON CONFLICT(invoice_id, group_name) DO UPDATE SET
                value = excluded.value,
                updated_at = CURRENT_TIMESTAMP",
            params![invoice_id, group_name, value],
        )?;
    } else {
        tx.execute(
            "DELETE FROM invoice_badges WHERE invoice_id = ?1 AND group_name = ?2",
            params![invoice_id, group_name],
        )?;
    }
    tx.commit()?;

    list_invoice_badges(conn, invoice_id)
}

#[derive(Debug, Deserialize)]
pub struct UpdateInvoiceRequest {
    pub id: i64,
    pub invoice_type: Option<Option<String>>,
    pub invoice_code: Option<Option<String>>,
    pub invoice_number: Option<Option<String>>,
    pub issue_date: Option<Option<String>>,
    pub seller_name: Option<Option<String>>,
    pub seller_tax_id: Option<Option<String>>,
    pub buyer_name: Option<Option<String>>,
    pub buyer_tax_id: Option<Option<String>>,
    pub currency: Option<Option<String>>,
    pub amount_without_tax: Option<Option<String>>,
    pub tax_amount: Option<Option<String>>,
    pub total_amount: Option<Option<String>>,
    pub category: Option<Option<String>>,
    pub remarks: Option<Option<String>>,
    pub confidence: Option<Option<f64>>,
    pub status: Option<Option<String>>,
    #[serde(default)]
    pub extra_fields: Option<Option<Map<String, Value>>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInvoiceResult {
    pub invoice: InvoiceSummary,
    pub errors: Vec<FieldError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

pub fn update_invoice(
    conn: &mut Connection,
    request: UpdateInvoiceRequest,
) -> Result<UpdateInvoiceResult, ExtractorError> {
    let errors = validate_invoice_fields(&request.issue_date, &request.status, request.confidence);

    let tx = conn.transaction()?;

    let mut set_clauses = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    macro_rules! add_field {
        ($field:ident, $val:expr) => {
            if let Some(v) = $val {
                set_clauses.push(format!("{} = ?{}", stringify!($field), values.len() + 1));
                values.push(Box::new(v));
            }
        };
    }

    add_field!(invoice_type, request.invoice_type);
    add_field!(invoice_code, request.invoice_code);
    add_field!(invoice_number, request.invoice_number);
    add_field!(issue_date, request.issue_date);
    add_field!(seller_name, request.seller_name);
    add_field!(seller_tax_id, request.seller_tax_id);
    add_field!(buyer_name, request.buyer_name);
    add_field!(buyer_tax_id, request.buyer_tax_id);
    add_field!(currency, request.currency);
    add_field!(amount_without_tax, request.amount_without_tax);
    add_field!(tax_amount, request.tax_amount);
    add_field!(total_amount, request.total_amount);
    add_field!(category, request.category);
    add_field!(remarks, request.remarks);
    add_field!(confidence, request.confidence);
    add_field!(status, request.status);

    if let Some(v) = request.extra_fields {
        set_clauses.push(format!("extra_fields = ?{}", values.len() + 1));
        let json_str = v.map(|m| serde_json::to_string(&m).unwrap_or_default());
        values.push(Box::new(json_str));
    }

    if set_clauses.is_empty() {
        drop(tx);
        return Ok(UpdateInvoiceResult {
            invoice: load_invoice_summary(conn, request.id)?,
            errors,
        });
    }

    set_clauses.push("updated_at = CURRENT_TIMESTAMP".to_owned());
    let sql = format!(
        "UPDATE invoices SET {} WHERE id = ?{}",
        set_clauses.join(", "),
        values.len() + 1
    );
    values.push(Box::new(request.id));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = values.iter().map(|v| v.as_ref()).collect();
    tx.execute(&sql, param_refs.as_slice())?;
    tx.commit()?;

    let invoice = load_invoice_summary(conn, request.id)?;
    Ok(UpdateInvoiceResult { invoice, errors })
}

fn validate_invoice_fields(
    issue_date: &Option<Option<String>>,
    status: &Option<Option<String>>,
    confidence: Option<Option<f64>>,
) -> Vec<FieldError> {
    let mut errors = Vec::new();

    if let Some(Some(ref date)) = issue_date {
        if !date.is_empty() && NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
            errors.push(FieldError {
                field: "issue_date".into(),
                message: "日期格式必须为 YYYY-MM-DD".into(),
            });
        }
    }

    if let Some(Some(ref s)) = status {
        let valid = ["pending_confirmation", "recognized", "reviewed", "flagged"];
        if !s.is_empty() && !valid.contains(&s.as_str()) {
            errors.push(FieldError {
                field: "status".into(),
                message: format!("状态值无效，允许的值: {valid:?}"),
            });
        }
    }

    if let Some(Some(c)) = confidence {
        if !(0.0..=1.0).contains(&c) {
            errors.push(FieldError {
                field: "confidence".into(),
                message: "置信度必须在 0 到 1 之间".into(),
            });
        }
    }

    errors
}

#[derive(Debug, Deserialize)]
pub struct UpdateInvoiceItemsRequest {
    pub invoice_id: i64,
    pub items: Vec<InvoiceItemChange>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action")]
pub enum InvoiceItemChange {
    #[serde(rename = "add")]
    Add {
        name: String,
        specification: Option<String>,
        unit: Option<String>,
        quantity: Option<String>,
        unit_price: Option<String>,
        amount: Option<String>,
        tax_rate: Option<String>,
        tax_amount: Option<String>,
    },
    #[serde(rename = "update")]
    Update {
        id: i64,
        name: Option<String>,
        specification: Option<String>,
        unit: Option<String>,
        quantity: Option<String>,
        unit_price: Option<String>,
        amount: Option<String>,
        tax_rate: Option<String>,
        tax_amount: Option<String>,
    },
    #[serde(rename = "delete")]
    Delete { id: i64 },
}

pub fn update_invoice_items(
    conn: &mut Connection,
    request: UpdateInvoiceItemsRequest,
) -> Result<Vec<InvoiceItemRow>, ExtractorError> {
    let tx = conn.transaction()?;

    for change in &request.items {
        match change {
            InvoiceItemChange::Add {
                name,
                specification,
                unit,
                quantity,
                unit_price,
                amount,
                tax_rate,
                tax_amount,
            } => {
                tx.execute(
                    "INSERT INTO invoice_items (invoice_id, name, specification, unit, quantity, unit_price, amount, tax_rate, tax_amount)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        request.invoice_id,
                        name,
                        specification,
                        unit,
                        quantity,
                        unit_price,
                        amount,
                        tax_rate,
                        tax_amount,
                    ],
                )?;
            }
            InvoiceItemChange::Update {
                id,
                name,
                specification,
                unit,
                quantity,
                unit_price,
                amount,
                tax_rate,
                tax_amount,
            } => {
                let mut set = Vec::new();
                let mut vals: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

                let mut add = |col: &str, val: &Option<String>| {
                    if val.is_some() {
                        set.push(format!("{col} = ?{}", vals.len() + 1));
                        vals.push(Box::new(val.clone()));
                    }
                };

                add("name", name);
                add("specification", specification);
                add("unit", unit);
                add("quantity", quantity);
                add("unit_price", unit_price);
                add("amount", amount);
                add("tax_rate", tax_rate);
                add("tax_amount", tax_amount);

                if !set.is_empty() {
                    let sql = format!(
                        "UPDATE invoice_items SET {} WHERE id = ?{} AND invoice_id = ?{}",
                        set.join(", "),
                        vals.len() + 1,
                        vals.len() + 2,
                    );
                    vals.push(Box::new(*id));
                    vals.push(Box::new(request.invoice_id));
                    let refs: Vec<&dyn rusqlite::types::ToSql> =
                        vals.iter().map(|v| v.as_ref()).collect();
                    tx.execute(&sql, refs.as_slice())?;
                }
            }
            InvoiceItemChange::Delete { id } => {
                tx.execute(
                    "DELETE FROM invoice_items WHERE id = ?1 AND invoice_id = ?2",
                    params![id, request.invoice_id],
                )?;
            }
        }
    }

    tx.commit()?;

    // Return updated items list
    let mut stmt = conn.prepare(
        "SELECT id, name, specification, unit, quantity, unit_price, amount, tax_rate, tax_amount
        FROM invoice_items WHERE invoice_id = ?1 ORDER BY id",
    )?;
    let items = stmt
        .query_map([request.invoice_id], |row| {
            Ok(InvoiceItemRow {
                id: row.get(0)?,
                name: row.get(1)?,
                specification: row.get(2)?,
                unit: row.get(3)?,
                quantity: row.get(4)?,
                unit_price: row.get(5)?,
                amount: row.get(6)?,
                tax_rate: row.get(7)?,
                tax_amount: row.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(items)
}

#[derive(Debug, Deserialize)]
struct InvoiceExtraction {
    is_invoice: bool,
    #[serde(default)]
    invoice_type: Option<String>,
    #[serde(default)]
    invoice_code: Option<String>,
    #[serde(default)]
    invoice_number: Option<String>,
    #[serde(default)]
    issue_date: Option<String>,
    #[serde(default)]
    seller: PartyExtraction,
    #[serde(default)]
    buyer: PartyExtraction,
    #[serde(default)]
    currency: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_amount")]
    amount_without_tax: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_amount")]
    tax_amount: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_amount")]
    total_amount: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    items: Vec<InvoiceItemExtraction>,
    #[serde(default)]
    remarks: Option<String>,
    #[serde(default)]
    extra_fields: Map<String, Value>,
    #[serde(default)]
    confidence: Option<f64>,
    #[serde(default)]
    needs_review: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct PartyExtraction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    tax_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct InvoiceItemExtraction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    spec: Option<String>,
    #[serde(default)]
    unit: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_amount")]
    quantity: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_amount")]
    unit_price: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_amount")]
    amount: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_amount")]
    tax_rate: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_amount")]
    tax_amount: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ExtractorError {
    #[error("raw file does not exist: {0}")]
    MissingRawFile(i64),
    #[error("文件不含有发票")]
    NonInvoice,
    #[error("识别置信度过低，可能是图片分辨率不清晰或发票内容不完整")]
    LowConfidence,
    #[error("invoice not found: {0}")]
    NotFound(i64),
    #[error("invalid extraction JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("invalid issue date, expected YYYY-MM-DD: {0}")]
    InvalidIssueDate(String),
    #[error("confidence must be between 0 and 1")]
    InvalidConfidence,
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("合并失败: {0}")]
    MergeError(String),
}

pub fn save_invoice_extraction(
    conn: &mut Connection,
    request: SaveInvoiceExtractionRequest,
) -> Result<InvoiceSummary, ExtractorError> {
    ensure_raw_file_exists(conn, request.raw_file_id)?;

    let extraction = parse_invoice_extraction_json(&request.response_json)?;
    let tx = conn.transaction()?;
    let invoice_id = insert_invoice(
        &tx,
        request.raw_file_id,
        request.source_page_range.as_deref(),
        &extraction,
    )?;
    insert_invoice_items(&tx, invoice_id, &extraction.items)?;
    insert_extraction_run(&tx, request, invoice_id)?;
    tx.commit()?;

    load_invoice_summary(conn, invoice_id)
}

pub fn list_invoices(conn: &Connection) -> Result<Vec<InvoiceSummary>, ExtractorError> {
    let mut stmt = conn.prepare(
        "SELECT
            id,
            raw_file_id,
            (SELECT mime_type FROM raw_files rf WHERE rf.id = invoices.raw_file_id) AS raw_file_mime,
            invoice_type,
            invoice_code,
            invoice_number,
            issue_date,
            seller_name,
            buyer_name,
            currency,
            total_amount,
            category,
            source_page_range,
            confidence,
            status,
            duplicate_status,
            created_at,
            updated_at,
            viewed_at,
            (SELECT GROUP_CONCAT(DISTINCT name) FROM invoice_items WHERE invoice_id = invoices.id) AS item_names
        FROM invoices
        ORDER BY id DESC
        LIMIT 100",
    )?;

    let mut invoices = stmt
        .query_map([], row_to_invoice_summary)?
        .collect::<Result<Vec<_>, _>>()?;
    attach_badges_to_summaries(conn, &mut invoices)?;

    Ok(invoices)
}

fn parse_invoice_extraction_json(value: &str) -> Result<InvoiceExtraction, ExtractorError> {
    let mut extraction: InvoiceExtraction = serde_json::from_str(value)?;
    normalize_extraction(&mut extraction)?;
    Ok(extraction)
}

fn normalize_extraction(extraction: &mut InvoiceExtraction) -> Result<(), ExtractorError> {
    if !extraction.is_invoice {
        return Err(ExtractorError::NonInvoice);
    }

    extraction.invoice_type = clean_optional(extraction.invoice_type.take());
    extraction.invoice_code = clean_optional(extraction.invoice_code.take());
    extraction.invoice_number = clean_optional(extraction.invoice_number.take());
    extraction.issue_date = clean_optional(extraction.issue_date.take());
    extraction.seller.name = clean_optional(extraction.seller.name.take());
    extraction.seller.tax_id = clean_optional(extraction.seller.tax_id.take());
    extraction.buyer.name = clean_optional(extraction.buyer.name.take());
    extraction.buyer.tax_id = clean_optional(extraction.buyer.tax_id.take());
    extraction.currency = clean_optional(extraction.currency.take()).or_else(|| Some("CNY".into()));
    extraction.category = clean_optional(extraction.category.take());
    extraction.remarks = clean_optional(extraction.remarks.take());
    normalize_extra_fields(&mut extraction.extra_fields);

    if let Some(issue_date) = extraction.issue_date.as_deref() {
        NaiveDate::parse_from_str(issue_date, "%Y-%m-%d")
            .map_err(|_| ExtractorError::InvalidIssueDate(issue_date.to_owned()))?;
    }

    if extraction
        .confidence
        .is_some_and(|confidence| !(0.0..=1.0).contains(&confidence))
    {
        return Err(ExtractorError::InvalidConfidence);
    }

    for item in &mut extraction.items {
        item.name = clean_optional(item.name.take());
        item.spec = clean_optional(item.spec.take());
        item.unit = clean_optional(item.unit.take());
    }

    extraction.items.retain(has_meaningful_item_data);

    Ok(())
}

fn insert_invoice(
    conn: &Connection,
    raw_file_id: i64,
    source_page_range: Option<&str>,
    extraction: &InvoiceExtraction,
) -> Result<i64, ExtractorError> {
    let currency = extraction.currency.as_deref().unwrap_or("CNY");
    let status = "recognized";

    conn.execute(
        "INSERT INTO invoices (
            raw_file_id,
            invoice_type,
            invoice_code,
            invoice_number,
            issue_date,
            seller_name,
            seller_tax_id,
            buyer_name,
            buyer_tax_id,
            currency,
            amount_without_tax,
            tax_amount,
            total_amount,
            category,
            remarks,
            extra_fields,
            source_page_range,
            confidence,
            status,
            duplicate_status
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, 'unknown')",
        params![
            raw_file_id,
            extraction.invoice_type,
            extraction.invoice_code,
            extraction.invoice_number,
            extraction.issue_date,
            extraction.seller.name,
            extraction.seller.tax_id,
            extraction.buyer.name,
            extraction.buyer.tax_id,
            currency,
            extraction.amount_without_tax,
            extraction.tax_amount,
            extraction.total_amount,
            extraction.category,
            extraction.remarks,
            extra_fields_json(extraction),
            source_page_range,
            extraction.confidence,
            status,
        ],
    )?;

    Ok(conn.last_insert_rowid())
}

fn normalize_extra_fields(fields: &mut Map<String, Value>) {
    fields.retain(|key, value| {
        if key.trim().is_empty() {
            return false;
        }
        match value {
            Value::Null => false,
            Value::String(s) => !s.trim().is_empty(),
            Value::Array(items) => !items.is_empty(),
            Value::Object(map) => !map.is_empty(),
            _ => true,
        }
    });
}

fn extra_fields_json(extraction: &InvoiceExtraction) -> Option<String> {
    (!extraction.extra_fields.is_empty())
        .then(|| Value::Object(extraction.extra_fields.clone()).to_string())
}

fn insert_invoice_items(
    conn: &Connection,
    invoice_id: i64,
    items: &[InvoiceItemExtraction],
) -> Result<(), ExtractorError> {
    for item in items {
        conn.execute(
            "INSERT INTO invoice_items (
                invoice_id,
                name,
                specification,
                unit,
                quantity,
                unit_price,
                amount,
                tax_rate,
                tax_amount
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                invoice_id,
                item.name.as_deref().unwrap_or("未命名明细"),
                item.spec,
                item.unit,
                item.quantity,
                item.unit_price,
                item.amount,
                item.tax_rate,
                item.tax_amount,
            ],
        )?;
    }

    Ok(())
}

fn insert_extraction_run(
    conn: &Connection,
    request: SaveInvoiceExtractionRequest,
    invoice_id: i64,
) -> Result<(), ExtractorError> {
    let provider_name = clean_optional(request.provider_name).unwrap_or_else(|| "manual".into());
    let model = clean_optional(request.model).unwrap_or_else(|| "unknown".into());
    let response_summary: String = request.response_json.chars().take(500).collect();

    conn.execute(
        "INSERT INTO extraction_runs (
            raw_file_id,
            invoice_id,
            provider_name,
            model,
            status,
            request_started_at,
            response_summary
        ) VALUES (?1, ?2, ?3, ?4, 'completed', CURRENT_TIMESTAMP, ?5)",
        params![
            request.raw_file_id,
            invoice_id,
            provider_name,
            model,
            response_summary,
        ],
    )?;

    Ok(())
}

fn load_invoice_summary(
    conn: &Connection,
    invoice_id: i64,
) -> Result<InvoiceSummary, ExtractorError> {
    let mut invoice = conn
        .query_row(
        "SELECT
            id,
            raw_file_id,
            (SELECT mime_type FROM raw_files rf WHERE rf.id = invoices.raw_file_id) AS raw_file_mime,
            invoice_type,
            invoice_code,
            invoice_number,
            issue_date,
            seller_name,
            buyer_name,
            currency,
            total_amount,
            category,
            source_page_range,
            confidence,
            status,
            duplicate_status,
            created_at,
            updated_at,
            viewed_at,
            (SELECT GROUP_CONCAT(DISTINCT name) FROM invoice_items WHERE invoice_id = invoices.id) AS item_names
        FROM invoices
        WHERE id = ?1",
        [invoice_id],
        row_to_invoice_summary,
    )
    .map_err(ExtractorError::from)?;
    invoice.badges = list_invoice_badges(conn, invoice_id)?;
    Ok(invoice)
}

fn row_to_invoice_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<InvoiceSummary> {
    Ok(InvoiceSummary {
        id: row.get(0)?,
        raw_file_id: row.get(1)?,
        raw_file_mime: row.get(2)?,
        invoice_type: row.get(3)?,
        invoice_code: row.get(4)?,
        invoice_number: row.get(5)?,
        issue_date: row.get(6)?,
        seller_name: row.get(7)?,
        buyer_name: row.get(8)?,
        currency: row.get(9)?,
        total_amount: row.get(10)?,
        category: row.get(11)?,
        source_page_range: row.get(12)?,
        confidence: row.get(13)?,
        status: row.get(14)?,
        duplicate_status: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
        viewed_at: row.get(18)?,
        item_names: row.get(19)?,
        badges: Vec::new(),
    })
}

pub fn mark_invoice_viewed(conn: &Connection, invoice_id: i64) -> Result<bool, ExtractorError> {
    let changed = conn.execute(
        "UPDATE invoices
        SET viewed_at = COALESCE(viewed_at, CURRENT_TIMESTAMP),
            updated_at = CURRENT_TIMESTAMP
        WHERE id = ?1 AND viewed_at IS NULL",
        [invoice_id],
    )?;
    Ok(changed > 0)
}

pub fn count_unviewed_invoices(conn: &Connection) -> Result<i64, ExtractorError> {
    conn.query_row(
        "SELECT COUNT(*) FROM invoices WHERE viewed_at IS NULL",
        [],
        |row| row.get(0),
    )
    .map_err(ExtractorError::from)
}

fn attach_badges_to_summaries(
    conn: &Connection,
    invoices: &mut [InvoiceSummary],
) -> Result<(), ExtractorError> {
    if invoices.is_empty() {
        return Ok(());
    }

    let ids = invoices
        .iter()
        .map(|invoice| invoice.id.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let mut stmt = conn.prepare(&format!(
        "SELECT invoice_id, group_name, value
         FROM invoice_badges
         WHERE invoice_id IN ({ids})
         ORDER BY invoice_id, group_name"
    ))?;

    let mut badges_by_invoice: HashMap<i64, Vec<InvoiceBadgeSelection>> = HashMap::new();
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            InvoiceBadgeSelection {
                group_name: row.get(1)?,
                value: row.get(2)?,
            },
        ))
    })?;

    for row in rows {
        let (invoice_id, badge) = row?;
        badges_by_invoice.entry(invoice_id).or_default().push(badge);
    }

    for invoice in invoices {
        invoice.badges = badges_by_invoice.remove(&invoice.id).unwrap_or_default();
    }

    Ok(())
}

fn ensure_raw_file_exists(conn: &Connection, raw_file_id: i64) -> Result<(), ExtractorError> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT id FROM raw_files WHERE id = ?1",
            [raw_file_id],
            |row| row.get(0),
        )
        .optional()?;

    if exists.is_some() {
        Ok(())
    } else {
        Err(ExtractorError::MissingRawFile(raw_file_id))
    }
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn has_meaningful_item_data(item: &InvoiceItemExtraction) -> bool {
    item.name.is_some()
        || item.spec.is_some()
        || item.unit.is_some()
        || item.quantity.is_some()
        || item.unit_price.is_some()
        || item.amount.is_some()
        || item.tax_rate.is_some()
        || item.tax_amount.is_some()
}

fn deserialize_optional_amount<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let Some(value) = value else {
        return Ok(None);
    };

    match value {
        serde_json::Value::Number(number) => Ok(Some(number.to_string())),
        serde_json::Value::String(value) => Ok(clean_optional(Some(value))),
        serde_json::Value::Null => Ok(None),
        other => Err(serde::de::Error::custom(format!(
            "expected amount as number, string, or null, got {other}"
        ))),
    }
}

// ---- Dashboard ----

#[derive(Debug, Clone, Serialize)]
pub struct DashboardStats {
    pub total_invoices: i64,
    pub total_amount: f64,
    pub currency: String,
    pub average_confidence: f64,
    pub this_month_count: i64,
    pub this_month_amount: f64,
    pub pending_count: i64,
    pub duplicate_count: i64,
    pub monthly_trend: Vec<MonthlyTrendPoint>,
    pub by_type: Vec<BreakdownItem>,
    pub by_status: Vec<BreakdownItem>,
    pub top_sellers: Vec<TopSellerItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MonthlyTrendPoint {
    pub month: String,
    pub count: i64,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BreakdownItem {
    pub label: String,
    pub count: i64,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TopSellerItem {
    pub seller_name: String,
    pub count: i64,
    pub amount: f64,
}

fn sum_amount() -> &'static str {
    "COALESCE(SUM(CAST(total_amount AS REAL)), 0.0)"
}

pub fn get_dashboard_stats(
    conn: &Connection,
    date_from: Option<&str>,
    date_to: Option<&str>,
) -> Result<DashboardStats, ExtractorError> {
    let where_clause: String = {
        let mut clauses: Vec<String> = Vec::new();
        if let Some(from) = date_from {
            clauses.push(format!("issue_date >= '{}'", from));
        }
        if let Some(to) = date_to {
            clauses.push(format!("issue_date <= '{}'", to));
        }
        if clauses.is_empty() {
            String::new()
        } else {
            format!(" AND {}", clauses.join(" AND "))
        }
    };

    let this_month = chrono::Local::now().format("%Y-%m-01").to_string();

    // Consolidated: total, amount, avg confidence, this_month, pending, duplicates
    // merged from 6 separate queries into 1 using conditional aggregation
    let (total_invoices, total_amount, average_confidence, this_month_count, this_month_amount, pending_count, duplicate_count): (i64, f64, f64, i64, f64, i64, i64) = conn.query_row(
        &format!(
            "SELECT COUNT(*), {sum}, COALESCE(AVG(confidence), 0.0),
                    COALESCE(SUM(CASE WHEN issue_date >= ?1 THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN issue_date >= ?1 THEN CAST(total_amount AS REAL) ELSE 0 END), 0.0),
                    COALESCE(SUM(CASE WHEN status = 'pending_confirmation' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN duplicate_status IN ('possible_duplicate', 'probable_duplicate') THEN 1 ELSE 0 END), 0)
             FROM invoices WHERE 1=1{where}",
            sum = sum_amount(),
            where = where_clause
        ),
        [&this_month],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
    )?;

    let currency = conn
        .query_row(
            "SELECT currency FROM invoices GROUP BY currency ORDER BY COUNT(*) DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "CNY".to_string());

    let mut trend_stmt = conn.prepare(&format!(
        "SELECT strftime('%Y-%m', issue_date) as month, COUNT(*), {}
            FROM invoices
            WHERE issue_date IS NOT NULL{}
            GROUP BY month
            ORDER BY month DESC
            LIMIT 12",
        sum_amount(),
        where_clause
    ))?;
    let trend_rows: Vec<(String, i64, f64)> = trend_stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut monthly_trend: Vec<MonthlyTrendPoint> = trend_rows
        .into_iter()
        .map(|(month, count, amount)| MonthlyTrendPoint {
            month,
            count,
            amount,
        })
        .collect();
    monthly_trend.reverse();

    let mut type_stmt = conn.prepare(&format!(
        "SELECT COALESCE(invoice_type, '未知') as label, COUNT(*), {}
            FROM invoices
            WHERE 1=1{}
            GROUP BY invoice_type
            ORDER BY COUNT(*) DESC",
        sum_amount(),
        where_clause
    ))?;
    let by_type: Vec<BreakdownItem> = type_stmt
        .query_map([], |row| {
            Ok(BreakdownItem {
                label: row.get(0)?,
                count: row.get(1)?,
                amount: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut status_stmt = conn.prepare(&format!(
        "SELECT status, COUNT(*), {}
            FROM invoices
            WHERE 1=1{}
            GROUP BY status
            ORDER BY COUNT(*) DESC",
        sum_amount(),
        where_clause
    ))?;
    let by_status: Vec<BreakdownItem> = status_stmt
        .query_map([], |row| {
            Ok(BreakdownItem {
                label: row.get(0)?,
                count: row.get(1)?,
                amount: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut seller_stmt = conn.prepare(&format!(
        "SELECT COALESCE(seller_name, '未知') as name, COUNT(*), {}
            FROM invoices
            WHERE 1=1{}
            GROUP BY seller_name
            ORDER BY COUNT(*) DESC
            LIMIT 5",
        sum_amount(),
        where_clause
    ))?;
    let top_sellers: Vec<TopSellerItem> = seller_stmt
        .query_map([], |row| {
            Ok(TopSellerItem {
                seller_name: row.get(0)?,
                count: row.get(1)?,
                amount: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(DashboardStats {
        total_invoices,
        total_amount,
        currency,
        average_confidence,
        this_month_count,
        this_month_amount,
        pending_count,
        duplicate_count,
        monthly_trend,
        by_type,
        by_status,
        top_sellers,
    })
}

pub fn invoice_to_embedding_text(invoice: &InvoiceDetail) -> String {
    let mut parts: Vec<String> = Vec::new();

    if let Some(ref t) = invoice.invoice_type {
        parts.push(format!("类型:{}", t));
    }
    if let Some(ref s) = invoice.seller_name {
        parts.push(format!("卖方:{}", s));
    }
    if let Some(ref b) = invoice.buyer_name {
        parts.push(format!("买方:{}", b));
    }
    if let Some(ref a) = invoice.total_amount {
        parts.push(format!("金额:{}", a));
    }
    if let Some(ref c) = invoice.category {
        parts.push(format!("类别:{}", c));
    }
    if !invoice.items.is_empty() {
        let item_names: Vec<&str> = invoice.items.iter().map(|i| i.name.as_str()).collect();
        parts.push(format!("项目:{}", item_names.join(",")));
    }
    if let Some(ref r) = invoice.remarks {
        if !r.is_empty() {
            parts.push(format!("备注:{}", r));
        }
    }

    let text = parts.join(" ");
    if text.len() > 8192 {
        text[..8192].to_string()
    } else {
        text
    }
}

// ---- Batch operations ----

#[derive(Debug, Deserialize)]
pub struct BatchUpdateRequest {
    pub ids: Vec<i64>,
    pub status: Option<String>,
    pub category: Option<String>,
}

pub fn batch_update_invoices(
    conn: &Connection,
    request: &BatchUpdateRequest,
) -> Result<Vec<InvoiceSummary>, ExtractorError> {
    if request.ids.is_empty() {
        return Ok(Vec::new());
    }

    let ids_str = request
        .ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");

    if let Some(ref status) = request.status {
        conn.execute(
            &format!("UPDATE invoices SET status = ?1 WHERE id IN ({})", ids_str),
            [status],
        )?;
    }
    if let Some(ref category) = request.category {
        conn.execute(
            &format!(
                "UPDATE invoices SET category = ?1 WHERE id IN ({})",
                ids_str
            ),
            [category],
        )?;
    }

    let mut stmt = conn.prepare(&format!(
        "SELECT
            id, raw_file_id,
            (SELECT mime_type FROM raw_files rf WHERE rf.id = invoices.raw_file_id) AS raw_file_mime,
            invoice_type, invoice_code, invoice_number, issue_date,
            seller_name, buyer_name, currency, total_amount, category,
            source_page_range, confidence, status, duplicate_status,
            created_at, updated_at, viewed_at,
            (SELECT GROUP_CONCAT(DISTINCT name) FROM invoice_items WHERE invoice_id = invoices.id) AS item_names
        FROM invoices WHERE id IN ({}) ORDER BY id",
        ids_str
    ))?;
    let mut rows = stmt
        .query_map([], |row| row_to_invoice_summary(row))?
        .collect::<Result<Vec<_>, _>>()?;
    attach_badges_to_summaries(conn, &mut rows)?;
    Ok(rows)
}

pub fn batch_delete_invoices(conn: &Connection, ids: &[i64]) -> Result<usize, ExtractorError> {
    if ids.is_empty() {
        return Ok(0);
    }
    let ids_str = ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");
    // Collect raw_file_ids before deleting invoices (so we can clean up orphaned raw files)
    let raw_file_ids: Vec<i64> = {
        let mut stmt = conn.prepare(&format!(
            "SELECT DISTINCT raw_file_id FROM invoices WHERE id IN ({})",
            ids_str
        ))?;
        let rows = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    // Delete referencing rows first (extraction_runs has no ON DELETE CASCADE)
    conn.execute(
        &format!(
            "DELETE FROM extraction_runs WHERE invoice_id IN ({})",
            ids_str
        ),
        [],
    )?;
    conn.execute(
        &format!(
            "DELETE FROM events WHERE reference_type = 'invoice' AND reference_id IN ({})",
            ids_str
        ),
        [],
    )?;
    conn.execute(
        &format!(
            "DELETE FROM dedupe_candidates WHERE invoice_id IN ({0}) OR candidate_invoice_id IN ({0})",
            ids_str
        ),
        [],
    )?;
    let count = conn.execute(
        &format!("DELETE FROM invoices WHERE id IN ({})", ids_str),
        [],
    )?;
    // Also clean up orphaned raw_files: find which of our raw_file_ids now have zero invoices
    if !raw_file_ids.is_empty() {
        let orphan_ids_str = raw_file_ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let mut stmt = conn.prepare(&format!(
            "SELECT rf.id FROM raw_files rf
             WHERE rf.id IN ({orphan_ids_str})
               AND NOT EXISTS (SELECT 1 FROM invoices i WHERE i.raw_file_id = rf.id)"
        ))?;
        let orphans: Vec<i64> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        if !orphans.is_empty() {
            let orphan_str = orphans
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");
            conn.execute(
                &format!("DELETE FROM extraction_runs WHERE raw_file_id IN ({orphan_str})"),
                [],
            )?;
            conn.execute(
                &format!("DELETE FROM import_jobs WHERE raw_file_id IN ({orphan_str})"),
                [],
            )?;
            conn.execute(
                &format!("DELETE FROM raw_files WHERE id IN ({orphan_str})"),
                [],
            )?;
        }
    }
    Ok(count)
}

// ---- Invoice Merging ----

#[derive(Debug, Clone, Serialize)]
pub struct MergeInvoicesResult {
    pub merged_invoice: InvoiceSummary,
    pub merged_from_ids: Vec<i64>,
    pub total_items_merged: usize,
}

pub fn merge_invoices(
    conn: &mut Connection,
    target_invoice_id: i64,
    source_invoice_ids: Vec<i64>,
) -> Result<MergeInvoicesResult, ExtractorError> {
    if source_invoice_ids.is_empty() {
        return Err(ExtractorError::MergeError(
            "至少需要选择一张要合并的发票".into(),
        ));
    }

    let mut all_ids = source_invoice_ids.clone();
    all_ids.push(target_invoice_id);
    all_ids.sort();
    all_ids.dedup();

    if all_ids.len() < 2 {
        return Err(ExtractorError::MergeError(
            "至少需要两张发票才能合并".into(),
        ));
    }

    if source_invoice_ids.contains(&target_invoice_id) {
        return Err(ExtractorError::MergeError(
            "目标发票不能在合并来源列表中".into(),
        ));
    }

    // Validate all invoices exist and belong to the same raw_file_id
    let target_raw_file_id: i64 = conn
        .query_row(
            "SELECT raw_file_id FROM invoices WHERE id = ?1",
            [target_invoice_id],
            |row| row.get(0),
        )
        .map_err(|_| ExtractorError::NotFound(target_invoice_id))?;

    for &source_id in &source_invoice_ids {
        let source_raw_file_id: i64 = conn
            .query_row(
                "SELECT raw_file_id FROM invoices WHERE id = ?1",
                [source_id],
                |row| row.get(0),
            )
            .map_err(|_| ExtractorError::NotFound(source_id))?;

        if source_raw_file_id != target_raw_file_id {
            return Err(ExtractorError::MergeError(format!(
                "发票 {} 和 {} 来自不同的文件，无法合并",
                target_invoice_id, source_id
            )));
        }
    }

    let tx = conn.transaction()?;

    // Collect page ranges from all invoices to compute merged range
    let mut all_page_numbers: Vec<usize> = Vec::new();

    // Get target page range
    let target_page_range: Option<String> = tx.query_row(
        "SELECT source_page_range FROM invoices WHERE id = ?1",
        [target_invoice_id],
        |row| row.get(0),
    )?;
    if let Some(ref pr) = target_page_range {
        for p in parse_page_range(pr) {
            if !all_page_numbers.contains(&p) {
                all_page_numbers.push(p);
            }
        }
    }

    // Get source page ranges and collect items count
    let mut total_items_merged: usize = 0;
    for &source_id in &source_invoice_ids {
        let source_page_range: Option<String> = tx.query_row(
            "SELECT source_page_range FROM invoices WHERE id = ?1",
            [source_id],
            |row| row.get(0),
        )?;
        if let Some(ref pr) = source_page_range {
            for p in parse_page_range(pr) {
                if !all_page_numbers.contains(&p) {
                    all_page_numbers.push(p);
                }
            }
        }

        // Count items being merged
        let item_count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM invoice_items WHERE invoice_id = ?1",
            [source_id],
            |row| row.get(0),
        )?;
        total_items_merged += item_count as usize;
    }

    // Merge page range
    all_page_numbers.sort();
    let merged_page_range = if all_page_numbers.is_empty() {
        None
    } else if all_page_numbers.len() == 1 {
        Some(all_page_numbers[0].to_string())
    } else {
        let min_page = all_page_numbers[0];
        let max_page = all_page_numbers[all_page_numbers.len() - 1];
        Some(format!("{}-{}", min_page, max_page))
    };

    // Sum amount fields
    let sum_amount = |col: &str| -> Result<Option<String>, ExtractorError> {
        let sql = format!(
            "SELECT SUM(CASE WHEN {} IS NULL OR {} = '' THEN 0 ELSE CAST({} AS REAL) END)
             FROM invoices WHERE id IN ({})",
            col,
            col,
            col,
            all_ids
                .iter()
                .map(|_| "?".to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        let refs: Vec<&dyn rusqlite::types::ToSql> = all_ids.iter().map(|id| id as _).collect();
        let total: Option<f64> = tx.query_row(&sql, refs.as_slice(), |row| row.get(0))?;
        Ok(total.map(|v| format!("{:.2}", v)))
    };

    let merged_total_amount = sum_amount("total_amount")?;
    let merged_amount_without_tax = sum_amount("amount_without_tax")?;
    let merged_tax_amount = sum_amount("tax_amount")?;

    // Move items from source invoices to target
    tx.execute(
        &format!(
            "UPDATE invoice_items SET invoice_id = ?1 WHERE invoice_id IN ({})",
            source_invoice_ids
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",")
        ),
        rusqlite::params_from_iter(
            std::iter::once(&target_invoice_id as &dyn rusqlite::types::ToSql)
                .chain(source_invoice_ids.iter().map(|id| id as _)),
        ),
    )?;

    // Update target invoice
    tx.execute(
        "UPDATE invoices SET
            source_page_range = ?1,
            total_amount = COALESCE(?2, total_amount),
            amount_without_tax = COALESCE(?3, amount_without_tax),
            tax_amount = COALESCE(?4, tax_amount),
            updated_at = CURRENT_TIMESTAMP
         WHERE id = ?5",
        rusqlite::params![
            merged_page_range,
            merged_total_amount,
            merged_amount_without_tax,
            merged_tax_amount,
            target_invoice_id,
        ],
    )?;

    // Delete extraction_runs for source invoices
    tx.execute(
        &format!(
            "DELETE FROM extraction_runs WHERE invoice_id IN ({})",
            source_invoice_ids
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",")
        ),
        rusqlite::params_from_iter(source_invoice_ids.iter()),
    )?;

    // Delete dedupe_candidates referencing source invoices
    tx.execute(
        &format!(
            "DELETE FROM dedupe_candidates WHERE invoice_id IN ({0}) OR candidate_invoice_id IN ({0})",
            source_invoice_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",")
        ),
        rusqlite::params_from_iter(source_invoice_ids.iter()),
    )?;

    // Delete source invoices
    tx.execute(
        &format!(
            "DELETE FROM invoices WHERE id IN ({})",
            source_invoice_ids
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",")
        ),
        rusqlite::params_from_iter(source_invoice_ids.iter()),
    )?;

    tx.commit()?;

    let merged_invoice = load_invoice_summary(conn, target_invoice_id)?;
    Ok(MergeInvoicesResult {
        merged_invoice,
        merged_from_ids: all_ids,
        total_items_merged,
    })
}

fn parse_page_range(range: &str) -> Vec<usize> {
    let range = range.trim();
    if let Some((start, end)) = range.split_once('-') {
        if let (Ok(s), Ok(e)) = (start.parse::<usize>(), end.parse::<usize>()) {
            return (s..=e).collect();
        }
    }
    range.parse::<usize>().ok().into_iter().collect()
}

// ---- Tag Options (lightweight) ----

#[derive(Debug, Clone, Serialize)]
pub struct TagOption {
    pub label: String,
    pub count: i64,
}

pub fn get_tag_options(conn: &Connection) -> Result<Vec<TagOption>, ExtractorError> {
    let mut stmt = conn.prepare(
        "SELECT label, SUM(cnt) as total FROM (
                SELECT COALESCE(category, '') AS label, COUNT(*) AS cnt
                FROM invoices WHERE category IS NOT NULL AND category != ''
                GROUP BY category
                UNION ALL
                SELECT
                    CASE
                        WHEN invoice_type LIKE '%专用%' THEN '专票'
                        WHEN invoice_type LIKE '%普通%' THEN '普票'
                        ELSE invoice_type
                    END AS label,
                    COUNT(*) AS cnt
                FROM invoices WHERE invoice_type IS NOT NULL AND invoice_type != ''
                GROUP BY label
                UNION ALL
                SELECT value AS label, COUNT(DISTINCT invoice_id) AS cnt
                FROM invoice_badges
                GROUP BY value
                UNION ALL
                SELECT 'PDF' AS label, COUNT(DISTINCT inv.id) AS cnt
                FROM invoices inv JOIN raw_files rf ON rf.id = inv.raw_file_id
                WHERE rf.mime_type = 'application/pdf'
                UNION ALL
                SELECT '图片' AS label, COUNT(DISTINCT inv.id) AS cnt
                FROM invoices inv JOIN raw_files rf ON rf.id = inv.raw_file_id
                WHERE rf.mime_type LIKE 'image/%'
                UNION ALL
                SELECT '文件' AS label, COUNT(DISTINCT inv.id) AS cnt
                FROM invoices inv JOIN raw_files rf ON rf.id = inv.raw_file_id
                WHERE rf.mime_type IS NOT NULL
                    AND rf.mime_type != 'application/pdf'
                    AND rf.mime_type NOT LIKE 'image/%'
            ) WHERE label != ''
            GROUP BY label
            ORDER BY total DESC
            LIMIT 30",
    )?;

    let options = stmt
        .query_map([], |row| {
            Ok(TagOption {
                label: row.get(0)?,
                count: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(options)
}

pub fn insert_usage_log(
    conn: &Connection,
    operation: &str,
    model: &str,
    prompt_tokens: i64,
    completion_tokens: i64,
    total_tokens: i64,
) -> Result<(), ExtractorError> {
    conn.execute(
        "INSERT INTO usage_log (operation, model, prompt_tokens, completion_tokens, total_tokens)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            operation,
            model,
            prompt_tokens,
            completion_tokens,
            total_tokens
        ],
    )?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct LlmUsageStats {
    pub total_calls: i64,
    pub llm_calls: i64,
    pub embedding_calls: i64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub total_tokens: i64,
    pub this_month_calls: i64,
    pub this_month_tokens: i64,
}

pub fn get_llm_usage(
    conn: &Connection,
    date_from: Option<&str>,
    date_to: Option<&str>,
) -> Result<LlmUsageStats, ExtractorError> {
    // Consolidated: total_calls, llm_calls, embedding_calls, tokens
    // merged from 6 separate queries into 1 using conditional aggregation
    let mut clauses: Vec<String> = Vec::new();
    let mut params: Vec<String> = Vec::new();

    if let Some(from) = date_from {
        clauses.push(format!("created_at >= ?{}", params.len() + 1));
        params.push(from.to_owned());
    }
    if let Some(to) = date_to {
        clauses.push(format!("created_at <= ?{}", params.len() + 1));
        params.push(format!("{to} 23:59:59"));
    }

    let where_clause = if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    };

    let (total_calls, llm_calls, embedding_calls, total_prompt_tokens, total_completion_tokens, total_tokens): (i64, i64, i64, i64, i64, i64) = conn.query_row(
        &format!(
            "SELECT
                COALESCE(COUNT(*), 0),
                COALESCE(SUM(CASE WHEN operation = 'llm_recognition' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN operation = 'embedding' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(prompt_tokens), 0),
                COALESCE(SUM(completion_tokens), 0),
                COALESCE(SUM(total_tokens), 0)
             FROM usage_log{where_clause}"
        ),
        rusqlite::params_from_iter(params.iter()),
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
    )?;

    // Consolidated: this_month_calls + this_month_tokens merged into 1 query
    let this_month_start = chrono::Local::now().format("%Y-%m-01").to_string();
    let (this_month_calls, this_month_tokens): (i64, i64) = conn.query_row(
        "SELECT COALESCE(COUNT(*), 0), COALESCE(SUM(total_tokens), 0)
         FROM usage_log WHERE created_at >= ?1",
        [&this_month_start],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    Ok(LlmUsageStats {
        total_calls,
        llm_calls,
        embedding_calls,
        total_prompt_tokens,
        total_completion_tokens,
        total_tokens,
        this_month_calls,
        this_month_tokens,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::run_migrations;

    #[test]
    fn saves_valid_extraction_to_invoice_tables() {
        let mut conn = Connection::open_in_memory().expect("open sqlite");
        run_migrations(&mut conn).expect("migrate");
        let raw_file_id = insert_test_raw_file(&conn);

        let invoice = save_invoice_extraction(
            &mut conn,
            SaveInvoiceExtractionRequest {
                raw_file_id,
                source_page_range: Some("1".into()),
                provider_name: Some("test-provider".into()),
                model: Some("vision-model".into()),
                response_json: r#"{
                    "is_invoice": true,
                    "invoice_type": "增值税电子普通发票",
                    "invoice_code": "044002300111",
                    "invoice_number": "12345678",
                    "issue_date": "2026-04-30",
                    "seller": {"name": "测试销售方", "tax_id": "SELLER-TAX"},
                    "buyer": {"name": "测试购买方", "tax_id": "BUYER-TAX"},
                    "currency": "CNY",
                    "amount_without_tax": 100.25,
                    "tax_amount": "6.02",
                    "total_amount": 106.27,
                    "category": "办公",
                    "items": [
                        {"name": "纸张", "quantity": 1, "unit_price": "100.25", "amount": 100.25}
                    ],
                    "remarks": "测试备注",
                    "confidence": 0.96,
                    "needs_review": false
                }"#
                .into(),
            },
        )
        .expect("save extraction");

        assert_eq!(invoice.raw_file_id, raw_file_id);
        assert_eq!(invoice.invoice_number.as_deref(), Some("12345678"));
        assert_eq!(invoice.seller_name.as_deref(), Some("测试销售方"));
        assert_eq!(invoice.total_amount.as_deref(), Some("106.27"));
        assert_eq!(invoice.status, "recognized");

        let item_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM invoice_items", [], |row| row.get(0))
            .expect("count items");
        assert_eq!(item_count, 1);
    }

    #[test]
    fn rejects_non_invoice_extraction() {
        let err = parse_invoice_extraction_json(r#"{"is_invoice": false}"#)
            .expect_err("non invoice should fail");

        assert!(matches!(err, ExtractorError::NonInvoice));
    }

    #[test]
    fn rejects_invalid_issue_date() {
        let err = parse_invoice_extraction_json(
            r#"{
                "is_invoice": true,
                "issue_date": "04/30/2026"
            }"#,
        )
        .expect_err("invalid issue date should fail");

        assert!(matches!(err, ExtractorError::InvalidIssueDate(_)));
    }

    #[test]
    fn dashboard_stats_empty() {
        let mut conn = Connection::open_in_memory().expect("open sqlite");
        run_migrations(&mut conn).expect("migrate");

        let stats = get_dashboard_stats(&conn, None, None).expect("get stats");
        assert_eq!(stats.total_invoices, 0);
        assert_eq!(stats.total_amount, 0.0);
        assert_eq!(stats.this_month_count, 0);
        assert_eq!(stats.pending_count, 0);
        assert_eq!(stats.duplicate_count, 0);
        assert!(stats.monthly_trend.is_empty());
        assert!(stats.by_type.is_empty());
        assert!(stats.by_status.is_empty());
        assert!(stats.top_sellers.is_empty());
    }

    #[test]
    fn dashboard_stats_with_data() {
        let mut conn = Connection::open_in_memory().expect("open sqlite");
        run_migrations(&mut conn).expect("migrate");
        let raw = insert_test_raw_file(&conn);

        // Insert two invoices
        save_invoice_extraction(
            &mut conn,
            SaveInvoiceExtractionRequest {
                raw_file_id: raw,
                source_page_range: None,
                provider_name: None,
                model: None,
                response_json: r#"{
                    "is_invoice": true,
                    "invoice_type": "增值税电子普通发票",
                    "invoice_code": "C1",
                    "invoice_number": "N1",
                    "issue_date": "2026-05-01",
                    "seller": {"name": "SellerA"},
                    "buyer": {"name": "BuyerA"},
                    "total_amount": 100.0,
                    "confidence": 0.9,
                    "needs_review": false
                }"#
                .into(),
            },
        )
        .expect("save 1");

        save_invoice_extraction(
            &mut conn,
            SaveInvoiceExtractionRequest {
                raw_file_id: raw,
                source_page_range: None,
                provider_name: None,
                model: None,
                response_json: r#"{
                    "is_invoice": true,
                    "invoice_type": "增值税专用发票",
                    "invoice_code": "C2",
                    "invoice_number": "N2",
                    "issue_date": "2026-04-15",
                    "seller": {"name": "SellerB"},
                    "buyer": {"name": "BuyerB"},
                    "total_amount": 200.0,
                    "confidence": 0.9,
                    "needs_review": false
                }"#
                .into(),
            },
        )
        .expect("save 2");

        // Set one to pending_confirmation
        conn.execute(
            "UPDATE invoices SET status = 'pending_confirmation' WHERE invoice_number = 'N2'",
            [],
        )
        .expect("update status");

        let stats = get_dashboard_stats(&conn, None, None).expect("get stats");
        assert_eq!(stats.total_invoices, 2);
        assert_eq!(stats.total_amount, 300.0);
        assert_eq!(stats.pending_count, 1);
        assert_eq!(stats.by_type.len(), 2);
        assert_eq!(stats.by_status.len(), 2);
        assert_eq!(stats.top_sellers.len(), 2);
        // Monthly trend: 2 invoices in different months
        assert_eq!(stats.monthly_trend.len(), 2);
        assert_eq!(stats.monthly_trend[0].month, "2026-04");
        assert_eq!(stats.monthly_trend[0].count, 1);
        assert_eq!(stats.monthly_trend[1].month, "2026-05");
        assert_eq!(stats.monthly_trend[1].count, 1);
    }

    fn insert_test_raw_file(conn: &Connection) -> i64 {
        conn.execute(
            "INSERT INTO raw_files (
                sha256,
                md5,
                original_name,
                current_name,
                extension,
                mime_type,
                byte_size,
                storage_path
            ) VALUES ('sha', 'md5', 'invoice.jpg', 'invoice.jpg', 'jpg', 'image/jpeg', 10, '/tmp/invoice.jpg')",
            [],
        )
        .expect("insert raw file");

        conn.last_insert_rowid()
    }
}
