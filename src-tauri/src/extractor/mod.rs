use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

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
    #[error("LLM result is marked as non-invoice")]
    NonInvoice,
    #[error("invalid extraction JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("invalid issue date, expected YYYY-MM-DD: {0}")]
    InvalidIssueDate(String),
    #[error("confidence must be between 0 and 1")]
    InvalidConfidence,
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
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
            updated_at
        FROM invoices
        ORDER BY id DESC
        LIMIT 100",
    )?;

    let invoices = stmt
        .query_map([], row_to_invoice_summary)?
        .collect::<Result<Vec<_>, _>>()?;

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
    let status = if extraction.needs_review.unwrap_or(true)
        || extraction
            .confidence
            .is_none_or(|confidence| confidence < 0.85)
    {
        "pending_confirmation"
    } else {
        "recognized"
    };

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
            source_page_range,
            confidence,
            status,
            duplicate_status
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, 'unknown')",
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
            source_page_range,
            extraction.confidence,
            status,
        ],
    )?;

    Ok(conn.last_insert_rowid())
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
    conn.query_row(
        "SELECT
            id,
            raw_file_id,
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
            updated_at
        FROM invoices
        WHERE id = ?1",
        [invoice_id],
        row_to_invoice_summary,
    )
    .map_err(ExtractorError::from)
}

fn row_to_invoice_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<InvoiceSummary> {
    Ok(InvoiceSummary {
        id: row.get(0)?,
        raw_file_id: row.get(1)?,
        invoice_type: row.get(2)?,
        invoice_code: row.get(3)?,
        invoice_number: row.get(4)?,
        issue_date: row.get(5)?,
        seller_name: row.get(6)?,
        buyer_name: row.get(7)?,
        currency: row.get(8)?,
        total_amount: row.get(9)?,
        category: row.get(10)?,
        source_page_range: row.get(11)?,
        confidence: row.get(12)?,
        status: row.get(13)?,
        duplicate_status: row.get(14)?,
        created_at: row.get(15)?,
        updated_at: row.get(16)?,
    })
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
