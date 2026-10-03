//! Red tests: agent loop no core com tool use + aprovação (spec/04).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Mutex;

use harness_core::agent_loop::{AgentLoop, LoopError};
use harness_core::events::Event;
use harness_core::provider_port::{
    ChatRequest, LlmProvider, ProviderError, StreamChunk, StreamResult,
};
use harness_core::tool_port::{
    ApprovalDecision, ApprovalPort, ToolCall, ToolDecision, ToolOutcome, ToolPort,
};
use harness_core::{ContentBlock, Message};

/// Provider falso scriptado (por turno, sem IO).
struct FakeProvider {
    responses: Mutex<std::collections::VecDeque<Vec<Result<StreamChunk, ProviderError>>>>,
}

impl FakeProvider {
    fn scripted(responses: Vec<Vec<Result<StreamChunk, ProviderError>>>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
        }
    }
}

#[async_trait::async_trait]
impl LlmProvider for FakeProvider {
    fn id(&self) -> &str {
        "fake"
    }
    async fn stream(&self, _req: ChatRequest) -> Result<StreamResult, ProviderError> {
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

/// Executor falso que registra chamadas.
struct FakeTools {
    calls: Mutex<Vec<ToolCall>>,
    decision: ToolDecision,
    output: String,
}

#[async_trait::async_trait]
impl ToolPort for FakeTools {
    fn decide(&self, _call: &ToolCall) -> ToolDecision {
        self.decision.clone()
    }
    async fn execute(&self, call: &ToolCall) -> ToolOutcome {
        self.calls.lock().unwrap().push(call.clone());
        ToolOutcome {
            content: self.output.clone(),
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

struct DenyAll;
#[async_trait::async_trait]
impl ApprovalPort for DenyAll {
    async fn decide(&self, _call: &ToolCall, _reason: &str) -> ApprovalDecision {
        ApprovalDecision::Deny {
            reason: "user denied".into(),
        }
    }
}

fn text_reply(text: &str) -> Vec<Result<StreamChunk, ProviderError>> {
    vec![
        Ok(StreamChunk::MessageStart),
        Ok(StreamChunk::TextDelta(text.into())),
        Ok(StreamChunk::Usage {
            input: 1,
            output: 1,
        }),
        Ok(StreamChunk::MessageStop),
    ]
}

fn tool_reply(name: &str) -> Vec<Result<StreamChunk, ProviderError>> {
    vec![
        Ok(StreamChunk::MessageStart),
        Ok(StreamChunk::ToolUse(ToolCall {
            id: "call-1".into(),
            name: name.into(),
            args: serde_json::json!({"path": "a.txt"}),
        })),
        Ok(StreamChunk::MessageStop),
    ]
}

fn req() -> ChatRequest {
    ChatRequest {
        model: "fake".into(),
        messages: vec![Message::user("do it")],
        max_tokens: 128,
        system: None,
        tools: vec![],
    }
}

#[tokio::test]
async fn plain_text_turn_has_no_tool_events() {
    let provider = FakeProvider::scripted(vec![text_reply("done")]);
    let tools = FakeTools {
        calls: Mutex::new(vec![]),
        decision: ToolDecision::Allow,
        output: String::new(),
    };
    let run = AgentLoop::new(&provider, &tools, &ApproveAll)
        .run(req())
        .await
        .unwrap();
    assert_eq!(run.outcome.text, "done");
    assert!(tools.calls.lock().unwrap().is_empty());
    assert!(
        run.events
            .iter()
            .all(|e| !matches!(e, Event::ToolCallRequested { .. }))
    );
}

#[tokio::test]
async fn tool_call_is_executed_approved_and_audited() {
    let provider = FakeProvider::scripted(vec![tool_reply("read_file"), text_reply("I read it")]);
    let tools = FakeTools {
        calls: Mutex::new(vec![]),
        decision: ToolDecision::NeedsApproval {
            reason: "writes".into(),
        },
        output: "file contents".into(),
    };
    let run = AgentLoop::new(&provider, &tools, &ApproveAll)
        .run(req())
        .await
        .unwrap();

    assert_eq!(run.outcome.text, "I read it");
    let calls = tools.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "read_file");

    // auditoria completa: Requested → Approved → Completed
    let seq: Vec<String> = run
        .events
        .iter()
        .filter_map(|e| match e {
            Event::ToolCallRequested { .. } => Some("requested"),
            Event::ToolCallApproved { .. } => Some("approved"),
            Event::ToolCallCompleted { .. } => Some("completed"),
            _ => None,
        })
        .map(String::from)
        .collect();
    assert_eq!(seq, vec!["requested", "approved", "completed"]);

    // tool_result entra como mensagem de usuário para o próximo turno
    let tool_result_msg = run
        .messages
        .iter()
        .find(|m| {
            m.content
                .iter()
                .any(|b| matches!(b, ContentBlock::ToolResult { .. }))
        })
        .unwrap();
    match &tool_result_msg.content[0] {
        ContentBlock::ToolResult {
            content, is_error, ..
        } => {
            assert_eq!(content, "file contents");
            assert!(!is_error);
        }
        _ => panic!("expected tool_result"),
    }
}

#[tokio::test]
async fn denied_tool_call_never_executes() {
    let provider = FakeProvider::scripted(vec![tool_reply("bash"), text_reply("ok denied")]);
    let tools = FakeTools {
        calls: Mutex::new(vec![]),
        decision: ToolDecision::NeedsApproval {
            reason: "exec".into(),
        },
        output: "should not appear".into(),
    };
    let run = AgentLoop::new(&provider, &tools, &DenyAll)
        .run(req())
        .await
        .unwrap();

    assert!(tools.calls.lock().unwrap().is_empty());
    assert!(
        run.events
            .iter()
            .any(|e| matches!(e, Event::ToolCallDenied { .. }))
    );
    // o LLM recebe tool_result marcado como erro/negação
    let tr = run
        .messages
        .iter()
        .flat_map(|m| &m.content)
        .find_map(|b| match b {
            ContentBlock::ToolResult {
                content, is_error, ..
            } => Some((content.clone(), *is_error)),
            _ => None,
        })
        .unwrap();
    assert!(tr.1, "denial deve ser is_error");
    assert!(tr.0.contains("denied"));
}

#[tokio::test]
async fn sandbox_deny_from_decide_skips_approval() {
    let provider = FakeProvider::scripted(vec![tool_reply("bash"), text_reply("fine")]);
    let tools = FakeTools {
        calls: Mutex::new(vec![]),
        decision: ToolDecision::Deny {
            reason: "read-only mode".into(),
        },
        output: "nope".into(),
    };
    // ApproveAll NÃO deve ser consultado: Deny do sandbox é final
    let run = AgentLoop::new(&provider, &tools, &ApproveAll)
        .run(req())
        .await
        .unwrap();
    assert!(tools.calls.lock().unwrap().is_empty());
    assert!(run.events.iter().any(
        |e| matches!(e, Event::ToolCallDenied { reason, .. } if reason.contains("read-only"))
    ));
}

#[tokio::test]
async fn infinite_tool_loop_hits_max_turns() {
    let provider = FakeProvider::scripted(vec![tool_reply("bash"); 10]);
    let tools = FakeTools {
        calls: Mutex::new(vec![]),
        decision: ToolDecision::Allow,
        output: "ok".into(),
    };
    let err = AgentLoop::new(&provider, &tools, &ApproveAll)
        .with_max_turns(3)
        .run(req())
        .await
        .unwrap_err();
    assert!(matches!(err, LoopError::MaxTurns(3)));
}
