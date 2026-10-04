//! Red tests: navegação de telas confiável em terminais legacy (fix: Ctrl+2 ≠ Char('2')+Ctrl).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use harnizator_tui::state::{AppState, Screen};

fn k(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn tab_cycles_through_screens() {
    let mut s = AppState::new("m", "m");
    assert_eq!(s.screen, Screen::Chat);
    let _ = s.handle_key(k(KeyCode::Tab));
    assert_eq!(s.screen, Screen::Graph);
    let _ = s.handle_key(k(KeyCode::Tab));
    assert_eq!(s.screen, Screen::Sessions);
    let _ = s.handle_key(k(KeyCode::Tab));
    assert_eq!(s.screen, Screen::Providers);
    let _ = s.handle_key(k(KeyCode::Tab));
    assert_eq!(s.screen, Screen::Help);
    let _ = s.handle_key(k(KeyCode::Tab));
    assert_eq!(s.screen, Screen::Chat, "wrap de volta ao chat");
}

#[test]
fn shift_tab_cycles_backwards() {
    let mut s = AppState::new("m", "m");
    let _ = s.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
    assert_eq!(s.screen, Screen::Help);
    let _ = s.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
    assert_eq!(s.screen, Screen::Providers);
}

#[test]
fn f_keys_map_to_screens() {
    let mut s = AppState::new("m", "m");
    let _ = s.handle_key(k(KeyCode::F(2)));
    assert_eq!(s.screen, Screen::Graph);
    let _ = s.handle_key(k(KeyCode::F(4)));
    assert_eq!(s.screen, Screen::Sessions);
    let _ = s.handle_key(k(KeyCode::F(5)));
    assert_eq!(s.screen, Screen::Providers);
    let _ = s.handle_key(k(KeyCode::F(3)));
    assert_eq!(s.screen, Screen::Help);
    let _ = s.handle_key(k(KeyCode::F(1)));
    assert_eq!(s.screen, Screen::Chat);
}

#[test]
fn ctrl_g_opens_graph() {
    let mut s = AppState::new("m", "m");
    let _ = s.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));
    assert_eq!(s.screen, Screen::Graph);
}

#[test]
fn ctrl_2_legacy_null_also_opens_graph() {
    let mut s = AppState::new("m", "m");
    // em terminais legacy Ctrl+2 chega como NUL
    let _ = s.handle_key(k(KeyCode::Null));
    assert_eq!(s.screen, Screen::Graph);
}

#[test]
fn tab_does_not_capture_keys_inside_approval_modal() {
    let mut s = AppState::new("m", "m");
    s.apply_ui_msg(harnizator_tui::ui_msg::UiMsg::NeedsApproval {
        call: harnizator_core::tool_port::ToolCall {
            id: "c1".into(),
            name: "bash".into(),
            args: serde_json::json!({}),
        },
        reason: "r".into(),
    });
    let action = s.handle_key(k(KeyCode::Tab));
    assert_eq!(action, harnizator_tui::state::Action::None);
    assert_eq!(s.screen, Screen::Chat, "modal continua aberto");
}
