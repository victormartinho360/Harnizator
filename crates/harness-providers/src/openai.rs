//! Provider OpenAI-compatible (chat completions SSE).

use futures::StreamExt;
use secrecy::{ExposeSecret, SecretString};
use serde_json::json;

use crate::http::send_with_retry;
use crate::provider::{
    ChatRequest, LlmProvider, ModelInfo, ProviderError, StreamChunk, StreamResult,
};
use crate::stream::{MapOutcome, sse_response_stream};

pub const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

/// Provider `kind = "openai" | "openai-compatible"`.
pub struct OpenAiProvider {
    id: String,
    base_url: String,
    api_key: SecretString,
    client: reqwest::Client,
}

impl OpenAiProvider {
    pub fn new(
        id: impl Into<String>,
        base_url: &str,
        api_key: SecretString,
    ) -> Result<Self, ProviderError> {
        Ok(Self {
            id: id.into(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            client: reqwest::Client::new(),
        })
    }
}

#[async_trait::async_trait]
impl LlmProvider for OpenAiProvider {
    fn id(&self) -> &str {
        &self.id
    }

    async fn stream(&self, req: ChatRequest) -> Result<StreamResult, ProviderError> {
        let messages: Vec<serde_json::Value> = req
            .messages
            .iter()
            .map(|m| {
                json!({
                    "role": match m.role {
                        harness_core::Role::User => "user",
                        harness_core::Role::Assistant => "assistant",
                        harness_core::Role::System => "system",
                    },
                    "content": m.text(),
                })
            })
            .collect();
        let mut messages_full = Vec::new();
        if let Some(system) = req.system {
            messages_full.push(json!({"role": "system", "content": system}));
        }
        messages_full.extend(messages);
        let body = json!({
            "model": req.model,
            "messages": messages_full,
            "max_tokens": req.max_tokens,
            "stream": true,
            "stream_options": {"include_usage": true},
        });

        let url = format!("{}/chat/completions", self.base_url);
        let key = self.api_key.clone();
        let resp = send_with_retry(|| {
            self.client
                .post(&url)
                .bearer_auth(key.expose_secret())
                .json(&body)
        })
        .await?;

        let inner = sse_response_stream(resp, |ev| {
            let data = ev.data.trim();
            if data == "[DONE]" {
                return MapOutcome {
                    chunks: vec![Ok(StreamChunk::MessageStop)],
                    is_stop: true,
                };
            }
            let v: serde_json::Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(e) => {
                    return MapOutcome {
                        chunks: vec![Err(ProviderError::Stream(e.to_string()))],
                        is_stop: false,
                    };
                }
            };
            let mut chunks = Vec::new();
            if let Some(text) = v["choices"]
                .get(0)
                .and_then(|c| c["delta"]["content"].as_str())
            {
                if !text.is_empty() {
                    chunks.push(Ok(StreamChunk::TextDelta(text.to_string())));
                }
            }
            if !v["usage"].is_null() && v.get("usage").is_some() {
                let u = &v["usage"];
                chunks.push(Ok(StreamChunk::Usage {
                    input: u["prompt_tokens"].as_u64().unwrap_or(0),
                    output: u["completion_tokens"].as_u64().unwrap_or(0),
                }));
            }
            MapOutcome {
                chunks,
                is_stop: false,
            }
        });
        // OpenAI não tem evento de início: emitimos MessageStart cedo.
        Ok(Box::pin(
            futures::stream::once(async { Ok(StreamChunk::MessageStart) }).chain(inner),
        ))
    }

    async fn models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Err(ProviderError::Stream(
            "models() not implemented yet".to_string(),
        ))
    }
}
