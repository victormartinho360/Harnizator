//! Red tests: models() de OpenAI-compatible e Anthropic via wiremock (spec/03).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use harness_providers::{AnthropicProvider, LlmProvider, ProviderError, OpenAiProvider};
use secrecy::SecretString;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn openai(server: &MockServer) -> OpenAiProvider {
    OpenAiProvider::new(
        "test-openai",
        &server.uri(),
        SecretString::from("sk-test".to_string()),
    )
    .unwrap()
}

fn anthropic(server: &MockServer) -> AnthropicProvider {
    AnthropicProvider::new(
        "test-anthropic",
        &server.uri(),
        SecretString::from("sk-ant".to_string()),
    )
    .unwrap()
}

// ---------- OpenAI-compatible ----------

#[tokio::test]
async fn openai_models_parses_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(header("authorization", "Bearer sk-test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "object": "list",
            "data": [
                {"id": "gpt-4o", "object": "model"},
                {"id": "gpt-4o-mini", "object": "model"}
            ]
        })))
        .mount(&server)
        .await;

    let models = openai(&server).models().await.unwrap();
    let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["gpt-4o", "gpt-4o-mini"]);
}

#[tokio::test]
async fn openai_models_nim_style_payload() {
    // NVIDIA NIM: mesmo shape OpenAI; ids com prefixo "nvidia/…"
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "object": "list",
            "data": [
                {"id": "nvidia/llama-3.1-nemotron-70b-instruct", "object": "model"},
            ]
        })))
        .mount(&server)
        .await;

    let models = openai(&server).models().await.unwrap();
    assert_eq!(models[0].id, "nvidia/llama-3.1-nemotron-70b-instruct");
}

#[tokio::test]
async fn openai_models_401_maps_to_auth() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .mount(&server)
        .await;

    let err = openai(&server).models().await;
    assert!(matches!(err, Err(ProviderError::Auth)));
}

#[tokio::test]
async fn openai_models_invalid_json_maps_to_stream_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let err = openai(&server).models().await;
    assert!(matches!(err, Err(ProviderError::Stream(_))));
}

// ---------- Anthropic ----------

#[tokio::test]
async fn anthropic_models_parses_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(header("x-api-key", "sk-ant"))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [
                {"id": "claude-sonnet-4-5", "type": "model", "display_name": "Claude Sonnet 4.5"},
                {"id": "claude-opus-4-1", "type": "model", "display_name": "Claude Opus 4.1"}
            ],
            "has_more": false
        })))
        .mount(&server)
        .await;

    let models = anthropic(&server).models().await.unwrap();
    let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["claude-sonnet-4-5", "claude-opus-4-1"]);
}

#[tokio::test]
async fn anthropic_models_401_maps_to_auth() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .mount(&server)
        .await;

    let err = anthropic(&server).models().await;
    assert!(matches!(err, Err(ProviderError::Auth)));
}
