//! Modo de sandbox e decisão de política (spec/04).

use harness_core::tool_port::{ToolCall, ToolDecision};

use crate::Risk;
use crate::flags::FlagPolicy;

/// Modo de sandbox da sessão.
#[derive(Debug, Clone)]
pub enum SandboxMode {
    /// Nega Write/Execute.
    ReadOnly,
    /// Permite Read+Write, nega Execute.
    WriteOnly,
    /// Execute só se o comando casa a flaglist.
    OnlyFlagged(FlagPolicy),
    /// Tudo permitido, mas sempre com aprovação interativa.
    FullAccess,
}

/// Operadores que tornam um comando composto: nunca auto-aprovável.
const COMPOUND_TOKENS: &[&str] = &["&&", "||", ";", "|", "`", "$(", ">", "<"];

/// O comando contém composição de shell?
pub fn is_compound(command: &str) -> bool {
    COMPOUND_TOKENS.iter().any(|t| command.contains(t))
}

impl SandboxMode {
    /// Decisão do modo para (risk, call). Checagem de path é feita antes,
    /// pelo Toolbelt; aqui entra apenas a matriz modo × risco.
    pub fn decide(&self, risk: Risk, call: &ToolCall) -> ToolDecision {
        match self {
            SandboxMode::ReadOnly => match risk {
                Risk::Read => ToolDecision::Allow,
                _ => ToolDecision::Deny {
                    reason: "read-only mode: writes/exec negados".into(),
                },
            },
            SandboxMode::WriteOnly => match risk {
                Risk::Read | Risk::Write => ToolDecision::Allow,
                Risk::Execute => ToolDecision::Deny {
                    reason: "write-only mode: exec negado".into(),
                },
            },
            SandboxMode::FullAccess => ToolDecision::NeedsApproval {
                reason: "full-access: aprovação interativa obrigatória".into(),
            },
            SandboxMode::OnlyFlagged(flags) => match risk {
                Risk::Read | Risk::Write => ToolDecision::Allow,
                Risk::Execute => {
                    let cmd = call.args["command"].as_str().unwrap_or_default();
                    if is_compound(cmd) {
                        ToolDecision::NeedsApproval {
                            reason: "comando composto nunca é auto-aprovado".into(),
                        }
                    } else if flags.is_flagged(cmd) {
                        ToolDecision::Allow
                    } else {
                        ToolDecision::NeedsApproval {
                            reason: format!("comando não flagado: {cmd}"),
                        }
                    }
                }
            },
        }
    }
}
