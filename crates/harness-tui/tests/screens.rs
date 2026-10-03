//! Red tests: snapshots de tela via TestBackend (spec/05).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use harness_core::AgentId;
use harness_core::events::Event;
use harness_core::tool_port::ToolCall;
use harness_tui::state::AppState;
use harness_tui::ui;
use harness_tui::ui_msg::UiMsg;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn render(state: &AppState, w: u16, h: u16) -> String {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| ui::draw(f, state)).unwrap();
    format!("{}", term.backend())
}

#[test]
fn empty_chat_80x24() {
    let s = AppState::new("anthropic/claude-test", "only-flagged");
    insta::assert_snapshot!(render(&s, 80, 24));
}

#[test]
fn streaming_chat_80x24() {
    let mut s = AppState::new("anthropic/claude-test", "only-flagged");
    s.push_user_message("how do I exit vim?".into());
    for d in ["Press ", "**Esc**", " then `:q!`"] {
        s.apply_event(&Event::AssistantDelta {
            agent: AgentId::new("root"),
            text: d.into(),
        });
    }
    insta::assert_snapshot!(render(&s, 80, 24));
}

#[test]
fn approval_modal_80x24() {
    let mut s = AppState::new("anthropic/claude-test", "full-access");
    s.push_user_message("clean up".into());
    s.apply_event(&Event::ToolCallRequested {
        agent: AgentId::new("root"),
        id: "c1".into(),
        name: "bash".into(),
        args: serde_json::json!({"command": "rm -rf target/"}),
    });
    s.apply_ui_msg(UiMsg::NeedsApproval {
        call: ToolCall {
            id: "c1".into(),
            name: "bash".into(),
            args: serde_json::json!({"command": "rm -rf target/"}),
        },
        reason: "full-access: aprovação interativa obrigatória".into(),
    });
    insta::assert_snapshot!(render(&s, 80, 24));
}

#[test]
fn empty_chat_120x40() {
    let s = AppState::new("openai/gpt-test", "read-only");
    insta::assert_snapshot!(render(&s, 120, 40));
}

#[test]
fn degenerate_sizes_never_panic() {
    let s = AppState::new("m", "s");
    for (w, h) in [(0, 0), (1, 1), (2, 2), (10, 3)] {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        let _ = term.draw(|f| ui::draw(f, &s));
    }
}
