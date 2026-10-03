//! Provider Anthropic (Messages API, SSE).

use secrecy::{ExposeSecret, SecretString};
use serde_json::json;

use crate::http::send_with_retry;
use crate::provider::{
    ChatRequest, LlmProvider, ModelInfo, ProviderError, StreamChunk, StreamResult,
};
use crate::sse::SseEvent;
use crate::stream::{MapOutcome, sse_response_stream};

pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com/v1";
pub const API_VERSION: &str = "2023-06-01";

/// Evento da Messages API, já normalizado dos frames SSE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnthropicEvent {
    MessageStart { input_tokens: u64 },
    TextDelta(String),
    OutputUsage { output_tokens: u64 },
    MessageStop,
    Ignored,
}

impl AnthropicEvent {
    /// Mapeia um frame SSE para um evento tipado (função pura, testável).
    pub fn from_sse(ev: &SseEvent) -> Result<Self, ProviderError> {
        let v: serde_json::Value =
            serde_json::from_str(&ev.data).map_err(|e| ProviderError::Stream(e.to_string()))?;
        match v.get("type").and_then(|t| t.as_str()) {
            Some("message_start") => {
                let input = v["message"]["usage"]["input_tokens"].as_u64().unwrap_or(0);
                Ok(Self::MessageStart {
                    input_tokens: input,
                })
            }
            Some("content_block_delta") => {
                if v["delta"]["type"].as_str() == Some("text_delta") {
                    Ok(Self::TextDelta(
                        v["delta"]["text"].as_str().unwrap_or_default().to_string(),
                    ))
                } else {
                    Ok(Self::Ignored)
                }
            }
            Some("message_delta") => {
                let output = v["usage"]["output_tokens"].as_u64().unwrap_or(0);
                Ok(Self::OutputUsage {
                    output_tokens: output,
                })
            }
            Some("message_stop") => Ok(Self::MessageStop),
            _ => Ok(Self::Ignored),
        }
    }
}

/// Provider `kind = "anthropic"`.
pub struct AnthropicProvider {
    id: String,
    base_url: String,
    api_key: SecretString,
    client: reqwest::Client,
}

impl AnthropicProvider {
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
impl LlmProvider for AnthropicProvider {
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
                        harness_core::Role::Assistant => "assistant",
                        _ => "user",
                    },
                    "content": m.text(),
                })
            })
            .collect();
        let mut body = json!({
            "model": req.model,
            "max_tokens": req.max_tokens,
            "messages": messages,
            "stream": true,
        });
        if let Some(system) = req.system {
            body["system"] = json!(system);
        }

        let url = format!("{}/messages", self.base_url);
        let key = self.api_key.clone();
        let resp = send_with_retry(|| {
            self.client
                .post(&url)
                .header("x-api-key", key.expose_secret())
                .header("anthropic-version", API_VERSION)
                .json(&body)
        })
        .await?;

        let mut input_tokens = 0u64;
        let mut output_tokens = 0u64;
        Ok(sse_response_stream(resp, move |ev| {
            let mapped = match AnthropicEvent::from_sse(ev) {
                Ok(m) => m,
                Err(e) => {
                    return MapOutcome {
                        chunks: vec![Err(e)],
                        is_stop: false,
                    };
                }
            };
            match mapped {
                AnthropicEvent::MessageStart { input_tokens: i } => {
                    input_tokens = i;
                    MapOutcome {
                        chunks: vec![Ok(StreamChunk::MessageStart)],
                        is_stop: false,
                    }
                }
                AnthropicEvent::TextDelta(t) => MapOutcome {
                    chunks: vec![Ok(StreamChunk::TextDelta(t))],
                    is_stop: false,
                },
                AnthropicEvent::OutputUsage { output_tokens: o } => {
                    output_tokens = o;
                    MapOutcome {
                        chunks: vec![],
                        is_stop: false,
                    }
                }
                AnthropicEvent::MessageStop => MapOutcome {
                    chunks: vec![
                        Ok(StreamChunk::Usage {
                            input: input_tokens,
                            output: output_tokens,
                        }),
                        Ok(StreamChunk::MessageStop),
                    ],
                    is_stop: true,
                },
                AnthropicEvent::Ignored => MapOutcome {
                    chunks: vec![],
                    is_stop: false,
                },
            }
        }))
    }

    async fn models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Err(ProviderError::Stream(
            "models() not implemented yet".to_string(),
        ))
    }
}
