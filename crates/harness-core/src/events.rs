//! Eventos publicados no bus interno (domínio puro).

use crate::AgentId;

/// Eventos de domínio do sistema. Seriáveis para persistência (Wave 5)
/// e consumíveis por qualquer UI via `UiUpdate` (Wave 3+).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    /// Delta de texto de streaming do assistente.
    AssistantDelta { agent: AgentId, text: String },
    /// Tool call recebida, aguardando decisão de sandbox/aprovação.
    ToolCallRequested {
        agent: AgentId,
        id: String,
        name: String,
        args: serde_json::Value,
    },
    /// Tool call aprovada (auto-allow ou usuário).
    ToolCallApproved { agent: AgentId, id: String },
    /// Tool call negada (política ou usuário).
    ToolCallDenied {
        agent: AgentId,
        id: String,
        reason: String,
    },
    /// Tool call executada.
    ToolCallCompleted {
        agent: AgentId,
        id: String,
        is_error: bool,
    },
    /// Um agente foi criado (root ou subagente).
    AgentSpawned {
        agent: AgentId,
        parent: Option<AgentId>,
        label: String,
    },
    /// Um agente terminou com sucesso.
    AgentFinished { agent: AgentId },
    /// Um agente foi interrompido (usuário ou crash recovery).
    AgentInterrupted { agent: AgentId },
    /// Contexto injetado pelo usuário no próximo turno do agente.
    ContextInjected { agent: AgentId, text: String },
    /// Erro geral de domínio.
    Error { message: String },
}
