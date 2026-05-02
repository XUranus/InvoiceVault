use std::{collections::HashMap, sync::Mutex};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChromaConfig {
    pub base_url: String,
    pub enabled: bool,
}

impl Default for ChromaConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:8000".into(),
            enabled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SimilarResult {
    pub invoice_id: i64,
    pub similarity: f64,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ChromaError {
    #[error("chromadb not configured")]
    NotConfigured,
    #[error("connection error: {0}")]
    Connection(String),
    #[error("http error: {0}")]
    Http(String),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("request error: {0}")]
    Reqwest(String),
}

impl From<reqwest::Error> for ChromaError {
    fn from(e: reqwest::Error) -> Self {
        ChromaError::Reqwest(e.to_string())
    }
}

const COLLECTION_NAME: &str = "invoice_embeddings";

pub struct ChromaClient {
    pub base_url: String,
    collection_id: Mutex<Option<String>>,
    client: reqwest::Client,
}

impl ChromaClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            collection_id: Mutex::new(None),
            client: reqwest::Client::new(),
        }
    }

    pub async fn health(&self) -> Result<bool, ChromaError> {
        let url = format!("{}/api/v2/heartbeat", self.base_url);
        let resp = self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| ChromaError::Connection(e.to_string()))?;

        Ok(resp.status().is_success())
    }

    pub async fn upsert_embedding(
        &self,
        invoice_id: i64,
        embedding: Vec<f32>,
        text: &str,
    ) -> Result<(), ChromaError> {
        let coll_id = self.ensure_collection().await?;
        let id = invoice_id.to_string();

        #[derive(Serialize)]
        struct UpsertBody {
            ids: Vec<String>,
            embeddings: Vec<Vec<f32>>,
            documents: Vec<String>,
            metadatas: Vec<HashMap<String, String>>,
        }

        let body = UpsertBody {
            ids: vec![id],
            embeddings: vec![embedding],
            documents: vec![text.to_string()],
            metadatas: vec![HashMap::new()],
        };

        let url = format!(
            "{}/api/v2/tenants/default_tenant/databases/default_database/collections/{}/upsert",
            self.base_url, coll_id
        );

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| ChromaError::Connection(e.to_string()))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ChromaError::Http(text));
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn delete_embedding(&self, invoice_id: i64) -> Result<(), ChromaError> {
        let coll_id = match self.collection_id.lock().expect("lock").as_ref() {
            Some(id) => id.clone(),
            None => return Ok(()), // Not initialized, nothing to delete
        };

        #[derive(Serialize)]
        struct DeleteBody {
            ids: Vec<String>,
        }

        let body = DeleteBody {
            ids: vec![invoice_id.to_string()],
        };

        let url = format!(
            "{}/api/v2/tenants/default_tenant/databases/default_database/collections/{}/delete",
            self.base_url, coll_id
        );

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| ChromaError::Connection(e.to_string()))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ChromaError::Http(text));
        }

        Ok(())
    }

    pub async fn query_similar(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<SimilarResult>, ChromaError> {
        let coll_id = self.ensure_collection().await?;

        #[derive(Serialize)]
        struct QueryBody {
            query_embeddings: Vec<Vec<f32>>,
            n_results: usize,
        }

        let body = QueryBody {
            query_embeddings: vec![embedding.to_vec()],
            n_results: limit,
        };

        let url = format!(
            "{}/api/v2/tenants/default_tenant/databases/default_database/collections/{}/query",
            self.base_url, coll_id
        );

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| ChromaError::Connection(e.to_string()))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ChromaError::Http(text));
        }

        #[derive(Deserialize)]
        struct QueryResponse {
            ids: Vec<Vec<String>>,
            distances: Vec<Vec<f64>>,
            metadatas: Vec<Vec<Option<HashMap<String, String>>>>,
        }

        let data: QueryResponse = resp.json().await?;

        let mut results = Vec::new();
        for i in 0..data.ids.first().map(|v| v.len()).unwrap_or(0) {
            let id_str = &data.ids[0][i];
            let distance = data.distances[0][i];
            let similarity = 1.0 / (1.0 + distance); // convert distance to similarity

            if let Ok(invoice_id) = id_str.parse::<i64>() {
                let metadata = data.metadatas[0][i].clone().unwrap_or_default();
                results.push(SimilarResult {
                    invoice_id,
                    similarity,
                    metadata,
                });
            }
        }

        Ok(results)
    }

    async fn ensure_collection(&self) -> Result<String, ChromaError> {
        if let Some(id) = self.collection_id.lock().expect("lock").as_ref() {
            return Ok(id.clone());
        }

        // Try to find existing collection
        let list_url = format!(
            "{}/api/v2/tenants/default_tenant/databases/default_database/collections",
            self.base_url
        );

        let resp = self
            .client
            .get(&list_url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| ChromaError::Connection(e.to_string()))?;

        if resp.status().is_success() {
            #[derive(Deserialize)]
            struct CollectionItem {
                id: String,
                name: String,
            }

            let collections: Vec<CollectionItem> = resp.json().await?;
            for c in &collections {
                if c.name == COLLECTION_NAME {
                    let mut lock = self.collection_id.lock().expect("lock");
                    *lock = Some(c.id.clone());
                    return Ok(c.id.clone());
                }
            }
        }

        // Create new collection
        #[derive(Serialize)]
        struct CreateBody {
            name: String,
            metadata: HashMap<String, String>,
        }

        let mut metadata = HashMap::new();
        metadata.insert("hnsw:space".to_string(), "cosine".to_string());

        let body = CreateBody {
            name: COLLECTION_NAME.to_string(),
            metadata,
        };

        let create_url = list_url; // same URL for POST

        let resp = self
            .client
            .post(&create_url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| ChromaError::Connection(e.to_string()))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ChromaError::Http(text));
        }

        #[derive(Deserialize)]
        struct Created {
            id: String,
        }

        let created: Created = resp.json().await?;
        let mut lock = self.collection_id.lock().expect("lock");
        *lock = Some(created.id.clone());

        Ok(created.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_disabled() {
        let config = ChromaConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.base_url, "http://localhost:8000");
    }

    #[test]
    fn client_constructs_with_trimmed_url() {
        let client = ChromaClient::new("http://localhost:8000/".into());
        assert_eq!(client.base_url, "http://localhost:8000");
    }
}
