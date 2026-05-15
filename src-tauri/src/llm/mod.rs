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
    #[serde(default)]
    pub scnet_ocr_api_key: Option<String>,
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

const MAX_RETRIES: u32 = 3;

pub async fn recognize_invoice_image(
    config: LlmProviderConfig,
    image_path: &Path,
    mime_type: &str,
    temperature: f32,
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
        temperature,
        max_tokens: 4096,
    };
    let endpoint = format!("{base_url}/chat/completions");
    let request_json = serde_json::to_value(&request)?;
    let headers = headers(api_key)?;

    let mut last_err: Option<LlmError> = None;
    for attempt in 1..=MAX_RETRIES {
        if attempt > 1 {
            let sleep_secs = attempt * 2;
            info!("Retry {attempt}/{MAX_RETRIES} after {sleep_secs}s sleep");
            tokio::time::sleep(Duration::from_secs(sleep_secs as u64)).await;
        }

        let started_at = Utc::now();
        let started = Instant::now();

        let response = match client
            .post(&endpoint)
            .headers(headers.clone())
            .json(&request)
            .send()
            .await
        {
            Ok(response) => response,
            Err(err) => {
                let err_str = err.to_string();
                warn!("LLM request attempt {attempt} failed: {err_str}");
                last_err = Some(err.into());
                // Network errors are retryable
                continue;
            }
        };

        let status = response.status();
        let status_code = status.as_u16();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            error!("LLM recognition HTTP {status} (attempt {attempt}): {}", truncate(&body, 200));
            let llm_err = LlmError::ProviderStatus {
                status: status_code,
                body: truncate(&body, 500),
            };
            // Retry on 429 (rate limit) and 5xx (server error)
            if status_code == 429 || status_code >= 500 {
                last_err = Some(llm_err);
                continue;
            }
            // Non-retryable HTTP error
            write_llm_audit_record(
                audit,
                LlmAuditRecord {
                    started_at,
                    operation: "invoice_recognition",
                    endpoint: &endpoint,
                    model,
                    duration_ms: started.elapsed().as_millis(),
                    status: Some(status_code),
                    request: request_json.clone(),
                    response: Some(body_to_value(&body)),
                    error: Some(format!("HTTP {status_code}")),
                },
            );
            return Err(llm_err);
        }

        let body = response.text().await?;

        let response_body: ChatCompletionResponse = match serde_json::from_str(&body) {
            Ok(rb) => rb,
            Err(e) => {
                warn!("LLM response JSON parse failed (attempt {attempt}): {e}");
                last_err = Some(e.into());
                continue;
            }
        };

        let content = response_body
            .choices
            .first()
            .and_then(|choice| choice.message.content.as_deref())
            .map(str::trim)
            .filter(|content| !content.is_empty());

        let Some(content) = content else {
            warn!("LLM recognition returned empty response content (attempt {attempt})");
            last_err = Some(LlmError::MissingAssistantContent);
            continue;
        };

        let response_json = match extract_json_object(content) {
            Ok(json) => json,
            Err(e) => {
                warn!("Failed to extract JSON from recognition response (attempt {attempt}): {e}");
                last_err = Some(e);
                continue;
            }
        };

        // Success — write audit and return
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
                response: Some(body_to_value(content)),
                error: None,
            },
        );

        let prompt_tokens = response_body.usage.as_ref().map_or(0, |u| u.prompt_tokens);
        let completion_tokens = response_body
            .usage
            .as_ref()
            .map_or(0, |u| u.completion_tokens);
        let total_tokens = response_body.usage.as_ref().map_or(0, |u| u.total_tokens);

        info!(
            "Recognition OK: model={model}, {}ms, tokens={total_tokens}, attempts={attempt}",
            started.elapsed().as_millis()
        );
        return Ok(InvoiceRecognitionResult {
            model: model.to_owned(),
            duration_ms: started.elapsed().as_millis(),
            response_preview: truncate(content, 160),
            response_json,
            prompt_tokens,
            completion_tokens,
            total_tokens,
        });
    }

    // All retries exhausted
    Err(last_err.unwrap_or(LlmError::MissingAssistantContent))
}

const MAX_VLM_ATTEMPTS: u32 = 3;
const CONFIDENCE_THRESHOLD: f64 = 0.5;
const VLM_TEMPERATURES: [f32; 3] = [0.0, 0.3, 0.5];

/// Try VLM recognition multiple times with varying temperatures.
/// If all attempts yield low confidence, send all results to LLM for audit.
/// Then optionally cross-validate with SCNet OCR if API key is configured.
pub async fn recognize_invoice_with_retries(
    config: LlmProviderConfig,
    image_path: &Path,
    mime_type: &str,
    audit: Option<&LlmAuditConfig>,
) -> Result<InvoiceRecognitionResult, LlmError> {
    // Step 1: VLM recognition (with retries + audit)
    let mut vlm_result = recognize_vlm_only(config.clone(), image_path, mime_type, audit).await?;

    // Step 2: Optional SCNet OCR cross-validation
    let scnet_key = config.scnet_ocr_api_key.as_deref().filter(|k| !k.is_empty());
    if let Some(api_key) = scnet_key {
        info!("SCNet OCR enabled, running cross-validation");
        let scnet_started = Utc::now();
        let scnet_timer = Instant::now();
        match crate::scnet_ocr::recognize_with_scnet(api_key, image_path).await {
            Ok(Some(scnet_json)) => {
                let elapsed = scnet_timer.elapsed().as_millis();
                write_llm_audit_record(
                    audit,
                    LlmAuditRecord {
                        started_at: scnet_started,
                        operation: "scnet_ocr",
                        endpoint: "scnet",
                        model: "scnet-vat-ocr",
                        duration_ms: elapsed,
                        status: Some(200),
                        request: json!({ "image_path": image_path.to_string_lossy() }),
                        response: Some(json!(&scnet_json)),
                        error: None,
                    },
                );
                let merged_json = crate::scnet_ocr::merge_vlm_and_scnet(
                    &vlm_result.response_json,
                    &scnet_json,
                );
                info!("SCNet OCR merged successfully with VLM result in {elapsed}ms");
                vlm_result.response_json = merged_json;
                vlm_result.response_preview = truncate(&vlm_result.response_json, 160);
            }
            Ok(None) => {
                let elapsed = scnet_timer.elapsed().as_millis();
                write_llm_audit_record(
                    audit,
                    LlmAuditRecord {
                        started_at: scnet_started,
                        operation: "scnet_ocr",
                        endpoint: "scnet",
                        model: "scnet-vat-ocr",
                        duration_ms: elapsed,
                        status: Some(200),
                        request: json!({ "image_path": image_path.to_string_lossy() }),
                        response: Some(json!({ "result": "no_invoice_detected" })),
                        error: None,
                    },
                );
                info!("SCNet OCR returned no invoice results, using VLM result as-is");
            }
            Err(e) => {
                let elapsed = scnet_timer.elapsed().as_millis();
                write_llm_audit_record(
                    audit,
                    LlmAuditRecord {
                        started_at: scnet_started,
                        operation: "scnet_ocr",
                        endpoint: "scnet",
                        model: "scnet-vat-ocr",
                        duration_ms: elapsed,
                        status: None,
                        request: json!({ "image_path": image_path.to_string_lossy() }),
                        response: None,
                        error: Some(e.to_string()),
                    },
                );
                warn!("SCNet OCR failed in {elapsed}ms, falling back to VLM result: {e}");
            }
        }
    }

    Ok(vlm_result)
}

/// Core VLM recognition with retries and LLM audit fallback.
async fn recognize_vlm_only(
    config: LlmProviderConfig,
    image_path: &Path,
    mime_type: &str,
    audit: Option<&LlmAuditConfig>,
) -> Result<InvoiceRecognitionResult, LlmError> {
    let mut candidates: Vec<String> = Vec::new();

    for attempt in 1..=MAX_VLM_ATTEMPTS {
        if attempt > 1 {
            let backoff = Duration::from_secs(attempt as u64);
            info!("Waiting {backoff:?} before retry attempt {attempt}");
            tokio::time::sleep(backoff).await;
        }

        let temp = VLM_TEMPERATURES
            .get((attempt - 1) as usize)
            .copied()
            .unwrap_or(0.5);

        info!("VLM recognition attempt {attempt}/{MAX_VLM_ATTEMPTS}, temperature={temp}");

        let result = recognize_invoice_image(
            config.clone(),
            image_path,
            mime_type,
            temp,
            audit,
        )
        .await?;

        // Check if it's not an invoice — return immediately
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&result.response_json) {
            if value.get("is_invoice").and_then(|v| v.as_bool()) == Some(false) {
                info!("VLM determined not an invoice on attempt {attempt}, returning immediately");
                return Ok(result);
            }

            let confidence = value
                .get("confidence")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            if confidence >= CONFIDENCE_THRESHOLD {
                info!(
                    "VLM attempt {attempt} confidence={confidence:.2} >= {CONFIDENCE_THRESHOLD}, accepting"
                );
                return Ok(result);
            }

            info!("VLM attempt {attempt} confidence={confidence:.2} < {CONFIDENCE_THRESHOLD}");
        }

        candidates.push(result.response_json);
    }

    // All attempts have low confidence — audit with LLM
    info!("All {MAX_VLM_ATTEMPTS} VLM attempts low confidence, running LLM audit");
    audit_invoice_results(config, &candidates, audit).await
}

/// Send all candidate recognition results to LLM for comparison and best-pick.
async fn audit_invoice_results(
    config: LlmProviderConfig,
    candidate_jsons: &[String],
    _audit_config: Option<&LlmAuditConfig>,
) -> Result<InvoiceRecognitionResult, LlmError> {
    let base_url = config.base_url.trim().trim_end_matches('/');
    let api_key = config.api_key.trim();
    let model = config.model.trim();

    let timeout = Duration::from_secs(config.timeout_seconds.unwrap_or(90).clamp(1, 300));
    let client = reqwest::Client::builder().timeout(timeout).build()?;

    let candidates_formatted: Vec<serde_json::Value> = candidate_jsons
        .iter()
        .enumerate()
        .map(|(i, json_str)| {
            serde_json::from_str::<serde_json::Value>(json_str)
                .unwrap_or_else(|_| serde_json::json!({"parse_error": true, "raw": json_str}))
                .as_object()
                .map(|obj| {
                    let mut filtered = obj.clone();
                    // Remove verbose items array for audit brevity
                    filtered.remove("items");
                    filtered.remove("extra_fields");
                    serde_json::Value::Object(filtered)
                })
                .unwrap_or_else(|| serde_json::json!({"index": i}))
        })
        .collect();

    let prompt = format!(
        r#"你是发票识别结果审计引擎。以下是一张发票图片被 VLM（视觉大模型）多次识别的结果。
请比较这些结果，选择最准确的一个，并评估可信度。

审计要点：
1. 比对关键字段一致性：发票号码、金额、购销方名称、日期
2. 如果多数结果一致但个别不同，以多数为准
3. 选择字段最完整、数值最合理的结果
4. 如果所有结果都有严重问题（如金额异常大、日期不合理），仍选择最接近的并给低分

只输出一个 JSON 对象，不要输出其他内容：
{{"selected_index": 0, "confidence": 0.85, "reason": "简要原因"}}

候选结果（共 {} 个）：
{}"#,
        candidate_jsons.len(),
        serde_json::to_string_pretty(&candidates_formatted).unwrap_or_default()
    );

    info!("Sending VLM audit request to LLM, {} candidates", candidate_jsons.len());

    let started = Instant::now();
    let request = ChatCompletionRequest {
        model,
        messages: vec![ChatMessage {
            role: "user",
            content: &prompt,
        }],
        temperature: 0.0,
        max_tokens: 1024,
    };
    let endpoint = format!("{base_url}/chat/completions");

    let response = client
        .post(&endpoint)
        .headers(headers(api_key)?)
        .json(&request)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        error!("LLM audit returned HTTP {status}: {}", truncate(&body, 200));
        // Fallback: return the first candidate as-is
        warn!("LLM audit failed, falling back to first VLM candidate");
        return Ok(InvoiceRecognitionResult {
            model: model.to_owned(),
            duration_ms: started.elapsed().as_millis(),
            response_preview: truncate(&candidate_jsons[0], 160),
            response_json: candidate_jsons[0].clone(),
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        });
    }

    let body = response.text().await?;
    let api_response: ChatCompletionResponse = serde_json::from_str(&body)?;

    let content = api_response
        .choices
        .first()
        .and_then(|c| c.message.content.as_deref())
        .map(str::trim)
        .filter(|c| !c.is_empty());

    let Some(content) = content else {
        warn!("LLM audit returned empty content, falling back to first candidate");
        return Ok(InvoiceRecognitionResult {
            model: model.to_owned(),
            duration_ms: started.elapsed().as_millis(),
            response_preview: truncate(&candidate_jsons[0], 160),
            response_json: candidate_jsons[0].clone(),
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        });
    };

    // Parse audit result
    let audit_json_str = match extract_json_object(content) {
        Ok(json) => json,
        Err(_) => {
            warn!("Failed to parse audit JSON, falling back to first candidate");
            return Ok(InvoiceRecognitionResult {
                model: model.to_owned(),
                duration_ms: started.elapsed().as_millis(),
                response_preview: truncate(&candidate_jsons[0], 160),
                response_json: candidate_jsons[0].clone(),
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            });
        }
    };

    let audit_value: serde_json::Value = match serde_json::from_str(&audit_json_str) {
        Ok(v) => v,
        Err(_) => {
            warn!("Failed to deserialize audit JSON, falling back to first candidate");
            return Ok(InvoiceRecognitionResult {
                model: model.to_owned(),
                duration_ms: started.elapsed().as_millis(),
                response_preview: truncate(&candidate_jsons[0], 160),
                response_json: candidate_jsons[0].clone(),
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            });
        }
    };

    let selected_index = audit_value
        .get("selected_index")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;
    let audit_confidence = audit_value
        .get("confidence")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5);
    let reason = audit_value
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let selected_index = selected_index.min(candidate_jsons.len() - 1);
    let selected_json = &candidate_jsons[selected_index];

    // Inject audit confidence into the selected result JSON
    let mut selected_value: serde_json::Value = serde_json::from_str(selected_json)
        .unwrap_or_else(|_| serde_json::json!({}));
    if let Some(obj) = selected_value.as_object_mut() {
        obj.insert(
            "confidence".to_string(),
            serde_json::json!(audit_confidence),
        );
        obj.insert(
            "needs_review".to_string(),
            serde_json::json!(audit_confidence < CONFIDENCE_THRESHOLD),
        );
    }
    let final_json = serde_json::to_string(&selected_value).unwrap_or_else(|_| selected_json.clone());

    let prompt_tokens = api_response.usage.as_ref().map_or(0, |u| u.prompt_tokens);
    let completion_tokens = api_response.usage.as_ref().map_or(0, |u| u.completion_tokens);
    let total_tokens = api_response.usage.as_ref().map_or(0, |u| u.total_tokens);

    info!(
        "LLM audit selected candidate {selected_index}, confidence={audit_confidence:.2}, reason: {reason}"
    );

    Ok(InvoiceRecognitionResult {
        model: model.to_owned(),
        duration_ms: started.elapsed().as_millis(),
        response_preview: truncate(&final_json, 160),
        response_json: final_json,
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
                scnet_ocr_api_key: None,
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
                scnet_ocr_api_key: None,
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
                scnet_ocr_api_key: None,
            },
            &sample_path,
            "image/jpeg",
            0.0,
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
