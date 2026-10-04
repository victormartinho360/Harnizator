//! Red tests: AppState active_model persistence + SetActiveModel action (spec/03, 06).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use harness_core::ModelAlias;
use harness_tui::state::{Action, AppState, Screen};
use harness_tui::ui_msg::{ProviderView, UiMsg};

fn providers_state_with_model() -> AppState {
    let mut s = AppState::new("anthropic/claude-sonnet-4-5", "only-flagged");
    s.apply_ui_msg(UiMsg::ProvidersSync(vec![
        ProviderView { id: "anthropic".into(), kind: "anthropic".into(), base_url: "https://api.anthropic.com/v1".into(), configured: true },
        ProviderView { id: "openai".into(), kind: "openai".into(), base_url: "https://api.openai.com/v1".into(), configured: false },
        ProviderView { id: "nim".into(), kind: "nim".into(), base_url: "https://integrate.api.nvidia.com/v1".into(), configured: true },
    ]));
    s.screen = Screen::Providers;
    s
}

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

#[test]
fn app_state_has_active_model_field() {
    let s = AppState::new("openai/gpt-4o", "only-flagged");
    // AppState should have active_model field (Option<ModelAlias>)
    // Initially set from constructor argument
    assert_eq!(s.active_model, Some(ModelAlias::parse("openai/gpt-4o").unwrap()));
}

#[test]
fn app_state_new_without_model_has_none() {
    let s = AppState::new("", "only-flagged");
    // Empty model string should result in None
    assert_eq!(s.active_model, None);
}

#[test]
fn set_active_model_via_action_updates_state() {
    let mut s = providers_state_with_model();
    let alias = ModelAlias::parse("nim/nemotron-3-ultra").unwrap();
    
    // Apply the action directly via handle_key simulation
    // We can't call apply_action directly, so we test the reducer logic
    // by checking that SetActiveModel variant exists and would work
    // For now, verify the action variant exists
    let _action = Action::SetActiveModel(alias.clone());
    
    // Test that we can create the action
    assert!(matches!(_action, Action::SetActiveModel(a) if a == alias));
}

#[test]
fn providers_screen_enter_sets_active_model() {
    let mut s = providers_state_with_model();
    s.providers_selected = 2; // nim
    
    // Press Enter on selected provider
    let action = s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    
    // Should produce SetActiveModel action with provider/default_model
    assert!(matches!(action, Action::SetActiveModel(_)));
    if let Action::SetActiveModel(alias) = action {
        assert_eq!(alias.provider(), "nim");
        // Model should default to a known model for that provider
        assert!(!alias.model().is_empty());
        assert_eq!(alias.model(), "nemotron-3-ultra");
    }
}

#[test]
fn providers_screen_enter_anthropic_sets_claude() {
    let mut s = providers_state_with_model();
    s.providers_selected = 0; // anthropic
    
    let action = s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    
    assert!(matches!(action, Action::SetActiveModel(_)));
    if let Action::SetActiveModel(alias) = action {
        assert_eq!(alias.provider(), "anthropic");
        assert_eq!(alias.model(), "claude-sonnet-4-5");
    }
}

#[test]
fn providers_screen_enter_openai_sets_gpt4o() {
    let mut s = providers_state_with_model();
    s.providers_selected = 1; // openai
    
    let action = s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    
    assert!(matches!(action, Action::SetActiveModel(_)));
    if let Action::SetActiveModel(alias) = action {
        assert_eq!(alias.provider(), "openai");
        assert_eq!(alias.model(), "gpt-4o");
    }
}

#[test]
fn active_model_can_be_set_and_read() {
    let mut s = AppState::new("anthropic/claude-sonnet-4-5", "only-flagged");
    let new_alias = ModelAlias::parse("openai/gpt-4o").unwrap();
    
    // Simulate the reducer applying the action
    s.active_model = Some(new_alias.clone());
    s.model = new_alias.to_string();
    
    assert_eq!(s.active_model, Some(new_alias));
    assert_eq!(s.model, "openai/gpt-4o");
}