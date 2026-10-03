//! Red tests for MockProvider + LlmProvider contract (Wave 0).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use futures::StreamExt;
use harness_core::Message;
use harness_providers::{ChatRequest, LlmProvider, MockProvider, ProviderError, StreamChunk};

fn scenario_toml() -> &'static str {
    r#"
[[response]]
when_contains = "hello"

[[response.chunk]]
text = "Hello "

[[response.chunk]]
text = "world"

[[response.chunk]]
usage = { input = 12, output = 3 }

[[response]]
when_contains = "fail"
[[response.chunk]]
error = "injected failure"
"#
}

fn req(text: &str) -> ChatRequest {
    ChatRequest {
        model: "mock/test-model".into(),
        messages: vec![Message::user(text)],
        max_tokens: 1024,
        system: None,
    }
}

#[tokio::test]
async fn mock_provider_streams_scripted_deltas() {
    let provider = MockProvider::from_scenario_str(scenario_toml()).unwrap();
    let mut stream = provider.stream(req("hello there")).await.unwrap();

    let mut chunks = vec![];
    while let Some(chunk) = stream.next().await {
        chunks.push(chunk.unwrap());
    }

    assert_eq!(
        chunks,
        vec![
            StreamChunk::MessageStart,
            StreamChunk::TextDelta("Hello ".into()),
            StreamChunk::TextDelta("world".into()),
            StreamChunk::Usage {
                input: 12,
                output: 3
            },
            StreamChunk::MessageStop,
        ]
    );
}

#[tokio::test]
async fn mock_provider_injects_scripted_errors() {
    let provider = MockProvider::from_scenario_str(scenario_toml()).unwrap();
    let mut stream = provider.stream(req("please fail")).await.unwrap();

    let mut saw_error = None;
    while let Some(chunk) = stream.next().await {
        if let Err(e) = chunk {
            saw_error = Some(e);
        }
    }
    let err = saw_error.expect("expected an injected error");
    assert!(matches!(err, ProviderError::Stream(_)));
}

#[tokio::test]
async fn mock_provider_errors_when_no_scenario_matches() {
    let provider = MockProvider::from_scenario_str(scenario_toml()).unwrap();
    let result = provider.stream(req("unmatched prompt")).await;
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("expected NoScenarioMatch"),
    };
    assert!(matches!(err, ProviderError::NoScenarioMatch));
}

#[test]
fn scenario_parses_from_toml() {
    let provider = MockProvider::from_scenario_str(scenario_toml()).unwrap();
    assert_eq!(provider.id(), "mock");
}
