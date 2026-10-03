//! Red tests: tool_calls streaming — OpenAI-compatible (Wave 2).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use futures::StreamExt;
use harness_core::Message;
use harness_providers::{ChatRequest, LlmProvider, OpenAiProvider, StreamChunk};
use secrecy::SecretString;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn streams_tool_call_with_accumulated_arguments() {
    let server = MockServer::start().await;
    let chunks = [
        r#"{"choices":[{"delta":{"role":"assistant","tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"bash","arguments":""}}]}}]}"#,
        r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"cmd\":"}}]}}]}"#,
        r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"ls\"}"}}]}}]}"#,
        r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
        "[DONE]",
    ];
    let body: String = chunks.iter().map(|c| format!("data: {c}\n\n")).collect();
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;

    let provider = OpenAiProvider::new(
        "test-openai",
        &server.uri(),
        SecretString::from("sk-x".to_string()),
    )
    .unwrap();
    let req = ChatRequest {
        model: "gpt-test".into(),
        messages: vec![Message::user("run ls")],
        max_tokens: 64,
        system: None,
        tools: vec![],
    };
    let mut stream = provider.stream(req).await.unwrap();
    let mut got = vec![];
    while let Some(c) = stream.next().await {
        got.push(c.unwrap());
    }

    let call = got
        .iter()
        .find_map(|c| match c {
            StreamChunk::ToolUse(call) => Some(call),
            _ => None,
        })
        .expect("expected ToolUse chunk");
    assert_eq!(call.id, "call_1");
    assert_eq!(call.name, "bash");
    assert_eq!(call.args, serde_json::json!({"cmd": "ls"}));
    assert!(matches!(got.last(), Some(StreamChunk::MessageStop)));
}
