//! Port de persistência de sessões (spec/07). Adapter: harnizator-store.

use crate::events::Event;
use crate::{AgentId, TokenUsage};

/// Metadados de uma sessão.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionMeta {
    pub id: String,
    pub title: String,
    pub model: String,
    pub sandbox_mode: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub usage: TokenUsage,
}

/// Evento persistido com metadados de ordem/tempo.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredEvent {
    pub agent: AgentId,
    pub seq: u64,
    pub ts: u64,
    pub event: Event,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("store error: {0}")]
    Backend(String),
    #[error("session not found: {0}")]
    NotFound(String),
}

/// Port de saída para persistência (SQLite no adapter).
pub trait SessionStore: Send + Sync {
    /// Cria uma sessão e retorna seu id.
    fn create_session(&self, title: &str, model: &str, sandbox: &str)
    -> Result<String, StoreError>;

    /// Append-only: grava um evento (seq por agente).
    fn append_event(
        &self,
        session: &str,
        agent: &AgentId,
        seq: u64,
        ts: u64,
        event: &Event,
    ) -> Result<(), StoreError>;

    /// Todos os eventos da sessão, ordenados por (agent, seq).
    fn events(&self, session: &str) -> Result<Vec<StoredEvent>, StoreError>;

    /// Lista sessões, mais recentes primeiro.
    fn sessions(&self) -> Result<Vec<SessionMeta>, StoreError>;

    /// Crash recovery: agentes Running/Idle passam a Interrupted (spec/07).
    fn recover(&self, session: &str) -> Result<u64, StoreError>;

    /// Acumula usage e atualiza updated_at.
    fn accumulate_usage(&self, session: &str, input: u64, output: u64) -> Result<(), StoreError>;
}
