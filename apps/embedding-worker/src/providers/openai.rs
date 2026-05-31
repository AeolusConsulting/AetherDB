use aetherdb_domain::EmbeddingModel;
use async_trait::async_trait;

use crate::provider::{EmbeddingProvider, ProviderError};

pub struct OpenAiProvider {
    client: reqwest::Client,
    base_url: String,
    model: EmbeddingModel,
    dim: u16,
    api_key: String,
}

impl OpenAiProvider {
    pub fn new(base_url: &str, model: &str, dim: u16, api_key: &str) -> Self {
        OpenAiProvider {
            client: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            model: EmbeddingModel(model.to_string()),
            dim,
            api_key: api_key.to_string(),
        }
    }
}

#[async_trait]
impl EmbeddingProvider for OpenAiProvider {
    fn model(&self) -> &EmbeddingModel {
        &self.model
    }

    fn dim(&self) -> u16 {
        self.dim
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, ProviderError> {
        let url = format!("{}/embeddings", self.base_url);
        let body = serde_json::json!({
            "model": self.model.0,
            "input": texts,
        });

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Retryable(e.to_string()))?;

        if resp.status().is_server_error() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Retryable(format!("HTTP 5xx: {text}")));
        }
        if resp.status().is_client_error() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Permanent(format!("HTTP 4xx: {text}")));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ProviderError::Retryable(e.to_string()))?;

        let data = json
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| ProviderError::Retryable("missing 'data' array".into()))?;

        let mut vectors = Vec::with_capacity(data.len());
        for item in data {
            let embedding = item
                .get("embedding")
                .and_then(|e| e.as_array())
                .ok_or_else(|| ProviderError::Retryable("missing 'embedding'".into()))?;

            let vec: Vec<f32> = embedding
                .iter()
                .filter_map(|v| v.as_f64().map(|f| f as f32))
                .collect();

            if vec.len() != self.dim as usize {
                return Err(ProviderError::DimensionMismatch {
                    expected: self.dim,
                    got: vec.len() as u16,
                });
            }
            vectors.push(vec);
        }

        Ok(vectors)
    }
}
