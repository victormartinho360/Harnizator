//! Mensagens do loop runtime → reducer de estado.
//!
//! Contém apenas dados de domínio/UI abstratos: nenhum tipo ratatui (spec/09).

use harness_core::events::Event;
use harness_core::tool_port::ToolCall;

/// Atualização de UI vinda do mundo async (agent loop, approvals).
#[derive(Debug)]
pub enum UiMsg {
    /// Evento de domínio do core.
    Core(Event),
    /// O core precisa de aprovação para uma tool call.
    NeedsApproval { call: ToolCall, reason: String },
    /// A aprovação pendente foi resolvida (qualquer decisão).
    ApprovalResolved,
    /// O turno terminou (ok ou com erro formatado).
    TurnFinished(Result<(), String>),
    /// Histórico atualizado pelo agent loop (tool_use/tool_result incluídos).
    SyncHistory(Vec<harness_core::Message>, harness_core::TokenUsage),
}
