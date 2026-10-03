//! harness-core: domínio puro do HarnessRS.
//!
//! ZERO dependências de IO/TUI/HTTP — apenas tipos de domínio, eventos
//! e a máquina de estados do agent loop (Wave 2+).

pub mod events;
pub mod message;
pub mod model_alias;

pub use events::Event;
pub use message::{ContentBlock, Message, Role};
pub use model_alias::{ModelAlias, ModelAliasError};

/// Identificador opaco de um agente na sessão.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct AgentId(String);

impl AgentId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Contagem de tokens de um turno.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
}
