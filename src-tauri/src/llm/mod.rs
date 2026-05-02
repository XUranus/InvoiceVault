use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProviderConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout_seconds: Option<u64>,
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
    let started = Instant::now();
    let request = ChatCompletionRequest {
        model,
        messages: vec![ChatMessage {
            role: "user",
            content: "Reply with exactly: pong",
        }],
        temperature: 0.0,
        max_tokens: 16,
    };

    let response = client
        .post(format!("{base_url}/chat/completions"))
        .headers(headers(api_key)?)
        .json(&request)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        error!(
            "LLM connection test returned HTTP {status}: {}",
            truncate(&body, 200)
        );
        return Err(LlmError::ProviderStatus {
            status: status.as_u16(),
            body: truncate(&body, 500),
        });
    }

    let response_body: ChatCompletionResponse = response.json().await?;
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
        max_tokens: 1800,
    };

    let response = client
        .post(format!("{base_url}/chat/completions"))
        .headers(headers(api_key)?)
        .json(&request)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        error!("LLM recognition HTTP {status}: {}", truncate(&body, 200));
        return Err(LlmError::ProviderStatus {
            status: status.as_u16(),
            body: truncate(&body, 500),
        });
    }

    let response_body: ChatCompletionResponse = response.json().await?;
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

    info!(
        "Recognition OK: model={model}, {}ms",
        started.elapsed().as_millis()
    );
    Ok(InvoiceRecognitionResult {
        model: model.to_owned(),
        duration_ms: started.elapsed().as_millis(),
        response_preview: truncate(content, 160),
        response_json,
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

fn invoice_recognition_prompt() -> &'static str {
    r#"你是发票识别引擎。请只输出一个 JSON 对象，不要输出 Markdown、解释或代码块。

如果图片不是发票，输出 {"is_invoice": false, "confidence": 0, "needs_review": true, "warnings": ["not an invoice"]}。

如果图片是发票，按下面字段输出。无法识别的字段用 null，金额用数字，日期必须使用 YYYY-MM-DD：
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
        let err = test_llm_connection(LlmProviderConfig {
            base_url: String::new(),
            api_key: "key".to_owned(),
            model: "model".to_owned(),
            timeout_seconds: Some(1),
        })
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

    #[tokio::test]
    #[ignore]
    async fn live_llm_connection_from_env() {
        let result = test_llm_connection(LlmProviderConfig {
            base_url: std::env::var("RECEIPTIER_LLM_BASE_URL").expect("RECEIPTIER_LLM_BASE_URL"),
            api_key: std::env::var("RECEIPTIER_LLM_API_KEY").expect("RECEIPTIER_LLM_API_KEY"),
            model: std::env::var("RECEIPTIER_LLM_MODEL").expect("RECEIPTIER_LLM_MODEL"),
            timeout_seconds: Some(30),
        })
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
