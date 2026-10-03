//! Red tests: spawn_agent tool + serviço persistente com context injection (spec/06).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};

use futures::channel::mpsc;
use harness_core::agents::AgentManager;
use harness_core::events::Event;
use harness_core::provider_port::{
    ChatRequest, LlmProvider, ProviderError, StreamChunk, StreamResult,
};
use harness_core::service::{ServiceCtx, run_agent_service};
use harness_core::subagent_tool::SpawnToolPort;
use harness_core::tool_port::{
    ApprovalDecision, ApprovalPort, ToolCall, ToolDecision, ToolOutcome, ToolPort,
};
use harness_core::{AgentId, Message};

// ---------- helpers compartilhados ----------

struct RecordingProvider {
    responses: Mutex<std::collections::VecDeque<Vec<Result<StreamChunk, ProviderError>>>>,
    requests: Mutex<Vec<ChatRequest>>,
}

impl RecordingProvider {
    fn scripted(responses: Vec<Vec<Result<StreamChunk, ProviderError>>>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
            requests: Mutex::new(vec![]),
        }
    }
}

#[async_trait::async_trait]
impl LlmProvider for RecordingProvider {
    fn id(&self) -> &str {
        "rec"
    }
    async fn stream(&self, req: ChatRequest) -> Result<StreamResult, ProviderError> {
        self.requests.lock().unwrap().push(req);
        let items = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("no scripted response left");
        Ok(Box::pin(futures::stream::iter(items)))
    }
    async fn models(&self) -> Result<Vec<harness_core::provider_port::ModelInfo>, ProviderError> {
        Ok(vec![])
    }
}

struct AllowAllTools;

#[async_trait::async_trait]
impl ToolPort for AllowAllTools {
    fn decide(&self, _call: &ToolCall) -> ToolDecision {
        ToolDecision::Allow
    }
    async fn execute(&self, call: &ToolCall) -> ToolOutcome {
        ToolOutcome {
            content: format!("ran {}", call.name),
            is_error: false,
        }
    }
}

struct ApproveAll;

#[async_trait::async_trait]
impl ApprovalPort for ApproveAll {
    async fn decide(&self, _call: &ToolCall, _reason: &str) -> ApprovalDecision {
        ApprovalDecision::Approve
    }
}

fn text_reply(text: &str) -> Vec<Result<StreamChunk, ProviderError>> {
    vec![
        Ok(StreamChunk::MessageStart),
        Ok(StreamChunk::TextDelta(text.into())),
        Ok(StreamChunk::MessageStop),
    ]
}

// ---------- spawn_agent tool ----------

#[test]
fn spawn_agent_spec_is_announced() {
    let manager = Arc::new(Mutex::new(AgentManager::new(8, 4)));
    let port = SpawnToolPort::new(Arc::new(AllowAllTools), manager, AgentId::new("root"));
    let names: Vec<_> = port.specs().iter().map(|s| s.name.clone()).collect();
    assert!(names.contains(&"spawn_agent".to_string()));
}

#[test]
fn spawn_agent_always_needs_approval() {
    let manager = Arc::new(Mutex::new(AgentManager::new(8, 4)));
    let port = SpawnToolPort::new(Arc::new(AllowAllTools), manager, AgentId::new("root"));
    let call = ToolCall {
        id: "t1".into(),
        name: "spawn_agent".into(),
        args: serde_json::json!({"label": "researcher", "prompt": "investigate x"}),
    };
    assert!(matches!(
        port.decide(&call),
        ToolDecision::NeedsApproval { .. }
    ));
}

#[tokio::test]
async fn spawn_agent_execute_registers_child_under_parent() {
    let manager = Arc::new(Mutex::new(AgentManager::new(8, 4)));
    let root = manager.lock().unwrap().root_id();
    let port = SpawnToolPort::new(Arc::new(AllowAllTools), manager.clone(), root.clone());
    let call = ToolCall {
        id: "t1".into(),
        name: "spawn_agent".into(),
        args: serde_json::json!({"label": "researcher", "prompt": "investigate x"}),
    };
    let outcome = port.execute(&call).await;
    assert!(!outcome.is_error);
    let nodes = manager.lock().unwrap().snapshot();
    let child = nodes.iter().find(|n| n.label == "researcher").unwrap();
    assert_eq!(child.parent, Some(root));
    assert!(
        outcome.content.contains("agent-1"),
        "outcome: {}",
        outcome.content
    );
}

#[tokio::test]
async fn spawn_tool_delegates_other_tools_to_inner() {
    let manager = Arc::new(Mutex::new(AgentManager::new(8, 4)));
    let root = manager.lock().unwrap().root_id();
    let port = SpawnToolPort::new(Arc::new(AllowAllTools), manager, root);
    let call = ToolCall {
        id: "t2".into(),
        name: "read_file".into(),
        args: serde_json::json!({}),
    };
    let outcome = port.execute(&call).await;
    assert_eq!(outcome.content, "ran read_file");
}

// ---------- serviço persistente + context injection ----------

#[tokio::test]
async fn injected_context_enters_next_turn() {
    // turno 1: responde; turno 2 (após injeção): responde
    let provider = RecordingProvider::scripted(vec![text_reply("first"), text_reply("second")]);
    let manager = Arc::new(Mutex::new(AgentManager::new(8, 4)));
    let agent = manager.lock().unwrap().root_id();

    let (tx, mut rx) = mpsc::unbounded::<String>();
    tx.unbounded_send("contexto injetado".to_string()).unwrap();
    drop(tx); // canal fecha: após consumir a injeção, o serviço termina idle

    let mut events: Vec<Event> = vec![];
    let result = run_agent_service(
        ServiceCtx {
            provider: &provider,
            tools: &AllowAllTools,
            approver: &ApproveAll,
            manager: manager.clone(),
            agent: agent.clone(),
            model: "model-x".to_string(),
        },
        vec![Message::user("initial question")],
        &mut rx,
        &mut |e| events.push(e.clone()),
    )
    .await
    .unwrap();

    assert_eq!(result.final_text, "second");
    // injeção foi auditada e apareceu na request do turno seguinte
    assert!(
        events.iter().any(
            |e| matches!(e, Event::ContextInjected { text, .. } if text == "contexto injetado")
        )
    );
    let reqs = provider.requests.lock().unwrap();
    assert_eq!(reqs.len(), 2);
    let second_turn = &reqs[1];
    assert!(
        second_turn
            .messages
            .iter()
            .any(|m| m.text().contains("contexto injetado")),
        "contexto injetado entra como mensagem: {:?}",
        second_turn.messages
    );
}

#[tokio::test]
async fn service_without_injection_answers_initial_prompt() {
    let provider = RecordingProvider::scripted(vec![text_reply("only")]);
    let manager = Arc::new(Mutex::new(AgentManager::new(8, 4)));
    let agent = manager.lock().unwrap().root_id();
    let (tx, mut rx) = mpsc::unbounded::<String>();
    drop(tx);
    let result = run_agent_service(
        ServiceCtx {
            provider: &provider,
            tools: &AllowAllTools,
            approver: &ApproveAll,
            manager,
            agent,
            model: "model-x".to_string(),
        },
        vec![Message::user("q")],
        &mut rx,
        &mut |_| {},
    )
    .await
    .unwrap();
    assert_eq!(result.final_text, "only");
}
