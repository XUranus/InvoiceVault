use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub enabled: bool,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            api_key: String::new(),
            model: "text-embedding-3-small".into(),
            enabled: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("embedding not configured")]
    NotConfigured,
    #[error("connection error: {0}")]
    Connection(String),
    #[error("api error: {0}")]
    Api(String),
    #[error("empty embedding returned")]
    EmptyEmbedding,
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("request error: {0}")]
    Reqwest(String),
}

impl From<reqwest::Error> for EmbeddingError {
    fn from(e: reqwest::Error) -> Self {
        EmbeddingError::Reqwest(e.to_string())
    }
}

/// Generate an embedding vector for the given text using an OpenAI-compatible /embeddings endpoint.
pub async fn generate_embedding(
    config: &EmbeddingConfig,
    text: &str,
) -> Result<Vec<f32>, EmbeddingError> {
    if !config.enabled || config.base_url.is_empty() {
        return Err(EmbeddingError::NotConfigured);
    }

    let base_url = config.base_url.trim().trim_end_matches('/');
    let url = format!("{base_url}/embeddings");

    #[derive(Serialize)]
    struct Request {
        model: String,
        input: String,
    }

    let body = Request {
        model: config.model.clone(),
        input: text.to_string(),
    };

    let mut req = reqwest::Client::new()
        .post(&url)
        .json(&body)
        .timeout(std::time::Duration::from_secs(30));

    if !config.api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", config.api_key));
    }

    let resp = req
        .send()
        .await
        .map_err(|e| EmbeddingError::Connection(e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        return Err(EmbeddingError::Api(format!("HTTP {status}: {text}")));
    }

    #[derive(Deserialize)]
    struct EmbeddingData {
        embedding: Vec<f32>,
    }

    #[derive(Deserialize)]
    struct Response {
        data: Vec<EmbeddingData>,
    }

    let response: Response = resp.json().await?;
    let embedding = response
        .data
        .into_iter()
        .next()
        .ok_or(EmbeddingError::EmptyEmbedding)?
        .embedding;

    if embedding.is_empty() {
        return Err(EmbeddingError::EmptyEmbedding);
    }

    Ok(embedding)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_disabled() {
        let config = EmbeddingConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.model, "text-embedding-3-small");
    }
}
