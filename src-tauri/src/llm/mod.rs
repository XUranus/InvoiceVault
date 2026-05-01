use std::time::{Duration, Instant};

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
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

    Ok(LlmConnectionTestResult {
        model: model.to_owned(),
        duration_ms: started.elapsed().as_millis(),
        response_preview: truncate(content, 120),
    })
}

fn headers(api_key: &str) -> Result<HeaderMap, LlmError> {
    let mut headers = HeaderMap::new();
    let authorization = HeaderValue::from_str(&format!("Bearer {api_key}"))
        .map_err(|_| LlmError::InvalidAuthorizationHeader)?;
    headers.insert(AUTHORIZATION, authorization);
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    Ok(headers)
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
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
}
