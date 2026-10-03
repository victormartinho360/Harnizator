//! Red tests: agent loop headless com tools reais + aprovação (spec/04, /09).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use harness_cli::{AutoApprove, run_agent_headless};
use harness_core::Message;
use harness_core::tool_port::{ApprovalDecision, ApprovalPort, ToolCall};
use harness_providers::{ChatRequest, MockProvider};
use harness_tools::{SandboxMode, ToolCtx, Toolbelt};

const SCENARIO: &str = r#"
[[response]]
when_contains = "read a.txt"

[[response.chunk]]
tool_use = { name = "read_file", args = { path = "a.txt" } }

[[response]]
when_contains = "magic-content"

[[response.chunk]]
text = "I read: "

[[response.chunk]]
text = "magic-content"
"#;

fn harness(dir: &tempfile::TempDir) -> (MockProvider, Toolbelt) {
    let provider = MockProvider::from_scenario_str(SCENARIO).unwrap();
    let belt = Toolbelt::new(ToolCtx::new(dir.path()).unwrap(), SandboxMode::FullAccess).unwrap();
    (provider, belt)
}

#[tokio::test]
async fn agent_executes_tool_and_streams_final_answer() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "magic-content\n").unwrap();
    let (provider, belt) = harness(&dir);

    let req = ChatRequest {
        model: "mock/test-model".into(),
        messages: vec![Message::user("please read a.txt")],
        max_tokens: 256,
        system: None,
        tools: vec![],
    };
    let mut out: Vec<u8> = vec![];
    let mut err: Vec<u8> = vec![];
    let summary = run_agent_headless(&provider, &belt, &AutoApprove, req, &mut out, &mut err)
        .await
        .unwrap();

    assert_eq!(summary.text, "I read: magic-content");
    let audit = String::from_utf8(err).unwrap();
    assert!(audit.contains("[tool] read_file requested"));
    assert!(audit.contains("approved"));
    assert!(audit.contains("completed (error=false)"));
}

struct DenyAll;
#[async_trait::async_trait]
impl ApprovalPort for DenyAll {
    async fn decide(&self, _call: &ToolCall, _reason: &str) -> ApprovalDecision {
        ApprovalDecision::Deny {
            reason: "headless policy".into(),
        }
    }
}

#[tokio::test]
async fn denied_tool_is_audited_and_never_runs() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "magic-content\n").unwrap();
    let (provider, belt) = harness(&dir);
    let req = ChatRequest {
        model: "mock/test-model".into(),
        messages: vec![Message::user("please read a.txt")],
        max_tokens: 256,
        system: None,
        tools: vec![],
    };
    let mut out: Vec<u8> = vec![];
    let mut err: Vec<u8> = vec![];
    // negação de read em FullAccess → o mock não casa o 2º turno (sem match):
    // loop deve logar negação e depois falhar com NoScenarioMatch upstream
    let _ = run_agent_headless(&provider, &belt, &DenyAll, req, &mut out, &mut err).await;
    let audit = String::from_utf8(err).unwrap();
    assert!(audit.contains("denied: headless policy"));
}
