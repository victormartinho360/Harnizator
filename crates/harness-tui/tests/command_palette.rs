//! Red tests: Command Palette (Ctrl+P) — fuzzy search providers/models + app commands (spec/06).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use harness_tui::state::{Action, AppState, InputMode, Screen};
use harness_tui::ui_msg::{ProviderView, UiMsg};

fn providers_state_with_model() -> AppState {
    let mut s = AppState::new("anthropic/claude-sonnet-4-5", "only-flagged");
    s.apply_ui_msg(UiMsg::ProvidersSync(vec![
        ProviderView { id: "anthropic".into(), kind: "anthropic".into(), base_url: "https://api.anthropic.com/v1".into(), configured: true },
        ProviderView { id: "openai".into(), kind: "openai".into(), base_url: "https://api.openai.com/v1".into(), configured: true },
        ProviderView { id: "google".into(), kind: "google".into(), base_url: "https://generativelanguage.googleapis.com/v1beta/openai".into(), configured: false },
        ProviderView { id: "meu-nim".into(), kind: "nim".into(), base_url: "https://integrate.api.nvidia.com/v1".into(), configured: true },
    ]));
    s
}

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

#[test]
fn ctrl_p_opens_command_palette() {
    let mut s = AppState::new("anthropic/claude-sonnet-4-5", "only-flagged");
    let action = s.handle_key(ctrl('p'));
    assert_eq!(s.screen, Screen::CommandPalette);
    assert_eq!(action, Action::None);
}

#[test]
fn colon_opens_command_palette() {
    let mut s = AppState::new("anthropic/claude-sonnet-4-5", "only-flagged");
    let action = s.handle_key(key(':'));
    assert_eq!(s.screen, Screen::CommandPalette);
    assert_eq!(action, Action::None);
}

#[test]
fn command_palette_shows_provider_model_combos() {
    let mut s = providers_state_with_model();
    s.screen = Screen::CommandPalette;
    s.input_mode = InputMode::CommandPalette { query: String::new() };
    
    // The palette should have entries for each provider/model combo
    // This test verifies the data structure exists
    // Actual rendering tested in UI tests
    assert!(!s.providers.is_empty());
}

#[test]
fn command_palette_fuzzy_filters_on_typing() {
    let mut s = providers_state_with_model();
    s.screen = Screen::CommandPalette;
    s.input_mode = InputMode::CommandPalette { query: String::new() };
    
    // Type "nim" - should filter to show only NIM-related entries
    for c in "nim".chars() {
        let _ = s.handle_key(key(c));
    }
    
    // Query should be updated
    if let InputMode::CommandPalette { query } = &s.input_mode {
        assert_eq!(query, "nim");
    } else {
        panic!("Expected CommandPalette input mode");
    }
}

#[test]
fn command_palette_enter_selects_provider_model() {
    let mut s = providers_state_with_model();
    s.screen = Screen::CommandPalette;
    s.input_mode = InputMode::CommandPalette { query: "nim".to_string() };
    // Palette should have filtered entries, first one selected
    
    // Press Enter to select → dispara fetch de modelos do provider
    let action = s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(action, Action::ListModels(_)));
}

#[test]
fn command_palette_colon_command_export() {
    let mut s = providers_state_with_model();
    s.screen = Screen::CommandPalette;
    s.input_mode = InputMode::CommandPalette { query: ":export".to_string() };
    
    let action = s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    
    // Should produce ExportProvidersTemplate action
    assert!(matches!(action, Action::ExportProvidersTemplate(_)));
}

#[test]
fn command_palette_escape_closes() {
    let mut s = providers_state_with_model();
    s.screen = Screen::CommandPalette;
    s.input_mode = InputMode::CommandPalette { query: "test".to_string() };
    
    let action = s.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    
    assert_eq!(s.screen, Screen::Chat);
    assert_eq!(s.input_mode, InputMode::Chat);
    assert_eq!(action, Action::None);
}

#[test]
fn command_palette_backspace_edits_query() {
    let mut s = providers_state_with_model();
    s.screen = Screen::CommandPalette;
    s.input_mode = InputMode::CommandPalette { query: "test".to_string() };
    
    let _ = s.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    
    if let InputMode::CommandPalette { query } = &s.input_mode {
        assert_eq!(query, "tes");
    } else {
        panic!("Expected CommandPalette input mode");
    }
}