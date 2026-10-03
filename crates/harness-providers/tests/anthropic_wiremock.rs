//! Red tests: Anthropic provider over wiremock (spec/03).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use futures::StreamExt;
use harness_core::Message;
use harness_providers::{AnthropicProvider, ChatRequest, LlmProvider, ProviderError, StreamChunk};
use secrecy::SecretString;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn provider(server: &MockServer) -> AnthropicProvider {
    AnthropicProvider::new(
        "test-anthropic",
        &server.uri(),
        SecretString::from("sk-ant".to_string()),
    )
    .unwrap()
}

fn req() -> ChatRequest {
    ChatRequest {
        model: "claude-test".into(),
        messages: vec![Message::user("hello")],
        max_tokens: 64,
        system: None,
    }
}

#[tokio::test]
async fn streams_anthropic_fixture() {
    let server = MockServer::start().await;
    let body = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/anthropic_text.sse"
    ))
    .unwrap();
    Mock::given(method("POST"))
        .and(path("/messages"))
        .and(header("x-api-key", "sk-ant"))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;

    let mut stream = provider(&server).stream(req()).await.unwrap();
    let mut chunks = vec![];
    while let Some(c) = stream.next().await {
        chunks.push(c.unwrap());
    }
    assert_eq!(
        chunks,
        vec![
            StreamChunk::MessageStart,
            StreamChunk::TextDelta("Hello".into()),
            StreamChunk::TextDelta(" world".into()),
            StreamChunk::Usage {
                input: 12,
                output: 7
            },
            StreamChunk::MessageStop,
        ]
    );
}

#[tokio::test]
async fn status_401_maps_to_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let err = provider(&server).stream(req()).await;
    assert!(matches!(err, Err(ProviderError::Auth)));
}
