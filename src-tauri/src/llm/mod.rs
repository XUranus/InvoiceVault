use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{Local, Utc};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProviderConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct LlmAuditConfig {
    pub dir: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct LlmConnectionTestResult {
    pub model: String,
    pub duration_ms: u128,
    pub response_preview: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InvoiceRecognitionResult {
    pub model: String,
    pub duration_ms: u128,
    pub response_json: String,
    pub response_preview: String,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("base URL is required")]
    MissingBaseUrl,
    #[error("API key is required")]
    MissingApiKey,
    #[error("model is required")]
    MissingModel,
    #[error("invalid authorization header")]
    InvalidAuthorizationHeader,
    #[error("request error: {0}")]
    Request(#[from] reqwest::Error),
    #[error("provider returned HTTP {status}: {body}")]
    ProviderStatus { status: u16, body: String },
    #[error("provider response did not include assistant content")]
    MissingAssistantContent,
    #[error("provider response did not include a JSON object")]
    MissingJsonObject,
    #[error("unsupported image MIME type for recognition: {0}")]
    UnsupportedImageMimeType(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    temperature: f32,
    max_tokens: u16,
}

#[derive(Debug, Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Serialize)]
struct VisionChatCompletionRequest<'a> {
    model: &'a str,
    messages: Vec<VisionChatMessage>,
    temperature: f32,
    max_tokens: u16,
}

#[derive(Debug, Serialize)]
struct VisionChatMessage {
    role: String,
    content: Vec<VisionContent>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum VisionContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrlContent },
}

#[derive(Debug, Serialize)]
struct ImageUrlContent {
    url: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
    usage: Option<ApiUsage>,
}

#[derive(Debug, Deserialize)]
struct ApiUsage {
    prompt_tokens: i64,
    completion_tokens: i64,
    total_tokens: i64,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct ChatChoiceMessage {
    content: Option<String>,
}

pub async fn test_llm_connection(
    config: LlmProviderConfig,
    audit: Option<&LlmAuditConfig>,
) -> Result<LlmConnectionTestResult, LlmError> {
    let base_url = config.base_url.trim().trim_end_matches('/');
    let api_key = config.api_key.trim();
    let model = config.model.trim();

    if base_url.is_empty() {
        return Err(LlmError::MissingBaseUrl);
    }
    if api_key.is_empty() {
        return Err(LlmError::MissingApiKey);
    }
    if model.is_empty() {
        return Err(LlmError::MissingModel);
    }

    let timeout = Duration::from_secs(config.timeout_seconds.unwrap_or(30).clamp(1, 300));
    let client = reqwest::Client::builder().timeout(timeout).build()?;
    let started_at = Utc::now();
    let started = Instant::now();
    let request = ChatCompletionRequest {
        model,
        messages: vec![ChatMessage {
            role: "user",
            content: "Reply with exactly: pong",
        }],
        temperature: 0.0,
        max_tokens: 512,
    };
    let endpoint = format!("{base_url}/chat/completions");
    let request_json = serde_json::to_value(&request)?;

    let response = match client
        .post(&endpoint)
        .headers(headers(api_key)?)
        .json(&request)
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            write_llm_audit_record(
                audit,
                LlmAuditRecord {
                    started_at,
                    operation: "connection_test",
                    endpoint: &endpoint,
                    model,
                    duration_ms: started.elapsed().as_millis(),
                    status: None,
                    request: request_json,
                    response: None,
                    error: Some(err.to_string()),
                },
            );
            return Err(err.into());
        }
    };

    let status = response.status();
    let status_code = status.as_u16();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        error!(
            "LLM connection test returned HTTP {status}: {}",
            truncate(&body, 200)
        );
        write_llm_audit_record(
            audit,
            LlmAuditRecord {
                started_at,
                operation: "connection_test",
                endpoint: &endpoint,
                model,
                duration_ms: started.elapsed().as_millis(),
                status: Some(status_code),
                request: request_json,
                response: Some(body_to_value(&body)),
                error: Some(format!("HTTP {status_code}")),
            },
        );
        return Err(LlmError::ProviderStatus {
            status: status_code,
            body: truncate(&body, 500),
        });
    }

    let body = response.text().await?;
    write_llm_audit_record(
        audit,
        LlmAuditRecord {
            started_at,
            operation: "connection_test",
            endpoint: &endpoint,
            model,
            duration_ms: started.elapsed().as_millis(),
            status: Some(status_code),
            request: request_json,
            response: Some(body_to_value(&body)),
            error: None,
        },
    );

    let response_body: ChatCompletionResponse = serde_json::from_str(&body)?;
    let content = response_body
        .choices
        .first()
        .and_then(|choice| choice.message.content.as_deref())
        .map(str::trim)
        .filter(|content| !content.is_empty())
        .ok_or(LlmError::MissingAssistantContent)?;

    info!(
        "LLM connection test OK: model={model}, {}ms",
        started.elapsed().as_millis()
    );
    Ok(LlmConnectionTestResult {
        model: model.to_owned(),
        duration_ms: started.elapsed().as_millis(),
        response_preview: truncate(content, 120),
    })
}

pub async fn recognize_invoice_image(
    config: LlmProviderConfig,
    image_path: &Path,
    mime_type: &str,
    audit: Option<&LlmAuditConfig>,
) -> Result<InvoiceRecognitionResult, LlmError> {
    let base_url = config.base_url.trim().trim_end_matches('/');
    let api_key = config.api_key.trim();
    let model = config.model.trim();

    if base_url.is_empty() {
        return Err(LlmError::MissingBaseUrl);
    }
    if api_key.is_empty() {
        return Err(LlmError::MissingApiKey);
    }
    if model.is_empty() {
        return Err(LlmError::MissingModel);
    }
    if !matches!(mime_type, "image/png" | "image/jpeg") {
        return Err(LlmError::UnsupportedImageMimeType(mime_type.to_owned()));
    }

    let timeout = Duration::from_secs(config.timeout_seconds.unwrap_or(90).clamp(1, 300));
    let client = reqwest::Client::builder().timeout(timeout).build()?;
    let image_bytes = fs::read(image_path)
        .inspect_err(|e| error!("Failed to read image for recognition: {e}"))?;
    let image_len = image_bytes.len();
    let image_data_url = format!("data:{mime_type};base64,{}", STANDARD.encode(image_bytes));
    let started_at = Utc::now();
    let started = Instant::now();
    info!("Sending recognition request, model={model}, {image_len} bytes");
    let request = VisionChatCompletionRequest {
        model,
        messages: vec![VisionChatMessage {
            role: "user".to_owned(),
            content: vec![
                VisionContent::Text {
                    text: invoice_recognition_prompt().to_owned(),
                },
                VisionContent::ImageUrl {
                    image_url: ImageUrlContent {
                        url: image_data_url,
                    },
                },
            ],
        }],
        temperature: 0.0,
        max_tokens: 4096,
    };
    let endpoint = format!("{base_url}/chat/completions");
    let request_json = serde_json::to_value(&request)?;

    let response = match client
        .post(&endpoint)
        .headers(headers(api_key)?)
        .json(&request)
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            write_llm_audit_record(
                audit,
                LlmAuditRecord {
                    started_at,
                    operation: "invoice_recognition",
                    endpoint: &endpoint,
                    model,
                    duration_ms: started.elapsed().as_millis(),
                    status: None,
                    request: request_json,
                    response: None,
                    error: Some(err.to_string()),
                },
            );
            return Err(err.into());
        }
    };

    let status = response.status();
    let status_code = status.as_u16();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        error!("LLM recognition HTTP {status}: {}", truncate(&body, 200));
        write_llm_audit_record(
            audit,
            LlmAuditRecord {
                started_at,
                operation: "invoice_recognition",
                endpoint: &endpoint,
                model,
                duration_ms: started.elapsed().as_millis(),
                status: Some(status_code),
                request: request_json,
                response: Some(body_to_value(&body)),
                error: Some(format!("HTTP {status_code}")),
            },
        );
        return Err(LlmError::ProviderStatus {
            status: status_code,
            body: truncate(&body, 500),
        });
    }

    let body = response.text().await?;
    write_llm_audit_record(
        audit,
        LlmAuditRecord {
            started_at,
            operation: "invoice_recognition",
            endpoint: &endpoint,
            model,
            duration_ms: started.elapsed().as_millis(),
            status: Some(status_code),
            request: request_json,
            response: Some(body_to_value(&body)),
            error: None,
        },
    );

    let response_body: ChatCompletionResponse = serde_json::from_str(&body)?;
    let content = response_body
        .choices
        .first()
        .and_then(|choice| choice.message.content.as_deref())
        .map(str::trim)
        .filter(|content| !content.is_empty())
        .ok_or_else(|| {
            error!("LLM recognition returned empty response content");
            LlmError::MissingAssistantContent
        })?;
    let response_json = extract_json_object(content)
        .inspect_err(|e| error!("Failed to extract JSON from recognition response: {e}"))?;

    let prompt_tokens = response_body.usage.as_ref().map_or(0, |u| u.prompt_tokens);
    let completion_tokens = response_body
        .usage
        .as_ref()
        .map_or(0, |u| u.completion_tokens);
    let total_tokens = response_body.usage.as_ref().map_or(0, |u| u.total_tokens);

    info!(
        "Recognition OK: model={model}, {}ms, tokens={total_tokens}",
        started.elapsed().as_millis()
    );
    Ok(InvoiceRecognitionResult {
        model: model.to_owned(),
        duration_ms: started.elapsed().as_millis(),
        response_preview: truncate(content, 160),
        response_json,
        prompt_tokens,
        completion_tokens,
        total_tokens,
    })
}

pub(crate) fn headers(api_key: &str) -> Result<HeaderMap, LlmError> {
    let mut headers = HeaderMap::new();
    let authorization = HeaderValue::from_str(&format!("Bearer {api_key}"))
        .map_err(|_| LlmError::InvalidAuthorizationHeader)?;
    headers.insert(AUTHORIZATION, authorization);
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    Ok(headers)
}

pub(crate) fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

pub struct LlmAuditRecord<'a> {
    pub started_at: chrono::DateTime<Utc>,
    pub operation: &'a str,
    pub endpoint: &'a str,
    pub model: &'a str,
    pub duration_ms: u128,
    pub status: Option<u16>,
    pub request: Value,
    pub response: Option<Value>,
    pub error: Option<String>,
}

pub fn write_llm_audit_record(audit: Option<&LlmAuditConfig>, record: LlmAuditRecord<'_>) {
    let Some(audit) = audit else {
        return;
    };

    if let Err(err) = write_llm_audit_record_inner(audit, record) {
        warn!("failed to write LLM audit log: {err}");
    }
}

fn write_llm_audit_record_inner(
    audit: &LlmAuditConfig,
    record: LlmAuditRecord<'_>,
) -> std::io::Result<()> {
    fs::create_dir_all(&audit.dir)?;
    let filename = format!("llm-audit-{}.jsonl", Local::now().format("%Y-%m-%d"));
    let path = audit.dir.join(filename);
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let line = json!({
        "timestamp": Utc::now().to_rfc3339(),
        "request_timestamp": record.started_at.to_rfc3339(),
        "operation": record.operation,
        "endpoint": record.endpoint,
        "model": record.model,
        "duration_ms": record.duration_ms,
        "status": record.status,
        "request": record.request,
        "response": record.response,
        "error": record.error,
    });
    serde_json::to_writer(&mut file, &line)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
    writeln!(file)?;
    Ok(())
}

pub fn body_to_value(body: &str) -> Value {
    serde_json::from_str(body).unwrap_or_else(|_| Value::String(body.to_owned()))
}

fn invoice_recognition_prompt() -> &'static str {
    r#"你是发票识别引擎。请只输出一个 JSON 对象，不要输出 Markdown、解释或代码块。

如果图片完全不是发票或票据（如风景照、人物照等），输出 {"is_invoice": false, "confidence": 0, "needs_review": true, "warnings": ["not an invoice"]}。
注意：即使图片带有"测试""样例""模拟""fake"等水印或标注，只要票据格式正确、字段可读，就应视为有效发票进行识别，不要因为水印而判定为非发票。

如果图片是发票、票据或测试票据，按下面字段输出。无法识别的字段用 null，金额用数字，日期必须使用 YYYY-MM-DD。
支持并尽量准确区分这些类型：
- 增值税电子普通发票、增值税电子专用发票、全电发票、增值税普通发票、增值税专用发票
- 通行费发票、出租车发票、火车票/铁路电子客票、机票行程单
- 定额发票、卷式发票、机动车销售统一发票、二手车销售统一发票
- 海关进口增值税专用缴款书、财政电子票据、非税收入票据、普通收据

特殊票据没有标准购销方时，尽量把承运方/收款方放入 seller，把乘客/付款方放入 buyer。特殊字段放入 extra_fields，不要塞进 remarks。
{
  "is_invoice": true,
  "invoice_type": "string|null",
  "invoice_code": "string|null",
  "invoice_number": "string|null",
  "issue_date": "YYYY-MM-DD|null",
  "seller": {"name": "string|null", "tax_id": "string|null"},
  "buyer": {"name": "string|null", "tax_id": "string|null"},
  "currency": "CNY",
  "amount_without_tax": "number|null",
  "tax_amount": "number|null",
  "total_amount": "number|null",
  "category": "string|null",
  "items": [
    {
      "name": "string|null",
      "spec": "string|null",
      "unit": "string|null",
      "quantity": "number|null",
      "unit_price": "number|null",
      "amount": "number|null",
      "tax_rate": "number|null",
      "tax_amount": "number|null"
    }
  ],
  "remarks": "string|null",
  "extra_fields": {
    "passenger_name": "string|null",
    "train_number": "string|null",
    "flight_number": "string|null",
    "departure": "string|null",
    "arrival": "string|null",
    "departure_time": "string|null",
    "arrival_time": "string|null",
    "toll_entry": "string|null",
    "toll_exit": "string|null",
    "license_plate": "string|null",
    "vehicle_type": "string|null",
    "vehicle_model": "string|null",
    "vin": "string|null",
    "engine_number": "string|null",
    "tax_payment_certificate_number": "string|null",
    "receipt_code": "string|null",
    "receipt_number": "string|null"
  },
  "confidence": 0.0,
  "needs_review": true,
  "warnings": ["string"]
}"#
}

fn extract_json_object(value: &str) -> Result<String, LlmError> {
    let chars: Vec<(usize, char)> = value.char_indices().collect();
    let Some((start_index, _)) = chars.iter().find(|(_, char_value)| *char_value == '{') else {
        return Err(LlmError::MissingJsonObject);
    };

    let mut depth = 0_i32;
    let mut in_string = false;
    let mut escaped = false;

    for (index, char_value) in value[*start_index..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            match char_value {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match char_value {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let end_index = *start_index + index + char_value.len_utf8();
                    return Ok(value[*start_index..end_index].trim().to_owned());
                }
            }
            _ => {}
        }
    }

    Err(LlmError::MissingJsonObject)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejects_missing_provider_fields() {
        let err = test_llm_connection(
            LlmProviderConfig {
                base_url: String::new(),
                api_key: "key".to_owned(),
                model: "model".to_owned(),
                timeout_seconds: Some(1),
            },
            None,
        )
        .await
        .expect_err("missing base url");

        assert!(matches!(err, LlmError::MissingBaseUrl));
    }

    #[test]
    fn truncates_by_char_boundary() {
        assert_eq!(truncate("发票识别", 2), "发票");
    }

    #[test]
    fn extracts_json_from_fenced_response() {
        let json = extract_json_object(
            r#"```json
{"is_invoice": true, "remarks": "brace in string } ok"}
```"#,
        )
        .expect("json object");

        assert_eq!(
            json,
            r#"{"is_invoice": true, "remarks": "brace in string } ok"}"#
        );
    }

    #[test]
    fn rejects_response_without_json() {
        let err = extract_json_object("not json").expect_err("missing json object");

        assert!(matches!(err, LlmError::MissingJsonObject));
    }

    #[test]
    fn writes_audit_record_as_jsonl() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let audit = LlmAuditConfig {
            dir: temp_dir.path().to_path_buf(),
        };
        write_llm_audit_record(
            Some(&audit),
            LlmAuditRecord {
                started_at: chrono::Utc::now(),
                operation: "test",
                endpoint: "https://example.test/v1/chat/completions",
                model: "model",
                duration_ms: 12,
                status: Some(200),
                request: serde_json::json!({"input":"ping"}),
                response: Some(serde_json::json!({"output":"pong"})),
                error: None,
            },
        );

        let entries = std::fs::read_dir(temp_dir.path())
            .expect("read audit dir")
            .collect::<Result<Vec<_>, _>>()
            .expect("entries");
        assert_eq!(entries.len(), 1);
        let contents = std::fs::read_to_string(entries[0].path()).expect("audit file");
        let value: serde_json::Value = serde_json::from_str(contents.trim()).expect("json line");
        assert_eq!(value["operation"], "test");
        assert_eq!(value["request"]["input"], "ping");
        assert_eq!(value["response"]["output"], "pong");
    }

    #[tokio::test]
    #[ignore]
    async fn live_llm_connection_from_env() {
        let result = test_llm_connection(
            LlmProviderConfig {
                base_url: std::env::var("RECEIPTIER_LLM_BASE_URL")
                    .expect("RECEIPTIER_LLM_BASE_URL"),
                api_key: std::env::var("RECEIPTIER_LLM_API_KEY").expect("RECEIPTIER_LLM_API_KEY"),
                model: std::env::var("RECEIPTIER_LLM_MODEL").expect("RECEIPTIER_LLM_MODEL"),
                timeout_seconds: Some(30),
            },
            None,
        )
        .await
        .expect("live llm connection");

        assert!(!result.response_preview.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn live_invoice_image_recognition_from_env() {
        let repo_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let sample_path = repo_dir
            .join("receipts")
            .join("微信图片_20260430161538.jpg");
        let result = recognize_invoice_image(
            LlmProviderConfig {
                base_url: std::env::var("RECEIPTIER_LLM_BASE_URL")
                    .expect("RECEIPTIER_LLM_BASE_URL"),
                api_key: std::env::var("RECEIPTIER_LLM_API_KEY").expect("RECEIPTIER_LLM_API_KEY"),
                model: std::env::var("RECEIPTIER_LLM_MODEL").expect("RECEIPTIER_LLM_MODEL"),
                timeout_seconds: Some(120),
            },
            &sample_path,
            "image/jpeg",
            None,
        )
        .await
        .expect("live invoice recognition");

        let value: serde_json::Value =
            serde_json::from_str(&result.response_json).expect("valid json response");
        assert_eq!(
            value.get("is_invoice").and_then(|value| value.as_bool()),
            Some(true)
        );
    }
}

pub async fn analyze_error_with_llm(
    config: LlmProviderConfig,
    error_message: &str,
) -> Result<String, LlmError> {
    let base_url = config.base_url.trim().trim_end_matches('/');
    let api_key = config.api_key.trim();
    let model = config.model.trim();

    if base_url.is_empty() || api_key.is_empty() || model.is_empty() {
        return Err(LlmError::MissingBaseUrl);
    }

    let timeout = Duration::from_secs(config.timeout_seconds.unwrap_or(30).clamp(1, 60));
    let client = reqwest::Client::builder().timeout(timeout).build()?;

    let prompt = format!(
        r#"你是一个邮件系统技术支持助手。用户在配置邮箱数据源时遇到了连接错误，请根据错误信息分析原因并给出简洁的解决建议（中文回答，不超过3句话）。

错误信息：
{error_message}"#
    );

    let request = ChatCompletionRequest {
        model,
        messages: vec![ChatMessage {
            role: "user",
            content: &prompt,
        }],
        temperature: 0.3,
        max_tokens: 256,
    };
    let endpoint = format!("{base_url}/chat/completions");

    let response = client
        .post(&endpoint)
        .headers(headers(api_key)?)
        .json(&request)
        .send()
        .await?;

    let body = response.text().await?;
    let api_response: ChatCompletionResponse = serde_json::from_str(&body)?;

    let content = api_response
        .choices
        .first()
        .and_then(|c| c.message.content.as_deref())
        .unwrap_or("无法分析错误原因")
        .to_owned();

    Ok(content)
}
