//! Mensagens do loop runtime → reducer de estado.
//!
//! Contém apenas dados de domínio/UI abstratos: nenhum tipo ratatui (spec/09).

use harness_core::events::Event;
use harness_core::tool_port::ToolCall;

/// View de um provider na tela Providers (espelha `core::provider_admin::ProviderConfigView`).
pub type ProviderView = harness_core::provider_admin::ProviderConfigView;

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
    /// Snapshot do grafo de agentes para a tela Graph.
    GraphSync(Vec<harness_core::agents::NodeView>),
    /// Lista de sessões persistidas para a tela Sessions.
    SessionsSync(Vec<harness_core::store_port::SessionMeta>),
    /// Lista de providers para a tela Providers.
    ProvidersSync(Vec<ProviderView>),
    /// Resultado do teste de conexão de um provider.
    ProviderTestResult {
        id: String,
        result: Result<String, String>,
    },
    /// Lista de modelos de um provider (para o picker do command palette).
    ModelsSync {
        provider: String,
        result: Result<Vec<String>, String>,
    },
}
