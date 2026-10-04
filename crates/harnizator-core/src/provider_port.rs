//! Port de providers LLM (vive no core; adapters em harnizator-providers).

use std::pin::Pin;

use futures::Stream;

use crate::message::Message;
use crate::tool_port::ToolSpec;

/// Requisição de chat normalizada (independente de provider).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatRequest {
    /// Nome do modelo (sem o prefixo `provider/`).
    pub model: String,
    pub messages: Vec<Message>,
    pub max_tokens: u32,
    pub system: Option<String>,
    /// Tools anunciadas ao modelo.
    #[serde(default)]
    pub tools: Vec<ToolSpec>,
}

/// Informação de um modelo listado pelo provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: Option<String>,
}

/// Chunk normalizado de streaming.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamChunk {
    MessageStart,
    TextDelta(String),
    ToolUse(crate::tool_port::ToolCall),
    Usage { input: u64, output: u64 },
    MessageStop,
}

/// Erros tipados de provider (ver spec/03).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProviderError {
    #[error("authentication failed for provider")]
    Auth,
    #[error("rate limited after retries")]
    RateLimited,
    #[error("stream interrupted: {0}")]
    Stream(String),
    #[error("no scenario entry matched the last user message (mock provider)")]
    NoScenarioMatch,
    #[error("invalid scenario: {0}")]
    InvalidScenario(String),
}

/// Stream boxed de chunks — o tipo que as UIs consomem.
pub type StreamResult = Pin<Box<dyn Stream<Item = Result<StreamChunk, ProviderError>> + Send>>;

/// Port de saída para provedores LLM (ver spec/03).
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    /// Identificador estável, ex.: "anthropic", "openai", "mock".
    fn id(&self) -> &str;

    /// Inicia um streaming de chat.
    async fn stream(&self, req: ChatRequest) -> Result<StreamResult, ProviderError>;

    /// Lista modelos disponíveis (usado pelo "testar conexão" da UI).
    async fn models(&self) -> Result<Vec<ModelInfo>, ProviderError>;
}
