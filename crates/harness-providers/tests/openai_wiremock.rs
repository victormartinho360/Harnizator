//! Red tests: OpenAI-compatible provider over wiremock (spec/03).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use futures::StreamExt;
use harness_core::Message;
use harness_providers::{ChatRequest, LlmProvider, OpenAiProvider, ProviderError, StreamChunk};
use secrecy::SecretString;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn provider(server: &MockServer) -> OpenAiProvider {
    OpenAiProvider::new(
        "test-openai",
        &server.uri(),
        SecretString::from("sk-test".to_string()),
    )
    .unwrap()
}

fn req() -> ChatRequest {
    ChatRequest {
        model: "gpt-test".into(),
        messages: vec![Message::user("hello")],
        max_tokens: 64,
        system: None,
    }
}

fn sse_body(chunks: &[&str]) -> String {
    chunks.iter().map(|c| format!("data: {c}\n\n")).collect()
}

#[tokio::test]
async fn streams_text_and_usage() {
    let server = MockServer::start().await;
    let body = sse_body(&[
        r#"{"choices":[{"delta":{"role":"assistant","content":""}}]}"#,
        r#"{"choices":[{"delta":{"content":"Hello"}}]}"#,
        r#"{"choices":[{"delta":{"content":" world"}}]}"#,
        r#"{"choices":[],"usage":{"prompt_tokens":9,"completion_tokens":2}}"#,
        "[DONE]",
    ]);
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("authorization", "Bearer sk-test"))
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
                input: 9,
                output: 2
            },
            StreamChunk::MessageStop,
        ]
    );
}

#[tokio::test]
async fn status_401_maps_to_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .mount(&server)
        .await;

    let err = provider(&server).stream(req()).await;
    assert!(matches!(err, Err(ProviderError::Auth)));
}

#[tokio::test]
async fn status_429_retries_then_succeeds() {
    let server = MockServer::start().await;
    // 2x 429, depois 200
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429))
        .up_to_n_times(2)
        .expect(2)
        .mount(&server)
        .await;
    let body = sse_body(&[r#"{"choices":[{"delta":{"content":"ok"}}]}"#, "[DONE]"]);
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .expect(1)
        .mount(&server)
        .await;

    let mut stream = provider(&server).stream(req()).await.unwrap();
    let mut text = String::new();
    while let Some(c) = stream.next().await {
        if let Ok(StreamChunk::TextDelta(t)) = c {
            text.push_str(&t);
        }
    }
    assert_eq!(text, "ok");
}

#[tokio::test]
async fn status_429_exhausts_retries() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;

    let err = provider(&server).stream(req()).await;
    assert!(matches!(err, Err(ProviderError::RateLimited)));
}

#[tokio::test]
async fn truncated_stream_yields_stream_interrupted() {
    let server = MockServer::start().await;
    // stream termina sem [DONE]
    let body = sse_body(&[r#"{"choices":[{"delta":{"content":"cut"}}]}"#]);
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;

    let mut stream = provider(&server).stream(req()).await.unwrap();
    let mut last_err = None;
    while let Some(c) = stream.next().await {
        if let Err(e) = c {
            last_err = Some(e);
        }
    }
    assert!(matches!(last_err, Some(ProviderError::Stream(_))));
}
