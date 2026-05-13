use std::path::Path;

use scnetocr::{InvoiceElements, OcrClient, OcrType};
use serde_json::{json, Value};
use tracing::{info, warn};

/// Call SCNet OCR to recognize a VAT invoice image.
/// Returns the structured extraction JSON string in project-internal format.
/// Returns None if no results found (not an invoice).
pub async fn recognize_with_scnet(
    api_key: &str,
    image_path: &Path,
) -> Result<Option<String>, String> {
    let client = OcrClient::new(api_key);
    let response = client
        .recognize(image_path, OcrType::VatInvoice)
        .await
        .map_err(|e| format!("SCNet OCR error: {e}"))?;

    for data_item in &response.data {
        for result_item in &data_item.result {
            if result_item.status != 200 {
                continue;
            }
            let elements: InvoiceElements = result_item
                .elements_as()
                .map_err(|e| format!("SCNet parse error: {e}"))?;
            let extraction = map_scnet_to_extraction_json(&elements);
            let json_str = serde_json::to_string(&extraction)
                .map_err(|e| format!("JSON serialization error: {e}"))?;
            info!(
                "SCNet OCR recognized invoice: code={:?}, number={:?}, amount={:?}",
                elements.invoice_code, elements.invoice_no, elements.total_amount_lower
            );
            return Ok(Some(json_str));
        }
    }

    info!("SCNet OCR returned no invoice results");
    Ok(None)
}

/// Map SCNet InvoiceElements to project-internal extraction JSON format.
fn map_scnet_to_extraction_json(elements: &InvoiceElements) -> Value {
    let items = elements
        .goods_details
        .as_ref()
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let name = item.get("goodsName").and_then(|v| v.as_str());
                    let amount = item.get("itemAmount").and_then(|v| v.as_str());
                    if name.is_none() && amount.is_none() {
                        return None;
                    }
                    Some(json!({
                        "name": name,
                        "spec": item.get("specification").and_then(|v| v.as_str()),
                        "unit": item.get("unit").and_then(|v| v.as_str()),
                        "quantity": item.get("quantity").and_then(|v| v.as_str()),
                        "unit_price": item.get("unitPrice").and_then(|v| v.as_str()),
                        "amount": amount,
                        "tax_rate": item.get("taxRate").and_then(|v| v.as_str()),
                        "tax_amount": item.get("taxAmount").and_then(|v| v.as_str()),
                    }))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    json!({
        "is_invoice": true,
        "invoice_type": elements.title,
        "invoice_code": elements.invoice_code,
        "invoice_number": elements.invoice_no,
        "issue_date": normalize_date(elements.invoice_date.as_deref()),
        "seller": {
            "name": elements.seller_name,
            "tax_id": elements.seller_code,
        },
        "buyer": {
            "name": elements.buyer_name,
            "tax_id": elements.buyer_code,
        },
        "currency": "CNY",
        "amount_without_tax": elements.pre_tax_total_amount,
        "tax_amount": elements.total_tax_amount,
        "total_amount": elements.total_amount_lower,
        "category": null,
        "items": items,
        "remarks": elements.remarks,
        "extra_fields": {},
        "confidence": 0.95,
        "needs_review": false,
        "warnings": [],
    })
}

/// Try to normalize a date string to YYYY-MM-DD format.
/// SCNet returns dates in various formats like "2026年04月30日" or "20260430" or "2026-04-30".
fn normalize_date(date: Option<&str>) -> Option<String> {
    let s = date?.trim();
    if s.is_empty() {
        return None;
    }
    // Already YYYY-MM-DD
    if s.len() == 10 && s.as_bytes()[4] == b'-' && s.as_bytes()[7] == b'-' {
        return Some(s.to_string());
    }
    // "2026年04月30日" → "2026-04-30"
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() == 8 {
        return Some(format!(
            "{}-{}-{}",
            &digits[0..4],
            &digits[4..6],
            &digits[6..8]
        ));
    }
    Some(s.to_string())
}

/// Merge VLM recognition result with SCNet OCR result.
/// VLM is the base; SCNet high-precision fields override/complete VLM.
/// Returns merged JSON string.
pub fn merge_vlm_and_scnet(vlm_json: &str, scnet_json: &str) -> String {
    let mut vlm: Value = match serde_json::from_str(vlm_json) {
        Ok(v) => v,
        Err(e) => {
            warn!("Failed to parse VLM JSON for merge: {e}");
            return vlm_json.to_string();
        }
    };
    let scnet: Value = match serde_json::from_str(scnet_json) {
        Ok(v) => v,
        Err(e) => {
            warn!("Failed to parse SCNet JSON for merge: {e}");
            return vlm_json.to_string();
        }
    };

    // SCNet overrides for high-precision fields
    override_field(&mut vlm, &scnet, "invoice_code");
    override_field(&mut vlm, &scnet, "invoice_number");
    override_field(&mut vlm, &scnet, "issue_date");
    override_field(&mut vlm, &scnet, "total_amount");
    override_field(&mut vlm, &scnet, "amount_without_tax");
    override_field(&mut vlm, &scnet, "tax_amount");

    // Merge seller/buyer: SCNet overrides if VLM has null
    override_nested_field(&mut vlm, &scnet, "seller", "name");
    override_nested_field(&mut vlm, &scnet, "seller", "tax_id");
    override_nested_field(&mut vlm, &scnet, "buyer", "name");
    override_nested_field(&mut vlm, &scnet, "buyer", "tax_id");

    // Items: use SCNet items if VLM has none, otherwise keep VLM
    if let Some(vlm_items) = vlm.get("items").and_then(|v| v.as_array()) {
        if vlm_items.is_empty() {
            if let Some(scnet_items) = scnet.get("items").and_then(|v| v.as_array()) {
                if !scnet_items.is_empty() {
                    if let Some(obj) = vlm.as_object_mut() {
                        obj.insert("items".to_string(), json!(scnet_items));
                    }
                }
            }
        }
    }

    // Confidence: take the higher value
    let vlm_conf = vlm.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let scnet_conf = scnet.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.0);
    if scnet_conf > vlm_conf {
        if let Some(obj) = vlm.as_object_mut() {
            obj.insert("confidence".to_string(), json!(scnet_conf));
        }
    }

    // Add warning that SCNet was used for cross-validation
    if let Some(obj) = vlm.as_object_mut() {
        let warnings = obj
            .entry("warnings")
            .or_insert_with(|| json!([]));
        if let Some(arr) = warnings.as_array_mut() {
            arr.push(json!("scnet_ocr_cross_validated"));
        }
    }

    serde_json::to_string(&vlm).unwrap_or_else(|_| vlm_json.to_string())
}

/// Override a top-level field in `target` from `source` if target has null/empty
/// and source has a non-null value.
fn override_field(target: &mut Value, source: &Value, field: &str) {
    let target_val = target.get(field);
    let source_val = source.get(field);

    let should_override = match target_val {
        None => true,
        Some(Value::Null) => source_val.is_some() && source_val != Some(&Value::Null),
        Some(Value::String(s)) if s.is_empty() => {
            source_val.is_some() && source_val != Some(&Value::Null)
        }
        _ => false,
    };

    if should_override {
        if let (Some(obj), Some(val)) = (target.as_object_mut(), source_val) {
            obj.insert(field.to_string(), val.clone());
        }
    }
}

/// Override a nested field like `seller.name` in `target` from `source`.
fn override_nested_field(target: &mut Value, source: &Value, parent: &str, field: &str) {
    let source_val = source
        .get(parent)
        .and_then(|p| p.get(field))
        .cloned();

    let target_val = target
        .get(parent)
        .and_then(|p| p.get(field));

    let should_override = match target_val {
        None => source_val.is_some() && source_val != Some(Value::Null),
        Some(Value::Null) => source_val.is_some() && source_val != Some(Value::Null),
        Some(Value::String(s)) if s.is_empty() => {
            source_val.is_some() && source_val != Some(Value::Null)
        }
        _ => false,
    };

    if should_override {
        if let Some(val) = source_val {
            if let Some(target_parent) = target.get_mut(parent).and_then(|v| v.as_object_mut()) {
                target_parent.insert(field.to_string(), val);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_fills_missing_fields_from_scnet() {
        let vlm = r#"{"is_invoice":true,"invoice_type":"增值税电子普通发票","invoice_code":null,"invoice_number":"2651200001808075851","issue_date":"2026-04-30","seller":{"name":"成都明和盛","tax_id":null},"buyer":{"name":null,"tax_id":null},"currency":"CNY","amount_without_tax":"123.89","tax_amount":"6.11","total_amount":"130.0","category":null,"items":[],"remarks":null,"extra_fields":{},"confidence":0.8,"needs_review":true,"warnings":[]}"#;
        let scnet = r#"{"is_invoice":true,"invoice_type":"增值税电子普通发票","invoice_code":"051002100311","invoice_number":"2651200001808075851","issue_date":"2026-04-30","seller":{"name":"成都明和盛信息技术有限公司","tax_id":"91510100MA6CXQXXXX"},"buyer":{"name":"测试公司","tax_id":"91110000XXXXX"},"currency":"CNY","amount_without_tax":"123.89","tax_amount":"6.11","total_amount":"130.0","category":null,"items":[],"remarks":null,"extra_fields":{},"confidence":0.95,"needs_review":false,"warnings":[]}"#;

        let merged = merge_vlm_and_scnet(vlm, scnet);
        let result: Value = serde_json::from_str(&merged).unwrap();

        // SCNet should fill in missing fields
        assert_eq!(result["invoice_code"], "051002100311");
        assert_eq!(result["seller"]["tax_id"], "91510100MA6CXQXXXX");
        assert_eq!(result["buyer"]["name"], "测试公司");
        // VLM already had this, should NOT be overridden
        assert_eq!(result["invoice_number"], "2651200001808075851");
        assert_eq!(result["seller"]["name"], "成都明和盛");
        // Confidence should take higher (SCNet)
        assert_eq!(result["confidence"], 0.95);
    }

    #[test]
    fn merge_keeps_vlm_when_scnet_fails() {
        let vlm = r#"{"is_invoice":true,"invoice_type":"增值税电子普通发票","invoice_code":"051002100311","invoice_number":"2651200001808075851","issue_date":"2026-04-30","seller":{"name":"成都明和盛","tax_id":"91510100MA6CXQXXXX"},"buyer":{"name":"测试公司","tax_id":"91110000XXXXX"},"currency":"CNY","amount_without_tax":"123.89","tax_amount":"6.11","total_amount":"130.0","category":null,"items":[],"remarks":null,"extra_fields":{},"confidence":0.8,"needs_review":true,"warnings":[]}"#;
        let scnet = r#"{"is_invoice":true,"invoice_type":null,"invoice_code":null,"invoice_number":null,"issue_date":null,"seller":{"name":null,"tax_id":null},"buyer":{"name":null,"tax_id":null},"currency":"CNY","amount_without_tax":null,"tax_amount":null,"total_amount":null,"category":null,"items":[],"remarks":null,"extra_fields":{},"confidence":0.0,"needs_review":false,"warnings":[]}"#;

        let merged = merge_vlm_and_scnet(vlm, scnet);
        let result: Value = serde_json::from_str(&merged).unwrap();

        // VLM fields should be preserved
        assert_eq!(result["invoice_code"], "051002100311");
        assert_eq!(result["invoice_number"], "2651200001808075851");
        assert_eq!(result["seller"]["name"], "成都明和盛");
    }

    #[test]
    fn normalize_date_handles_chinese_format() {
        assert_eq!(normalize_date(Some("2026年04月30日")), Some("2026-04-30".to_string()));
        assert_eq!(normalize_date(Some("20260430")), Some("2026-04-30".to_string()));
        assert_eq!(normalize_date(Some("2026-04-30")), Some("2026-04-30".to_string()));
        assert_eq!(normalize_date(None), None);
        assert_eq!(normalize_date(Some("")), None);
    }
}
