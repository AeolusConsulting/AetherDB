use async_trait::async_trait;

use crate::provider::{ExtractionResult, LlmProvider, LlmProviderError};

const EXTRACTION_PROMPT: &str = r#"Extract entities and relationships from the following text. Return ONLY valid JSON with this exact structure:
{"entities": [{"name": "EntityName", "entity_type": "person|organization|concept|location|event|technology|other"}], "relationships": [{"source": "EntityName1", "target": "EntityName2", "relationship_type": "related_to|works_at|located_in|part_of|created_by|uses"}]}
If no entities are found, return {"entities": [], "relationships": []}.
Text: "#;

pub struct OpenAiLlmProvider {
    client: reqwest::Client,
    base_url: String,
    model: String,
    api_key: String,
}

impl OpenAiLlmProvider {
    pub fn new(base_url: &str, model: &str, api_key: &str) -> Self {
        OpenAiLlmProvider {
            client: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            api_key: api_key.to_string(),
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAiLlmProvider {
    fn model_name(&self) -> &str {
        &self.model
    }

    async fn extract_entities(&self, content: &str) -> Result<ExtractionResult, LlmProviderError> {
        let truncated: String = content.chars().take(4000).collect();
        let url = format!("{}/chat/completions", self.base_url);
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": "You are an entity extraction assistant. Always respond with valid JSON only."},
                {"role": "user", "content": format!("{EXTRACTION_PROMPT}{truncated}")}
            ],
            "temperature": 0.0,
            "response_format": {"type": "json_object"},
        });

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmProviderError::Retryable(e.to_string()))?;

        if resp.status().is_server_error() {
            let text = resp.text().await.unwrap_or_default();
            return Err(LlmProviderError::Retryable(format!("HTTP 5xx: {text}")));
        }
        if resp.status().is_client_error() {
            let text = resp.text().await.unwrap_or_default();
            return Err(LlmProviderError::Permanent(format!("HTTP 4xx: {text}")));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| LlmProviderError::Retryable(e.to_string()))?;

        let content_str = json
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .unwrap_or("{}");

        serde_json::from_str(content_str)
            .map_err(|e| LlmProviderError::ParseError(format!("JSON parse error: {e}")))
    }
}
