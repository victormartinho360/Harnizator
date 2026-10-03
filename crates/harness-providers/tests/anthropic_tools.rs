//! Red tests: tool_use streaming — Anthropic (spec/04, Wave 2).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use futures::StreamExt;
use harness_core::Message;
use harness_providers::{AnthropicProvider, ChatRequest, LlmProvider, StreamChunk};
use secrecy::SecretString;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn streams_tool_use_with_accumulated_json_args() {
    let server = MockServer::start().await;
    let body = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/anthropic_tool.sse"
    ))
    .unwrap();
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;

    let provider = AnthropicProvider::new(
        "test-anthropic",
        &server.uri(),
        SecretString::from("sk-ant".to_string()),
    )
    .unwrap();
    let req = ChatRequest {
        model: "claude-test".into(),
        messages: vec![Message::user("read a.txt")],
        max_tokens: 64,
        system: None,
        tools: vec![],
    };

    let mut stream = provider.stream(req).await.unwrap();
    let mut chunks = vec![];
    while let Some(c) = stream.next().await {
        chunks.push(c.unwrap());
    }

    let tool_use = chunks
        .iter()
        .find_map(|c| match c {
            StreamChunk::ToolUse(call) => Some(call),
            _ => None,
        })
        .expect("expected a ToolUse chunk");
    assert_eq!(tool_use.id, "toolu_1");
    assert_eq!(tool_use.name, "read_file");
    assert_eq!(tool_use.args, serde_json::json!({"path": "a.txt"}));

    // encerra com Usage + MessageStop
    assert!(matches!(chunks.last(), Some(StreamChunk::MessageStop)));
}

#[tokio::test]
async fn sends_tools_and_tool_blocks_back() {
    // verifica o formato da request: tools anunciadas e tool_use/tool_result serializados
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "model": "claude-test",
            "max_tokens": 64,
            "stream": true,
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "read a.txt"}]},
                {"role": "assistant", "content": [
                    {"type": "tool_use", "id": "toolu_1", "name": "read_file", "input": {"path": "a.txt"}}
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "toolu_1", "content": "file contents", "is_error": false}
                ]}
            ],
            "tools": [
                {"name": "read_file", "description": "reads", "input_schema": {"type": "object"}}
            ]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        ))
        .expect(1)
        .mount(&server)
        .await;

    let provider = AnthropicProvider::new(
        "test-anthropic",
        &server.uri(),
        SecretString::from("sk-ant".to_string()),
    )
    .unwrap();
    let req = ChatRequest {
        model: "claude-test".into(),
        messages: vec![
            Message::user("read a.txt"),
            Message {
                role: harness_core::Role::Assistant,
                content: vec![harness_core::ContentBlock::ToolUse {
                    id: "toolu_1".into(),
                    name: "read_file".into(),
                    input: serde_json::json!({"path": "a.txt"}),
                }],
            },
            Message {
                role: harness_core::Role::User,
                content: vec![harness_core::ContentBlock::ToolResult {
                    tool_use_id: "toolu_1".into(),
                    content: "file contents".into(),
                    is_error: false,
                }],
            },
        ],
        max_tokens: 64,
        system: None,
        tools: vec![harness_core::tool_port::ToolSpec {
            name: "read_file".into(),
            description: "reads".into(),
            input_schema: serde_json::json!({"type": "object"}),
        }],
    };
    let mut stream = provider.stream(req).await.unwrap();
    while stream.next().await.is_some() {}
    // se chegou aqui sem panic do wiremock, o body casou
}
