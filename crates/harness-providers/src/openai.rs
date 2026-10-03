//! Provider OpenAI-compatible (chat completions SSE).

use std::collections::BTreeMap;

use futures::StreamExt;
use secrecy::{ExposeSecret, SecretString};
use serde_json::json;

use harness_core::tool_port::ToolCall;
use harness_core::{ContentBlock, Role};

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

/// Converte mensagens de domínio para o formato chat completions.
/// Mensagens de tool_result viram mensagens `role=tool` separadas.
fn openai_messages(msgs: &[harness_core::Message]) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for m in msgs {
        for b in &m.content {
            match b {
                ContentBlock::Text { text } => out.push(json!({
                    "role": match m.role {
                        Role::User => "user",
                        Role::Assistant => "assistant",
                        Role::System => "system",
                    },
                    "content": text,
                })),
                ContentBlock::ToolUse { id, name, input } => out.push(json!({
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": id,
                        "type": "function",
                        "function": {"name": name, "arguments": input.to_string()},
                    }],
                })),
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    ..
                } => out.push(json!({
                    "role": "tool",
                    "tool_call_id": tool_use_id,
                    "content": content,
                })),
            }
        }
    }
    out
}

#[async_trait::async_trait]
impl LlmProvider for OpenAiProvider {
    fn id(&self) -> &str {
        &self.id
    }

    async fn stream(&self, req: ChatRequest) -> Result<StreamResult, ProviderError> {
        let mut messages: Vec<serde_json::Value> = Vec::new();
        if let Some(system) = req.system {
            messages.push(json!({"role": "system", "content": system}));
        }
        messages.extend(openai_messages(&req.messages));
        let mut body = json!({
            "model": req.model,
            "messages": messages,
            "max_tokens": req.max_tokens,
            "stream": true,
            "stream_options": {"include_usage": true},
        });
        if !req.tools.is_empty() {
            body["tools"] = json!(
                req.tools
                    .iter()
                    .map(|t| json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.input_schema,
                        },
                    }))
                    .collect::<Vec<_>>()
            );
        }

        let url = format!("{}/chat/completions", self.base_url);
        let key = self.api_key.clone();
        let resp = send_with_retry(|| {
            self.client
                .post(&url)
                .bearer_auth(key.expose_secret())
                .json(&body)
        })
        .await?;

        // acumulador de tool_calls parciais (index → (id, name, args_buf))
        let mut pending: BTreeMap<usize, (String, String, String)> = BTreeMap::new();
        let inner = sse_response_stream(resp, move |ev| {
            let out = |chunks: Vec<Result<StreamChunk, ProviderError>>, is_stop: bool| MapOutcome {
                chunks,
                is_stop,
            };
            let data = ev.data.trim();
            if data == "[DONE]" {
                let mut chunks = Vec::new();
                for (_, (id, name, buf)) in std::mem::take(&mut pending) {
                    match serde_json::from_str(buf.trim()) {
                        Ok(args) => {
                            chunks.push(Ok(StreamChunk::ToolUse(ToolCall { id, name, args })))
                        }
                        Err(e) => chunks.push(Err(ProviderError::Stream(format!(
                            "invalid tool args JSON: {e}"
                        )))),
                    }
                }
                chunks.push(Ok(StreamChunk::MessageStop));
                return out(chunks, true);
            }
            let v: serde_json::Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(e) => return out(vec![Err(ProviderError::Stream(e.to_string()))], false),
            };
            let mut chunks = Vec::new();
            if let Some(choice) = v["choices"].get(0) {
                if let Some(text) = choice["delta"]["content"].as_str() {
                    if !text.is_empty() {
                        chunks.push(Ok(StreamChunk::TextDelta(text.to_string())));
                    }
                }
                if let Some(calls) = choice["delta"]["tool_calls"].as_array() {
                    for tc in calls {
                        let idx = tc["index"].as_u64().unwrap_or(0) as usize;
                        let entry = pending
                            .entry(idx)
                            .or_insert_with(|| (String::new(), String::new(), String::new()));
                        if let Some(id) = tc["id"].as_str() {
                            entry.0 = id.to_string();
                        }
                        if let Some(name) = tc["function"]["name"].as_str() {
                            entry.1 = name.to_string();
                        }
                        if let Some(args) = tc["function"]["arguments"].as_str() {
                            entry.2.push_str(args);
                        }
                    }
                }
            }
            if !v["usage"].is_null() && v.get("usage").is_some() {
                let u = &v["usage"];
                chunks.push(Ok(StreamChunk::Usage {
                    input: u["prompt_tokens"].as_u64().unwrap_or(0),
                    output: u["completion_tokens"].as_u64().unwrap_or(0),
                }));
            }
            out(chunks, false)
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
