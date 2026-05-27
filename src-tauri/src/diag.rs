//! 诊断模块：验证 LLM 连接、发票识别准确度和 Embedding 引擎状态。
//!
//! 执行多步诊断（文本生成、图片识别、结果对比、Embedding），
//! 通过 ground truth 比对评估识别质量并输出诊断报告。

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::app_core::constants::DIAGNOSTIC_CONFIG_FILE;
use crate::embedding::EmbeddingTestResult;
use crate::llm::{
    recognize_invoice_with_retries, test_llm_connection as run_llm_connection_test, LlmAuditConfig,
    LlmProviderConfig,
};

/// 诊断用的发票标准答案字段。
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

/// 诊断配置，包含测试图片路径和标准答案。
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

/// 单个诊断步骤的结果。
#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticStep {
    pub name: String,
    pub passed: bool,
    pub duration_ms: u128,
    pub message: String,
    pub details: Option<String>,
}

/// 完整诊断结果，包含各步骤和综合得分。
#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticResult {
    pub steps: Vec<DiagnosticStep>,
    pub score: Option<f64>,
    pub all_passed: bool,
}

/// Embedded test image for diagnostics — avoids dependency on resource bundling
const TEST_IMAGE_BYTES: &[u8] = include_bytes!("../../sample/fake-invoice-1.png");

/// 将内嵌的测试样例文件写入 app_data_dir/sample/（如不存在）。
pub fn ensure_samples(app_data_dir: &Path) {
    let samples_dir = app_data_dir.join("sample");
    if let Err(e) = std::fs::create_dir_all(&samples_dir) {
        warn!("Failed to create samples dir: {e}");
        return;
    }
    let image_path = samples_dir.join("fake-invoice-1.png");
    if !image_path.exists() {
        if let Err(e) = std::fs::write(&image_path, TEST_IMAGE_BYTES) {
            warn!("Failed to write test image: {e}");
        } else {
            info!("Wrote embedded test image to {}", image_path.display());
        }
    }
}

/// 加载诊断配置。若不存在则创建默认配置。
pub fn load_config(app_data_dir: &Path) -> DiagnosticConfig {
    let config_path = app_data_dir.join(DIAGNOSTIC_CONFIG_FILE);
    let samples_dir = app_data_dir.join("sample");

    if let Ok(json) = std::fs::read_to_string(&config_path) {
        if let Ok(mut config) = serde_json::from_str::<DiagnosticConfig>(&json) {
            // Rewrite test_image_path to absolute under app_data_dir
            if !config.test_image_path.is_empty()
                && !Path::new(&config.test_image_path).is_absolute()
            {
                let abs = samples_dir.join(&config.test_image_path);
                config.test_image_path = abs.to_string_lossy().into_owned();
            }
            return config;
        }
    }

    // No config yet — build default with absolute image path and ground truth
    let config = DiagnosticConfig {
        test_image_path: samples_dir
            .join("fake-invoice-1.png")
            .to_string_lossy()
            .into_owned(),
        ground_truth: GroundTruth {
            invoice_number: Some("TEST20250400098765".into()),
            issue_date: Some("2025-04-30".into()),
            seller_name: Some("云海智联数码（测试）有限公司".into()),
            buyer_name: Some("星辰未来科技（测试）有限公司".into()),
            total_amount: Some(1882.02),
            amount_without_tax: Some(1665.50),
            tax_amount: Some(216.52),
            items_count: Some(2),
            ..GroundTruth::default()
        },
        enabled: true,
    };
    if let Err(e) = save_config(app_data_dir, &config) {
        warn!("Failed to persist default diagnostic config: {e}");
    }
    config
}

/// 保存诊断配置到磁盘。
pub fn save_config(app_data_dir: &Path, config: &DiagnosticConfig) -> std::io::Result<()> {
    let path = app_data_dir.join(DIAGNOSTIC_CONFIG_FILE);
    let json = serde_json::to_string_pretty(config)?;
    std::fs::write(path, json)
}

/// 执行完整诊断流程：文本生成、图片识别、结果对比、Embedding 检测。
pub async fn run_diagnostic(
    diag_config: &DiagnosticConfig,
    llm_config: &LlmProviderConfig,
    emb_test_result: Option<&EmbeddingTestResult>,
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
    let step2 = if diag_config.test_image_path.is_empty() {
        DiagnosticStep {
            name: "图片识别".into(),
            passed: false,
            duration_ms: 0,
            message: "测试图片路径未配置。请在 diagnostic_config.json 中设置 test_image_path。"
                .into(),
            details: None,
        }
    } else {
        let test_image_path = PathBuf::from(&diag_config.test_image_path);
        if test_image_path.exists() {
            let mime = infer_mime(&test_image_path);
            let start = Instant::now();
            info!("Diagnostic: sending test image to recognition");
            match recognize_invoice_with_retries(llm_config.clone(), &test_image_path, &mime, audit)
                .await
            {
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

    // Step 4: Embedding (optional, local ONNX)
    let step4 = if let Some(result) = emb_test_result {
        DiagnosticStep {
            name: "Embedding".into(),
            passed: true,
            duration_ms: result.duration_ms as u128,
            message: format!(
                "模型: {}，维度: {}，耗时: {}ms",
                result.model, result.dimensions, result.duration_ms
            ),
            details: None,
        }
    } else {
        DiagnosticStep {
            name: "Embedding".into(),
            passed: true,
            duration_ms: 0,
            message: "本地 Embedding 未加载，跳过".into(),
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

    let get_f64 = |obj: &serde_json::Value, key: &str| -> Option<f64> {
        obj.get(key).and_then(|v| v.as_f64()).or_else(|| {
            obj.get(key)
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok())
        })
    };

    // String fields: contains match
    // $nested: optional nested object name to search (e.g. "seller" or "buyer")
    macro_rules! check_str {
        ($field:ident, $keys:expr) => {
            check_str!($field, $keys, None::<&str>);
        };
        ($field:ident, $keys:expr, $nested:expr) => {
            if let Some(ref expected) = gt.$field {
                total += 1;
                let recognized_val = {
                    let mut found: Option<String> = None;
                    // Check top-level keys
                    for key in $keys {
                        if let Some(v) = recognized.get(key).and_then(|v| v.as_str()) {
                            let s = v.trim();
                            if !s.is_empty() {
                                found = Some(s.to_string());
                                break;
                            }
                        }
                    }
                    // Check nested object if specified
                    if found.is_none() {
                        if let Some(nested_name) = $nested {
                            if let Some(obj) =
                                recognized.get(nested_name).and_then(|v| v.as_object())
                            {
                                for key in $keys {
                                    if let Some(v) = obj.get(*key).and_then(|v| v.as_str()) {
                                        let s = v.trim();
                                        if !s.is_empty() {
                                            found = Some(s.to_string());
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    found
                };
                let got = recognized_val.as_deref().unwrap_or("(空)");
                let name = stringify!($field);
                if recognized_val.as_ref().map_or(false, |v| {
                    v.contains(expected.as_str()) || expected.contains(v.as_str())
                }) {
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
    check_str!(seller_name, &["seller_name", "name"], Some("seller"));
    check_str!(buyer_name, &["buyer_name", "name"], Some("buyer"));

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
                lines.push(format!(
                    "items_count: ✗ (期望: {expected_count}, 实际: {got})"
                ));
            }
        } else {
            lines.push(format!(
                "items_count: ✗ (期望: {expected_count}, 实际: (空))"
            ));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_ground_truth_matches_model_output() {
        let gt = GroundTruth {
            invoice_number: Some("TEST20250400098765".into()),
            issue_date: Some("2025-04-30".into()),
            seller_name: Some("云海智联数码（测试）有限公司".into()),
            buyer_name: Some("星辰未来科技（测试）有限公司".into()),
            total_amount: Some(1882.02),
            amount_without_tax: Some(1665.50),
            tax_amount: Some(216.52),
            items_count: Some(2),
            ..GroundTruth::default()
        };
        let recognized = serde_json::json!({
            "is_invoice": true,
            "invoice_type": null,
            "invoice_number": "TEST20250400098765",
            "issue_date": "2025-04-30",
            "seller": {"name": "云海智联数码（测试）有限公司", "tax_id": "ABCDEFG1234567890Z"},
            "buyer": {"name": "星辰未来科技（测试）有限公司", "tax_id": "1234567890ABCDEF12"},
            "total_amount": 1882.02,
            "amount_without_tax": 1665.50,
            "tax_amount": 216.52,
            "items": [
                {"name": "测试商品A-智能数据采集器"},
                {"name": "测试服务B-云端数据存储服务"}
            ]
        });
        let (matched, total, details) = compare_ground_truth(&gt, &recognized);
        eprintln!("{details}");
        assert_eq!(total, 8);
        assert_eq!(matched, 8, "all fields should match");
    }
}
