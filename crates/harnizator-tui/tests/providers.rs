//! Red tests: tela Providers (Ctrl+5) — CRUD + teste de conexão (spec/03/06-wave6).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use harnizator_tui::state::{Action, AppState, InputMode, Screen};
use harnizator_tui::ui;
use harnizator_tui::ui_msg::{ProviderView, UiMsg};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn view(id: &str, configured: bool) -> ProviderView {
    ProviderView {
        id: id.into(),
        kind: "anthropic".into(),
        base_url: "https://api.anthropic.com/v1".into(),
        configured,
    }
}

fn providers_state() -> AppState {
    let mut s = AppState::new("m", "only-flagged");
    s.apply_ui_msg(UiMsg::ProvidersSync(vec![
        view("anthropic", true),
        view("openai", false),
        view("google", false),
    ]));
    s.screen = Screen::Providers;
    s
}

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

#[test]
fn ctrl_5_opens_providers_screen() {
    let mut s = AppState::new("m", "m");
    let _ = s.handle_key(ctrl('5'));
    assert_eq!(s.screen, Screen::Providers);
}

#[test]
fn providers_sync_marks_configured() {
    let s = providers_state();
    assert_eq!(s.providers.len(), 3);
    assert!(s.providers[0].configured);
    assert!(!s.providers[1].configured);
}

#[test]
fn k_opens_masked_key_input_and_enter_submits() {
    let mut s = providers_state();
    let _ = s.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)); // openai
    let _ = s.handle_key(key('k'));
    assert_eq!(s.input_mode, InputMode::SetKey("openai".into()));
    for c in "sk-live-x".chars() {
        let _ = s.handle_key(key(c));
    }
    let action = s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(
        action,
        Action::SetProviderKey("openai".into(), "sk-live-x".into())
    );
    assert_eq!(s.input_mode, InputMode::Chat);
    assert!(s.input.is_empty());
}

#[test]
fn masked_input_shows_bullets_not_secrets() {
    let mut s = providers_state();
    let _ = s.handle_key(key('k'));
    for c in "abc".chars() {
        let _ = s.handle_key(key(c));
    }
    let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    term.draw(|f| ui::draw(f, &s)).unwrap();
    let screen = format!("{}", term.backend());
    assert!(screen.contains("•••"));
    assert!(!screen.contains("abc"));
}

#[test]
fn a_walks_add_provider_form() {
    let mut s = providers_state();
    let _ = s.handle_key(key('a'));
    assert_eq!(s.input_mode, InputMode::AddProviderName);
    let _ = s.handle_key(key('m'));
    let _ = s.handle_key(key('e'));
    let _ = s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(s.input_mode, InputMode::AddProviderKind { .. }));
    // kind step: default openai-compatible
    let _ = s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(s.input_mode, InputMode::AddProviderBaseUrl { .. }));
    for c in "http://x:9/v1".chars() {
        let _ = s.handle_key(key(c));
    }
    let action = s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(action, Action::AddProvider { .. }));
}

#[test]
fn t_triggers_connection_test() {
    let mut s = providers_state();
    let action = s.handle_key(key('t'));
    assert_eq!(action, Action::TestConnection("anthropic".into()));
}

#[test]
fn d_removes_selected_provider() {
    let mut s = providers_state();
    let action = s.handle_key(key('d'));
    assert_eq!(action, Action::RemoveProvider("anthropic".into()));
}

#[test]
fn providers_screen_snapshot() {
    let s = providers_state();
    let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    term.draw(|f| ui::draw(f, &s)).unwrap();
    insta::assert_snapshot!(format!("{}", term.backend()));
}
