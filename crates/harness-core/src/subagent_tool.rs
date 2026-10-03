//! ToolPort wrapper que adiciona a tool builtin `spawn_agent` (spec/06).

use std::sync::{Arc, Mutex};

use crate::AgentId;
use crate::agents::{AgentManager, SpawnError};
use crate::tool_port::{ToolCall, ToolDecision, ToolOutcome, ToolPort, ToolSpec};

/// Adiciona `spawn_agent` sobre um ToolPort interno (sandbox/policy dele
/// continuam valendo para as demais tools).
pub struct SpawnToolPort {
    inner: Arc<dyn ToolPort>,
    manager: Arc<Mutex<AgentManager>>,
    parent: AgentId,
}

impl SpawnToolPort {
    pub fn new(
        inner: Arc<dyn ToolPort>,
        manager: Arc<Mutex<AgentManager>>,
        parent: AgentId,
    ) -> Self {
        Self {
            inner,
            manager,
            parent,
        }
    }
}

#[async_trait::async_trait]
impl ToolPort for SpawnToolPort {
    fn decide(&self, call: &ToolCall) -> ToolDecision {
        if call.name == "spawn_agent" {
            return ToolDecision::NeedsApproval {
                reason: "spawn_agent cria um subagente (execução concorrente)".into(),
            };
        }
        self.inner.decide(call)
    }

    async fn execute(&self, call: &ToolCall) -> ToolOutcome {
        if call.name != "spawn_agent" {
            return self.inner.execute(call).await;
        }
        let label = call.args["label"].as_str().unwrap_or("agent").to_string();
        let prompt = call.args["prompt"].as_str().unwrap_or_default().to_string();
        let mut manager = match self.manager.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        match manager.spawn(Some(&self.parent), label.clone(), prompt) {
            Ok(id) => ToolOutcome {
                content: format!("spawned {} ({})", id, label),
                is_error: false,
            },
            Err(SpawnError::MaxDepth) => ToolOutcome {
                content: "error: max agent depth reached".into(),
                is_error: true,
            },
            Err(e) => ToolOutcome {
                content: format!("error: {e}"),
                is_error: true,
            },
        }
    }

    fn specs(&self) -> Vec<ToolSpec> {
        let mut specs = self.inner.specs();
        specs.push(ToolSpec {
            name: "spawn_agent".to_string(),
            description: "Spawn a subagent with a label and an initial prompt. It runs concurrently under this agent.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "label": {"type": "string"},
                    "prompt": {"type": "string"},
                },
                "required": ["label", "prompt"],
            }),
        });
        specs
    }
}
