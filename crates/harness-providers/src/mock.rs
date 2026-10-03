//! MockProvider: provider determinístico dirigido por cenário TOML.
//!
//! Peça central do TDD (ver 03-providers.md): respostas scriptadas por
//! substring do último prompt do usuário, com injeção de erros.

use std::path::Path;

use futures::stream;

use crate::provider::{
    ChatRequest, LlmProvider, ModelInfo, ProviderError, StreamChunk, StreamResult,
};

/// Um chunk scriptado do cenário.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(untagged)]
enum ScenarioChunk {
    Text { text: String },
    Error { error: String },
    Usage { usage: ScenarioUsage },
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ScenarioUsage {
    input: u64,
    output: u64,
}

/// Uma resposta scriptada, casada por substring do último prompt.
#[derive(Debug, Clone, serde::Deserialize)]
struct ScenarioResponse {
    when_contains: String,
    #[serde(default)]
    chunk: Vec<ScenarioChunk>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct Scenario {
    response: Vec<ScenarioResponse>,
}

/// Provider falso, deterministicamente scriptado via TOML.
pub struct MockProvider {
    scenario: Scenario,
}

impl MockProvider {
    /// Carrega um cenário de uma string TOML.
    pub fn from_scenario_str(toml_src: &str) -> Result<Self, ProviderError> {
        let scenario: Scenario =
            toml::from_str(toml_src).map_err(|e| ProviderError::InvalidScenario(e.to_string()))?;
        Ok(Self { scenario })
    }

    /// Carrega um cenário de um arquivo TOML (`--mock scenario.toml`).
    pub fn from_scenario_file(path: &Path) -> Result<Self, ProviderError> {
        let src = std::fs::read_to_string(path)
            .map_err(|e| ProviderError::InvalidScenario(e.to_string()))?;
        Self::from_scenario_str(&src)
    }

    fn match_response(&self, req: &ChatRequest) -> Option<&ScenarioResponse> {
        let last_user_text = req
            .messages
            .iter()
            .rev()
            .find(|m| matches!(m.role, harness_core::Role::User))
            .map(|m| m.text())
            .unwrap_or_default();
        self.scenario
            .response
            .iter()
            .find(|r| last_user_text.contains(&r.when_contains))
    }
}

#[async_trait::async_trait]
impl LlmProvider for MockProvider {
    fn id(&self) -> &str {
        "mock"
    }

    async fn stream(&self, req: ChatRequest) -> Result<StreamResult, ProviderError> {
        let response = self
            .match_response(&req)
            .ok_or(ProviderError::NoScenarioMatch)?;

        let mut items = vec![Ok(StreamChunk::MessageStart)];
        for chunk in &response.chunk {
            match chunk {
                ScenarioChunk::Text { text } => {
                    items.push(Ok(StreamChunk::TextDelta(text.clone())))
                }
                ScenarioChunk::Error { error } => {
                    items.push(Err(ProviderError::Stream(error.clone())));
                }
                ScenarioChunk::Usage { usage } => items.push(Ok(StreamChunk::Usage {
                    input: usage.input,
                    output: usage.output,
                })),
            }
        }
        items.push(Ok(StreamChunk::MessageStop));

        Ok(Box::pin(stream::iter(items)))
    }

    async fn models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(vec![ModelInfo {
            id: "mock/test-model".to_string(),
            display_name: Some("Mock test model".to_string()),
        }])
    }
}
