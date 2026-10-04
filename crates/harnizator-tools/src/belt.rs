//! Toolbelt: registry de tools + sandbox policy → implementa o port
//! `ToolPort` do core (spec/04).

use harnizator_core::tool_port::{ToolCall, ToolDecision, ToolOutcome, ToolPort, ToolSpec};

use crate::builtins::{BashTool, EditFileTool, GlobTool, GrepTool, ReadFileTool, WriteFileTool};
use crate::sandbox::SandboxMode;
use crate::{Tool, ToolCtx, ToolError};

/// Conjunto de tools + política de sandbox da sessão.
pub struct Toolbelt {
    ctx: ToolCtx,
    mode: SandboxMode,
    tools: Vec<Box<dyn Tool>>,
}

impl Toolbelt {
    /// Cria o toolbelt com os builtins padrão.
    pub fn new(ctx: ToolCtx, mode: SandboxMode) -> Result<Self, ToolError> {
        Ok(Self {
            ctx,
            mode,
            tools: vec![
                Box::new(ReadFileTool),
                Box::new(WriteFileTool),
                Box::new(EditFileTool),
                Box::new(BashTool),
                Box::new(GlobTool),
                Box::new(GrepTool),
            ],
        })
    }

    pub fn ctx(&self) -> &ToolCtx {
        &self.ctx
    }

    pub fn mode(&self) -> &SandboxMode {
        &self.mode
    }

    pub fn spec_names(&self) -> Vec<String> {
        self.tools.iter().map(|t| t.name().to_string()).collect()
    }

    fn find(&self, name: &str) -> Option<&dyn Tool> {
        self.tools
            .iter()
            .find(|t| t.name() == name)
            .map(|t| t.as_ref())
    }

    /// Decisão do sandbox (ver spec/04: matriz modo × risco × escape).
    pub fn decide(&self, call: &ToolCall) -> ToolDecision {
        let Some(tool) = self.find(&call.name) else {
            return ToolDecision::Deny {
                reason: format!("unknown tool: {}", call.name),
            };
        };
        // path escape trumps qualquer modo (defesa em profundidade)
        for p in tool.path_args(&call.args) {
            if let Err(e) = self.ctx.resolve(&p) {
                return ToolDecision::Deny {
                    reason: e.to_string(),
                };
            }
        }
        self.mode.decide(tool.risk(&call.args), call)
    }

    pub async fn execute_call(&self, call: &ToolCall) -> ToolOutcome {
        let Some(tool) = self.find(&call.name) else {
            return ToolOutcome {
                content: format!("error: unknown tool: {}", call.name),
                is_error: true,
            };
        };
        match tool.execute(call.args.clone(), &self.ctx).await {
            Ok(out) => {
                let is_error = out.is_error_flag();
                let mut content = out.content;
                if out.truncated {
                    content.push_str("\n[output truncated]");
                }
                if let Some(code) = out.exit_code {
                    content.push_str(&format!("\n[exit={code}]"));
                }
                ToolOutcome { content, is_error }
            }
            Err(e) => ToolOutcome {
                content: format!("error: {e}"),
                is_error: true,
            },
        }
    }
}

#[async_trait::async_trait]
impl ToolPort for Toolbelt {
    fn decide(&self, call: &ToolCall) -> ToolDecision {
        self.decide(call)
    }

    async fn execute(&self, call: &ToolCall) -> ToolOutcome {
        self.execute_call(call).await
    }

    fn specs(&self) -> Vec<ToolSpec> {
        self.tools
            .iter()
            .map(|t| ToolSpec {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: t.schema(),
            })
            .collect()
    }
}
