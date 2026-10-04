//! Red tests: command palette lista modelos reais via provider.models() (spec/06).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use harnizator_core::ModelAlias;
use harnizator_tui::state::{Action, AppState, InputMode, Screen};
use harnizator_tui::ui_msg::{ProviderView, UiMsg};

fn providers_state() -> AppState {
    let mut s = AppState::new("anthropic/claude-sonnet-4-5", "only-flagged");
    s.apply_ui_msg(UiMsg::ProvidersSync(vec![
        ProviderView { id: "anthropic".into(), kind: "anthropic".into(), base_url: "https://api.anthropic.com/v1".into(), configured: true },
        ProviderView { id: "nim".into(), kind: "nim".into(), base_url: "https://integrate.api.nvidia.com/v1".into(), configured: true },
    ]));
    s.screen = Screen::CommandPalette;
    s.input_mode = InputMode::CommandPalette { query: String::new() };
    s
}

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

fn enter() -> KeyEvent {
    KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
}

#[test]
fn enter_on_provider_requests_models() {
    let mut s = providers_state();
    let action = s.handle_key(enter());
    // selecionar provider no palette dispara fetch de modelos, não SetActiveModel
    assert_eq!(action, Action::ListModels("anthropic".into()));
}

#[test]
fn models_sync_populates_model_picker() {
    let mut s = providers_state();
    let _ = s.handle_key(enter()); // ListModels
    s.apply_ui_msg(UiMsg::ModelsSync {
        provider: "anthropic".into(),
        result: Ok(vec![
            "claude-sonnet-4-5".into(),
            "claude-opus-4-1".into(),
        ]),
    });
    assert_eq!(s.palette_provider.as_deref(), Some("anthropic"));
    assert_eq!(s.palette_models.len(), 2);
    assert_eq!(s.palette_models[0], "claude-sonnet-4-5");
}

#[test]
fn enter_on_model_sets_active_model() {
    let mut s = providers_state();
    let _ = s.handle_key(enter()); // ListModels("anthropic")
    s.apply_ui_msg(UiMsg::ModelsSync {
        provider: "anthropic".into(),
        result: Ok(vec!["claude-opus-4-1".into(), "claude-sonnet-4-5".into()]),
    });
    // navega para o segundo modelo e seleciona
    let _ = s.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    let action = s.handle_key(enter());
    let alias = ModelAlias::parse("anthropic/claude-sonnet-4-5").unwrap();
    assert_eq!(action, Action::SetActiveModel(alias.clone()));
    assert_eq!(s.active_model, Some(alias));
    assert_eq!(s.screen, Screen::Chat);
}

#[test]
fn explicit_alias_with_slash_selects_directly() {
    let mut s = providers_state();
    for c in "nim/nvidia/llama-3.1-nemotron-70b-instruct".chars() {
        let _ = s.handle_key(key(c));
    }
    let action = s.handle_key(enter());
    let alias = ModelAlias::parse("nim/nvidia/llama-3.1-nemotron-70b-instruct").unwrap();
    assert_eq!(action, Action::SetActiveModel(alias));
}

#[test]
fn models_error_shows_status() {
    let mut s = providers_state();
    let _ = s.handle_key(enter());
    s.apply_ui_msg(UiMsg::ModelsSync {
        provider: "nim".into(),
        result: Err("401 unauthorized".into()),
    });
    assert!(s.providers_status.contains("nim"));
    assert!(s.providers_status.contains("401"));
}

#[test]
fn esc_from_models_goes_back_to_providers_list() {
    let mut s = providers_state();
    let _ = s.handle_key(enter());
    s.apply_ui_msg(UiMsg::ModelsSync {
        provider: "anthropic".into(),
        result: Ok(vec!["m".into()]),
    });
    let _ = s.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(s.palette_models.is_empty());
    assert_eq!(s.screen, Screen::CommandPalette); // volta à lista, não fecha
}

#[test]
fn palette_scrolls_to_keep_selection_visible() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    let mut s = providers_state();
    let _ = s.handle_key(enter());
    let models: Vec<String> = (0..50).map(|i| format!("model-{i:02}")).collect();
    s.apply_ui_msg(UiMsg::ModelsSync {
        provider: "anthropic".into(),
        result: Ok(models),
    });
    // desce além da área visível (terminal 80x12 → poucas linhas de lista)
    for _ in 0..40 {
        let _ = s.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    }
    assert_eq!(s.palette_selected, 40);
    let mut term = Terminal::new(TestBackend::new(80, 12)).unwrap();
    term.draw(|f| harnizator_tui::ui::draw(f, &s)).unwrap();
    let screen = format!("{}", term.backend());
    assert!(screen.contains("model-40"), "seleção deve estar visível após scroll:\n{screen}");

    // e sobe de volta: primeiro visível de novo
    for _ in 0..40 {
        let _ = s.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    }
    assert_eq!(s.palette_selected, 0);
    let mut term = Terminal::new(TestBackend::new(80, 12)).unwrap();
    term.draw(|f| harnizator_tui::ui::draw(f, &s)).unwrap();
    let screen = format!("{}", term.backend());
    assert!(screen.contains("model-00"));
}

#[test]
fn palette_renders_models_after_provider_selected() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    let mut s = providers_state();
    // Enter no primeiro provider → dispara fetch
    let action = s.handle_key(enter());
    assert!(matches!(action, Action::ListModels(_)));
    // resposta chega
    s.apply_ui_msg(UiMsg::ModelsSync {
        provider: "anthropic".into(),
        result: Ok(vec!["claude-a".into(), "claude-b".into()]),
    });
    let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    term.draw(|f| harnizator_tui::ui::draw(f, &s)).unwrap();
    let screen = format!("{}", term.backend());
    assert!(screen.contains("claude-a"), "modelo deve aparecer:\n{screen}");
    assert!(screen.contains("claude-b"), "modelo deve aparecer:\n{screen}");
}

#[test]
fn palette_renders_providers_on_open() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    let s = providers_state();
    let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    term.draw(|f| harnizator_tui::ui::draw(f, &s)).unwrap();
    let screen = format!("{}", term.backend());
    assert!(screen.contains("anthropic"), "provider deve aparecer:\n{screen}");
    assert!(screen.contains("nim"), "provider deve aparecer:\n{screen}");
}
