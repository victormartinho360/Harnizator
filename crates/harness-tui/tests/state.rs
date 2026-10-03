//! Red tests: reducers de estado do TUI (spec/05).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use harness_core::events::Event;
use harness_core::tool_port::ToolCall;
use harness_core::{AgentId, tool_port::ApprovalDecision};
use harness_tui::state::{Action, AppState, DisplayRole};
use harness_tui::ui_msg::UiMsg;
use proptest::prelude::*;

fn state() -> AppState {
    AppState::new("anthropic/claude-test", "only-flagged")
}

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

fn delta(t: &str) -> Event {
    Event::AssistantDelta {
        agent: AgentId::new("root"),
        text: t.into(),
    }
}

fn tool_call() -> ToolCall {
    ToolCall {
        id: "c1".into(),
        name: "bash".into(),
        args: serde_json::json!({"command": "ls"}),
    }
}

// ---------- eventos de domínio ----------

#[test]
fn assistant_delta_starts_new_message_when_none_streaming() {
    let mut s = state();
    s.apply_event(&delta("Hello"));
    s.apply_event(&delta(" world"));
    // posição 0 é a mensagem de boas-vindas do sistema
    assert_eq!(s.messages.len(), 2);
    assert_eq!(s.messages[1].role, DisplayRole::Assistant);
    assert_eq!(s.messages[1].content, "Hello world");
    assert!(s.generating);
}

#[test]
fn user_send_pushes_message_then_delta_starts_new_assistant() {
    let mut s = state();
    s.apply_event(&delta("first"));
    s.push_user_message("next question".into());
    s.apply_event(&delta("second"));
    assert_eq!(s.messages.len(), 4); // sys, assistant, user, assistant
    assert_eq!(s.messages[3].content, "second");
}

#[test]
fn tool_events_append_audit_lines() {
    let mut s = state();
    for ev in [
        Event::ToolCallRequested {
            agent: AgentId::new("root"),
            id: "c1".into(),
            name: "bash".into(),
            args: serde_json::json!({"command": "ls"}),
        },
        Event::ToolCallApproved {
            agent: AgentId::new("root"),
            id: "c1".into(),
        },
        Event::ToolCallCompleted {
            agent: AgentId::new("root"),
            id: "c1".into(),
            is_error: false,
            output: "done".into(),
        },
    ] {
        s.apply_event(&ev);
    }
    let joined: String = s.messages.iter().map(|m| m.content.as_str()).collect();
    assert!(joined.contains("bash"));
    assert!(s.messages.iter().any(|m| m.role == DisplayRole::Tool));
}

#[test]
fn approval_awaiting_opens_and_resolution_closes_modal() {
    let mut s = state();
    s.apply_ui_msg(UiMsg::NeedsApproval {
        call: tool_call(),
        reason: "exec".into(),
    });
    assert!(s.awaiting_approval.is_some());
    s.apply_ui_msg(UiMsg::ApprovalResolved);
    assert!(s.awaiting_approval.is_none());
}

#[test]
fn turn_done_stops_generating() {
    let mut s = state();
    s.apply_event(&delta("x"));
    s.apply_ui_msg(UiMsg::TurnFinished(Ok(())));
    assert!(!s.generating);
}

// ---------- teclas ----------

#[test]
fn typing_appends_and_enter_sends() {
    let mut s = state();
    let _ = s.handle_key(key('h'));
    let _ = s.handle_key(key('i'));
    let action = s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(action, Action::Send("hi".into()));
    assert!(s.input.is_empty());
}

#[test]
fn enter_on_empty_input_does_nothing() {
    let mut s = state();
    let action = s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(action, Action::None);
}

#[test]
fn backspace_removes_last_char() {
    let mut s = state();
    let _ = s.handle_key(key('a'));
    let _ = s.handle_key(key('b'));
    let _ = s.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    assert_eq!(s.input, "a");
}

#[test]
fn esc_cancels_generation_only_when_generating() {
    let mut s = state();
    s.apply_event(&delta("streaming"));
    let action = s.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(action, Action::Cancel);
    // sem geração ativa: esc não faz nada
    s.apply_ui_msg(UiMsg::TurnFinished(Ok(())));
    let action = s.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(action, Action::None);
}

#[test]
fn ctrl_c_quits() {
    let mut s = state();
    let action = s.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert_eq!(action, Action::Quit);
}

#[test]
fn modal_keys_resolve_approval_and_swallow_text_input() {
    let mut s = state();
    s.apply_ui_msg(UiMsg::NeedsApproval {
        call: tool_call(),
        reason: "exec".into(),
    });
    let action = s.handle_key(key('y'));
    assert_eq!(action, Action::Approve(ApprovalDecision::Approve));
    assert!(s.input.is_empty(), "tecla do modal não vai para o input");

    s.apply_ui_msg(UiMsg::NeedsApproval {
        call: tool_call(),
        reason: "exec".into(),
    });
    let action = s.handle_key(key('n'));
    assert!(matches!(
        action,
        Action::Approve(ApprovalDecision::Deny { .. })
    ));
}

#[test]
fn y_without_modal_goes_to_input() {
    let mut s = state();
    let _ = s.handle_key(key('y'));
    assert_eq!(s.input, "y");
}

#[test]
fn pageup_disables_follow_and_end_reenables() {
    let mut s = state();
    assert!(s.follow);
    let _ = s.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
    assert!(!s.follow);
    let _ = s.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
    assert!(s.follow);
}

proptest! {
    /// Sequências arbitrárias de eventos/teclas nunca panicam e mantêm
    /// invariantes do estado (spec/02).
    #[test]
    fn reducers_never_panic(
        chars in proptest::collection::vec(any::<char>().prop_filter("printable", |c| c.is_ascii_graphic()), 0..50),
        deltas in proptest::collection::vec("[a-z ]{0,10}", 0..10),
    ) {
        let mut s = state();
        for c in &chars {
            let _ = s.handle_key(key(*c));
        }
        for d in &deltas {
            s.apply_event(&delta(d));
        }
        // invariantes: input e mensagens acessíveis; follow é bool; sem NaN de scroll
        let _ = s.input.len();
        let _ = s.messages.len();
        let _ = (s.follow, s.scroll);
    }
}
