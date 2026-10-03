//! Provider Anthropic (Messages API, SSE).

use secrecy::{ExposeSecret, SecretString};
use serde_json::json;

use harness_core::tool_port::ToolCall;
use harness_core::{ContentBlock, Role};

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
    ToolUseStart { id: String, name: String },
    InputJsonDelta(String),
    ContentBlockStop,
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
            Some("content_block_start") => {
                if v["content_block"]["type"].as_str() == Some("tool_use") {
                    Ok(Self::ToolUseStart {
                        id: v["content_block"]["id"]
                            .as_str()
                            .unwrap_or_default()
                            .to_string(),
                        name: v["content_block"]["name"]
                            .as_str()
                            .unwrap_or_default()
                            .to_string(),
                    })
                } else {
                    Ok(Self::Ignored)
                }
            }
            Some("content_block_delta") => match v["delta"]["type"].as_str() {
                Some("text_delta") => Ok(Self::TextDelta(
                    v["delta"]["text"].as_str().unwrap_or_default().to_string(),
                )),
                Some("input_json_delta") => Ok(Self::InputJsonDelta(
                    v["delta"]["partial_json"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                )),
                _ => Ok(Self::Ignored),
            },
            Some("content_block_stop") => Ok(Self::ContentBlockStop),
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

/// Converte blocos de domínio para o formato da Messages API.
fn anthropic_content(msg: &harness_core::Message) -> serde_json::Value {
    let blocks: Vec<serde_json::Value> = msg
        .content
        .iter()
        .map(|b| match b {
            ContentBlock::Text { text } => json!({"type": "text", "text": text}),
            ContentBlock::ToolUse { id, name, input } => {
                json!({"type": "tool_use", "id": id, "name": name, "input": input})
            }
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => json!({
                "type": "tool_result",
                "tool_use_id": tool_use_id,
                "content": content,
                "is_error": is_error,
            }),
        })
        .collect();
    json!(blocks)
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
                        Role::Assistant => "assistant",
                        _ => "user",
                    },
                    "content": anthropic_content(m),
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
        if !req.tools.is_empty() {
            body["tools"] = json!(
                req.tools
                    .iter()
                    .map(|t| json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.input_schema,
                    }))
                    .collect::<Vec<_>>()
            );
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
        // acumulador de tool_use: args chegam como JSON parcial
        let mut pending_tool: Option<(String, String, String)> = None;
        Ok(sse_response_stream(resp, move |ev| {
            let mapped = match AnthropicEvent::from_sse(ev) {
                Ok(m) => m,
                Err(e) => {
                    return map(None, Err(e));
                }
            };
            let map_local = mapped;
            match map_local {
                AnthropicEvent::MessageStart { input_tokens: i } => {
                    input_tokens = i;
                    map(Some(StreamChunk::MessageStart), Ok(()))
                }
                AnthropicEvent::TextDelta(t) => map(Some(StreamChunk::TextDelta(t)), Ok(())),
                AnthropicEvent::ToolUseStart { id, name } => {
                    pending_tool = Some((id, name, String::new()));
                    map(None, Ok(()))
                }
                AnthropicEvent::InputJsonDelta(partial) => {
                    if let Some((_, _, buf)) = pending_tool.as_mut() {
                        buf.push_str(&partial);
                    }
                    map(None, Ok(()))
                }
                AnthropicEvent::ContentBlockStop => {
                    if let Some((id, name, buf)) = pending_tool.take() {
                        let trimmed = buf.trim();
                        if trimmed.is_empty() {
                            return map(
                                Some(StreamChunk::ToolUse(ToolCall {
                                    id,
                                    name,
                                    args: serde_json::json!({}),
                                })),
                                Ok(()),
                            );
                        }
                        return match serde_json::from_str(trimmed) {
                            Ok(args) => map(
                                Some(StreamChunk::ToolUse(ToolCall { id, name, args })),
                                Ok(()),
                            ),
                            Err(e) => map(
                                None,
                                Err(ProviderError::Stream(format!(
                                    "invalid tool args JSON: {e}"
                                ))),
                            ),
                        };
                    }
                    map(None, Ok(()))
                }
                AnthropicEvent::OutputUsage { output_tokens: o } => {
                    output_tokens = o;
                    map(None, Ok(()))
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
                AnthropicEvent::Ignored => map(None, Ok(())),
            }
        }))
    }

    async fn models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Err(ProviderError::Stream(
            "models() not implemented yet".to_string(),
        ))
    }
}

/// Helper local: monta MapOutcome para zero/um chunk Ok ou um Err.
fn map(chunk: Option<StreamChunk>, unit: Result<(), ProviderError>) -> MapOutcome {
    let chunks = match (chunk, unit) {
        (Some(c), _) => vec![Ok(c)],
        (None, Ok(())) => vec![],
        (None, Err(e)) => vec![Err(e)],
    };
    MapOutcome {
        chunks,
        is_stop: false,
    }
}
