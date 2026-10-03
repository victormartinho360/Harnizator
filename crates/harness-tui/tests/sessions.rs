//! Red tests: tela Sessions (Ctrl+4) — lista, navegação, resume (spec/07).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use harness_core::TokenUsage;
use harness_core::store_port::SessionMeta;
use harness_tui::state::{Action, AppState, Screen};
use harness_tui::ui;
use harness_tui::ui_msg::UiMsg;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn meta(id: &str, title: &str, input: u64, output: u64) -> SessionMeta {
    SessionMeta {
        id: id.into(),
        title: title.into(),
        model: "m".into(),
        sandbox_mode: "only-flagged".into(),
        created_at: 1_700_000_000,
        updated_at: 1_700_000_100,
        usage: TokenUsage { input, output },
    }
}

fn sessions_state() -> AppState {
    let mut s = AppState::new("m", "only-flagged");
    s.apply_ui_msg(UiMsg::SessionsSync(vec![
        meta("s-bbb", "second", 5, 2),
        meta("s-aaa", "first", 10, 3),
    ]));
    s.screen = Screen::Sessions;
    s
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

#[test]
fn ctrl_4_opens_sessions_screen() {
    let mut s = AppState::new("m", "m");
    let _ = s.handle_key(ctrl('4'));
    assert_eq!(s.screen, Screen::Sessions);
}

#[test]
fn sessions_sync_populates_and_selects_first() {
    let s = sessions_state();
    assert_eq!(s.sessions.len(), 2);
    assert_eq!(s.sessions_selected, 0);
}

#[test]
fn enter_resumes_selected_session() {
    let mut s = sessions_state();
    let _ = s.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    let action = s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(action, Action::ResumeSession("s-aaa".into()));
    assert_eq!(s.screen, Screen::Chat);
}

#[test]
fn esc_goes_back_to_chat_without_resume() {
    let mut s = sessions_state();
    let _ = s.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(s.screen, Screen::Chat);
}

#[test]
fn sessions_screen_snapshot() {
    let s = sessions_state();
    let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    term.draw(|f| ui::draw(f, &s)).unwrap();
    insta::assert_snapshot!(format!("{}", term.backend()));
}
