//! Red tests: UI headless sobre MockProvider (spec/09: paridade).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use harnizator_cli::run_headless;
use harnizator_core::{Message, TokenUsage};
use harnizator_providers::{ChatRequest, MockProvider};

const SCENARIO: &str = r#"
[[response]]
when_contains = "hello"

[[response.chunk]]
text = "Hello "

[[response.chunk]]
text = "world"

[[response.chunk]]
usage = { input = 5, output = 2 }
"#;

#[tokio::test]
async fn headless_writes_deltas_and_reports_usage() {
    let provider = MockProvider::from_scenario_str(SCENARIO).unwrap();
    let req = ChatRequest {
        model: "mock/test-model".into(),
        messages: vec![Message::user("hello")],
        max_tokens: 128,
        system: None,
        tools: vec![],
    };
    let mut out: Vec<u8> = vec![];
    let summary = run_headless(&provider, req, &mut out).await.unwrap();

    assert_eq!(String::from_utf8(out).unwrap(), "Hello world\n");
    assert_eq!(summary.text, "Hello world");
    assert_eq!(
        summary.usage,
        Some(TokenUsage {
            input: 5,
            output: 2
        })
    );
}

#[tokio::test]
async fn headless_propagates_provider_errors() {
    let provider = MockProvider::from_scenario_str(SCENARIO).unwrap();
    let req = ChatRequest {
        model: "mock/test-model".into(),
        messages: vec![Message::user("no match here")],
        max_tokens: 128,
        system: None,
        tools: vec![],
    };
    let mut out: Vec<u8> = vec![];
    assert!(run_headless(&provider, req, &mut out).await.is_err());
}
