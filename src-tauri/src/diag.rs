use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::embedding::{test_embedding_connection as run_embedding_test, EmbeddingConfig};
use crate::llm::{
    recognize_invoice_image, test_llm_connection as run_llm_connection_test, LlmAuditConfig,
    LlmProviderConfig,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundTruth {
    pub invoice_type: Option<String>,
    pub invoice_code: Option<String>,
    pub invoice_number: Option<String>,
    pub issue_date: Option<String>,
    pub seller_name: Option<String>,
    pub buyer_name: Option<String>,
    pub total_amount: Option<f64>,
    pub amount_without_tax: Option<f64>,
    pub tax_amount: Option<f64>,
    pub items_count: Option<usize>,
}

impl Default for GroundTruth {
    fn default() -> Self {
        Self {
            invoice_type: None,
            invoice_code: None,
            invoice_number: None,
            issue_date: None,
            seller_name: None,
            buyer_name: None,
            total_amount: None,
            amount_without_tax: None,
            tax_amount: None,
            items_count: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticConfig {
    pub test_image_path: String,
    pub ground_truth: GroundTruth,
    pub enabled: bool,
}

impl Default for DiagnosticConfig {
    fn default() -> Self {
        Self {
            test_image_path: String::new(),
            ground_truth: GroundTruth::default(),
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticStep {
    pub name: String,
    pub passed: bool,
    pub duration_ms: u128,
    pub message: String,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticResult {
    pub steps: Vec<DiagnosticStep>,
    pub score: Option<f64>,
    pub all_passed: bool,
}

pub fn load_config(app_data_dir: &Path, resource_dir: Option<&Path>) -> DiagnosticConfig {
    let path = app_data_dir.join("diagnostic_config.json");
    if let Ok(json) = std::fs::read_to_string(&path) {
        if let Ok(config) = serde_json::from_str::<DiagnosticConfig>(&json) {
            return config;
        }
    }
    // First run: try to copy bundled diagnostic_config.json from resource_dir
    if let Some(res_dir) = resource_dir {
        // In dev mode resource_dir is target/debug/ but resources are in target/debug/_up_/
        let base_dir = if res_dir.join("_up_").is_dir() {
            res_dir.join("_up_")
        } else {
            res_dir.to_path_buf()
        };
        let bundled_config = base_dir.join("sample").join("diagnostic_config.json");
        if bundled_config.exists() {
            if let Ok(json) = std::fs::read_to_string(&bundled_config) {
                if let Ok(mut config) = serde_json::from_str::<DiagnosticConfig>(&json) {
                    // Resolve test_image_path relative to resource_dir
                    if !config.test_image_path.is_empty() && !Path::new(&config.test_image_path).is_absolute() {
                        let resolved = base_dir.join(&config.test_image_path);
                        if resolved.exists() {
                            config.test_image_path = resolved.to_string_lossy().into_owned();
                        }
                    }
                    let _ = save_config(app_data_dir, &config);
                    return config;
                }
            }
        }
    }
    let config = DiagnosticConfig::default();
    let _ = save_config(app_data_dir, &config);
    config
}

pub fn save_config(app_data_dir: &Path, config: &DiagnosticConfig) -> std::io::Result<()> {
    let path = app_data_dir.join("diagnostic_config.json");
    let json = serde_json::to_string_pretty(config)?;
    std::fs::write(path, json)
}

pub async fn run_diagnostic(
    diag_config: &DiagnosticConfig,
    llm_config: &LlmProviderConfig,
    embedding_config: Option<&EmbeddingConfig>,
    audit: Option<&LlmAuditConfig>,
) -> DiagnosticResult {
    let mut steps = Vec::new();
    let mut score: Option<f64> = None;

    // Step 1: Text generation
    let step1 = {
        let start = Instant::now();
        match run_llm_connection_test(llm_config.clone(), audit).await {
            Ok(result) => DiagnosticStep {
                name: "文本生成".into(),
                passed: true,
                duration_ms: start.elapsed().as_millis(),
                message: format!("成功，响应: {}", result.response_preview),
                details: None,
            },
            Err(err) => DiagnosticStep {
                name: "文本生成".into(),
                passed: false,
                duration_ms: start.elapsed().as_millis(),
                message: format!("失败: {err}"),
                details: None,
            },
        }
    };
    steps.push(step1);

    // Step 2: Image recognition
    let mut recognition_json: Option<serde_json::Value> = None;
    let test_image_path = PathBuf::from(&diag_config.test_image_path);
    let step2 = if test_image_path.exists() {
        let mime = infer_mime(&test_image_path);
        let start = Instant::now();
        info!("Diagnostic: sending test image to recognition");
        match recognize_invoice_image(llm_config.clone(), &test_image_path, &mime, audit).await {
            Ok(result) => {
                let parsed = serde_json::from_str::<serde_json::Value>(&result.response_json);
                match parsed {
                    Ok(val) => {
                        let is_invoice = val
                            .get("is_invoice")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        recognition_json = Some(val);
                        DiagnosticStep {
                            name: "图片识别".into(),
                            passed: is_invoice,
                            duration_ms: start.elapsed().as_millis(),
                            message: if is_invoice {
                                "识别为有效发票".into()
                            } else {
                                "未识别为发票".into()
                            },
                            details: Some(result.response_preview),
                        }
                    }
                    Err(err) => DiagnosticStep {
                        name: "图片识别".into(),
                        passed: false,
                        duration_ms: start.elapsed().as_millis(),
                        message: format!("识别结果 JSON 解析失败: {err}"),
                        details: Some(result.response_preview),
                    },
                }
            }
            Err(err) => DiagnosticStep {
                name: "图片识别".into(),
                passed: false,
                duration_ms: start.elapsed().as_millis(),
                message: format!("识别失败: {err}"),
                details: None,
            },
        }
    } else {
        DiagnosticStep {
            name: "图片识别".into(),
            passed: false,
            duration_ms: 0,
            message: format!("测试图片不存在: {}", diag_config.test_image_path),
            details: None,
        }
    };
    steps.push(step2);

    // Step 3: Ground truth comparison
    let gt = &diag_config.ground_truth;
    let has_any_gt = gt.invoice_type.is_some()
        || gt.seller_name.is_some()
        || gt.buyer_name.is_some()
        || gt.total_amount.is_some()
        || gt.amount_without_tax.is_some()
        || gt.tax_amount.is_some()
        || gt.items_count.is_some();

    let step3 = if let Some(ref val) = recognition_json {
        if has_any_gt {
            let (matched, total, details) = compare_ground_truth(gt, val);
            let s = if total > 0 {
                (matched as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            score = Some(s);
            DiagnosticStep {
                name: "结果对比".into(),
                passed: s >= 50.0,
                duration_ms: 0,
                message: format!("匹配 {matched}/{total} 字段，得分 {s:.0}%"),
                details: Some(details),
            }
        } else {
            DiagnosticStep {
                name: "结果对比".into(),
                passed: true,
                duration_ms: 0,
                message: "未配置 ground truth，跳过对比".into(),
                details: None,
            }
        }
    } else {
        DiagnosticStep {
            name: "结果对比".into(),
            passed: false,
            duration_ms: 0,
            message: "上一步未获得识别结果，跳过对比".into(),
            details: None,
        }
    };
    steps.push(step3);

    // Step 4: Embedding (optional)
    let step4 = if let Some(emb_config) = embedding_config {
        if emb_config.enabled && !emb_config.base_url.is_empty() && !emb_config.api_key.is_empty()
        {
            let start = Instant::now();
            match run_embedding_test(emb_config).await {
                Ok(result) => DiagnosticStep {
                    name: "Embedding".into(),
                    passed: true,
                    duration_ms: start.elapsed().as_millis(),
                    message: format!(
                        "模型: {}，维度: {}，耗时: {}ms",
                        result.model, result.dimensions, result.duration_ms
                    ),
                    details: None,
                },
                Err(err) => DiagnosticStep {
                    name: "Embedding".into(),
                    passed: false,
                    duration_ms: start.elapsed().as_millis(),
                    message: format!("失败: {err}"),
                    details: None,
                },
            }
        } else {
            DiagnosticStep {
                name: "Embedding".into(),
                passed: true,
                duration_ms: 0,
                message: "未配置 Embedding，跳过".into(),
                details: None,
            }
        }
    } else {
        DiagnosticStep {
            name: "Embedding".into(),
            passed: true,
            duration_ms: 0,
            message: "未配置 Embedding，跳过".into(),
            details: None,
        }
    };
    steps.push(step4);

    let all_passed = steps.iter().all(|s| s.passed);
    DiagnosticResult {
        steps,
        score,
        all_passed,
    }
}

fn compare_ground_truth(
    gt: &GroundTruth,
    recognized: &serde_json::Value,
) -> (usize, usize, String) {
    let mut matched = 0usize;
    let mut total = 0usize;
    let mut lines = Vec::new();

    // Helper: extract recognized value from either top-level or nested object
    let get_str = |obj: &serde_json::Value, keys: &[&str]| -> Option<String> {
        for key in keys {
            if let Some(v) = obj.get(key).and_then(|v| v.as_str()) {
                let s = v.trim();
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        }
        // Check nested seller/buyer objects
        if let Some(seller) = obj.get("seller").and_then(|v| v.as_object()) {
            for key in keys {
                if let Some(v) = seller.get(*key).and_then(|v| v.as_str()) {
                    let s = v.trim();
                    if !s.is_empty() {
                        return Some(s.to_string());
                    }
                }
            }
        }
        if let Some(buyer) = obj.get("buyer").and_then(|v| v.as_object()) {
            for key in keys {
                if let Some(v) = buyer.get(*key).and_then(|v| v.as_str()) {
                    let s = v.trim();
                    if !s.is_empty() {
                        return Some(s.to_string());
                    }
                }
            }
        }
        None
    };

    let get_f64 = |obj: &serde_json::Value, key: &str| -> Option<f64> {
        obj.get(key)
            .and_then(|v| v.as_f64())
            .or_else(|| obj.get(key).and_then(|v| v.as_str()).and_then(|s| s.parse().ok()))
    };

    // String fields: contains match
    macro_rules! check_str {
        ($field:ident, $keys:expr) => {
            if let Some(ref expected) = gt.$field {
                total += 1;
                let recognized_val = get_str(recognized, $keys);
                let got = recognized_val.as_deref().unwrap_or("(空)");
                let name = stringify!($field);
                if recognized_val.as_ref().map_or(false, |v| v.contains(expected.as_str()) || expected.contains(v.as_str())) {
                    matched += 1;
                    lines.push(format!("{name}: ✓ ({got})"));
                } else {
                    lines.push(format!("{name}: ✗ (期望: {expected}, 实际: {got})"));
                }
            }
        };
    }

    check_str!(invoice_type, &["invoice_type"]);
    check_str!(invoice_code, &["invoice_code"]);
    check_str!(invoice_number, &["invoice_number"]);
    check_str!(issue_date, &["issue_date"]);
    check_str!(seller_name, &["seller_name", "name"]);
    check_str!(buyer_name, &["buyer_name", "name"]);

    // Numeric fields: ±5% tolerance
    macro_rules! check_amount {
        ($field:ident, $key:expr) => {
            if let Some(expected) = gt.$field {
                total += 1;
                let recognized_val = get_f64(recognized, $key);
                let name = stringify!($field);
                if let Some(got) = recognized_val {
                    let tolerance = expected.abs() * 0.05;
                    if (got - expected).abs() <= tolerance {
                        matched += 1;
                        lines.push(format!("{name}: ✓ ({got:.2})"));
                    } else {
                        lines.push(format!("{name}: ✗ (期望: {expected:.2}, 实际: {got:.2})"));
                    }
                } else {
                    lines.push(format!("{name}: ✗ (期望: {expected:.2}, 实际: (空))"));
                }
            }
        };
    }

    check_amount!(total_amount, "total_amount");
    check_amount!(amount_without_tax, "amount_without_tax");
    check_amount!(tax_amount, "tax_amount");

    // items_count: exact match
    if let Some(expected_count) = gt.items_count {
        total += 1;
        let actual_count = recognized
            .get("items")
            .and_then(|v| v.as_array())
            .map(|arr| arr.len());
        if let Some(got) = actual_count {
            if got == expected_count {
                matched += 1;
                lines.push(format!("items_count: ✓ ({got})"));
            } else {
                lines.push(format!("items_count: ✗ (期望: {expected_count}, 实际: {got})"));
            }
        } else {
            lines.push(format!("items_count: ✗ (期望: {expected_count}, 实际: (空))"));
        }
    }

    (matched, total, lines.join("\n"))
}

fn infer_mime(path: &Path) -> String {
    let lower = path.to_string_lossy().to_lowercase();
    if lower.ends_with(".png") {
        "image/png".into()
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg".into()
    } else {
        "application/octet-stream".into()
    }
}
