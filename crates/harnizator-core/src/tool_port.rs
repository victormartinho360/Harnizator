//! Port de tools e de aprovação (vive no core; adapters em harnizator-tools e nas UIs).

/// ToolCall completa vinda do LLM.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: serde_json::Value,
}

/// Schema de uma tool anunciada ao modelo.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Resultado da execução de uma tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolOutcome {
    pub content: String,
    pub is_error: bool,
}

/// Decisão do sandbox para uma chamada (spec/04).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolDecision {
    /// Executa sem aprovação.
    Allow,
    /// Precisa de aprovação interativa.
    NeedsApproval { reason: String },
    /// Negado por política; a aprovação nem é consultada.
    Deny { reason: String },
}

/// Port de ferramentas consumido pelo agent loop.
#[async_trait::async_trait]
pub trait ToolPort: Send + Sync {
    /// Decisão de política do sandbox para a chamada.
    fn decide(&self, call: &ToolCall) -> ToolDecision;

    /// Executa a tool (pré-condição: `decide` retornou Allow ou aprovação).
    async fn execute(&self, call: &ToolCall) -> ToolOutcome;

    /// Schemas anunciados ao LLM.
    fn specs(&self) -> Vec<ToolSpec> {
        Vec::new()
    }
}

/// Decisão do usuário (ou de uma UI headless) diante de `NeedsApproval`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    Approve,
    /// Aprova e adiciona à allowlist da sessão (Wave 3+).
    ApproveAndAllowlist,
    Deny {
        reason: String,
    },
}

/// Port de aprovação: o core emite a necessidade; a UI decide como perguntar.
#[async_trait::async_trait]
pub trait ApprovalPort: Send + Sync {
    async fn decide(&self, call: &ToolCall, reason: &str) -> ApprovalDecision;
}
