//! Red tests: tela Grafo, roteamento e ações por nó (spec/06).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use harnizator_core::agents::{AgentStatus, NodeView};
use harnizator_core::{AgentId, TokenUsage};
use harnizator_tui::graph::GraphLayout;
use harnizator_tui::state::{Action, AppState, InputMode, Screen};
use harnizator_tui::ui;
use harnizator_tui::ui_msg::UiMsg;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn node(
    id: &str,
    parent: Option<&str>,
    label: &str,
    status: AgentStatus,
    depth: usize,
) -> NodeView {
    NodeView {
        id: AgentId::new(id),
        parent: parent.map(AgentId::new),
        label: label.into(),
        status,
        depth,
        usage: TokenUsage::default(),
    }
}

fn graph_state(nodes: Vec<NodeView>) -> AppState {
    let mut s = AppState::new("m", "only-flagged");
    s.apply_ui_msg(UiMsg::GraphSync(nodes));
    s.screen = Screen::Graph;
    s
}

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn sample_tree() -> Vec<NodeView> {
    vec![
        node("root", None, "root", AgentStatus::Running, 0),
        node("a", Some("root"), "research", AgentStatus::Running, 1),
        node("b", Some("root"), "code", AgentStatus::Queued, 1),
        node("c", Some("a"), "fetch", AgentStatus::Done, 2),
        node("d", Some("a"), "sum", AgentStatus::Interrupted, 2),
    ]
}

// ---------- reducer de navegação/ações ----------

#[test]
fn graph_sync_populates_nodes_and_selects_first() {
    let s = graph_state(sample_tree());
    assert_eq!(s.graph_nodes.len(), 5);
    assert_eq!(s.graph_selected, 0);
}

#[test]
fn arrows_move_selection_cyclically() {
    let mut s = graph_state(sample_tree());
    let _ = s.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(s.graph_selected, 1);
    let _ = s.handle_key(key('l'));
    assert_eq!(s.graph_selected, 2);
    for _ in 0..3 {
        let _ = s.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    }
    assert_eq!(s.graph_selected, 0, "wrap em volta do primeiro");
}

#[test]
fn x_interrupts_selected_agent() {
    let mut s = graph_state(sample_tree());
    let _ = s.handle_key(key('l')); // seleciona "research" (a)
    let action = s.handle_key(key('x'));
    assert_eq!(action, Action::Interrupt(AgentId::new("a")));
}

#[test]
fn m_enters_message_input_mode_and_enter_sends_to_agent() {
    let mut s = graph_state(sample_tree());
    let _ = s.handle_key(key('l')); // "a"
    let _ = s.handle_key(key('m'));
    assert_eq!(s.screen, Screen::Chat);
    assert_eq!(s.input_mode, InputMode::Message(AgentId::new("a")));
    for c in "go".chars() {
        let _ = s.handle_key(key(c));
    }
    let action = s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(action, Action::SendToAgent(AgentId::new("a"), "go".into()));
    assert_eq!(s.input_mode, InputMode::Chat);
}

#[test]
fn i_enters_context_injection_mode() {
    let mut s = graph_state(sample_tree());
    let _ = s.handle_key(key('l'));
    let _ = s.handle_key(key('i'));
    assert_eq!(s.input_mode, InputMode::Inject(AgentId::new("a")));
    let _ = s.handle_key(key('x'));
    let action = s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(action, Action::InjectContext(AgentId::new("a"), "x".into()));
}

#[test]
fn ctrl_1_and_ctrl_2_switch_screens() {
    let mut s = graph_state(sample_tree());
    let _ = s.handle_key(ctrl('1'));
    assert_eq!(s.screen, Screen::Chat);
    let _ = s.handle_key(ctrl('2'));
    assert_eq!(s.screen, Screen::Graph);
}

#[test]
fn enter_on_graph_returns_to_chat() {
    let mut s = graph_state(sample_tree());
    let action = s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(action, Action::None);
    assert_eq!(s.screen, Screen::Chat);
}

#[test]
fn esc_in_input_mode_reverts_to_chat_mode_without_sending() {
    let mut s = graph_state(sample_tree());
    let _ = s.handle_key(key('l'));
    let _ = s.handle_key(key('i'));
    let action = s.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(action, Action::None);
    assert_eq!(s.input_mode, InputMode::Chat);
}

// ---------- layout do grafo ----------

#[test]
fn graph_layout_layers_by_depth() {
    let layout = GraphLayout::compute(&sample_tree(), 120, 40);
    let root = layout.position(&AgentId::new("root")).unwrap();
    let a = layout.position(&AgentId::new("a")).unwrap();
    let c = layout.position(&AgentId::new("c")).unwrap();
    assert!(root.y < a.y, "root acima dos filhos");
    assert!(a.y < c.y, "pais acima dos filhos");
    let d = layout.position(&AgentId::new("d")).unwrap();
    assert_eq!(a.y, d.y - 4, "mesma profundidade, mesma linha");
}

// ---------- snapshots ----------

fn render(s: &AppState, w: u16, h: u16) -> String {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| ui::draw(f, s)).unwrap();
    format!("{}", term.backend())
}

#[test]
fn graph_screen_single_node() {
    let s = graph_state(vec![node("root", None, "root", AgentStatus::Running, 0)]);
    insta::assert_snapshot!(render(&s, 80, 24));
}

#[test]
fn graph_screen_five_nodes() {
    let s = graph_state(sample_tree());
    insta::assert_snapshot!(render(&s, 80, 24));
}

#[test]
fn graph_screen_many_nodes_with_pan() {
    let nodes: Vec<NodeView> = std::iter::once(node("root", None, "root", AgentStatus::Running, 0))
        .chain((1..=12).map(|i| {
            node(
                &format!("agent-{i}"),
                Some("root"),
                &format!("worker-{i}"),
                if i % 2 == 0 {
                    AgentStatus::Running
                } else {
                    AgentStatus::Queued
                },
                1,
            )
        }))
        .collect();
    let mut s = graph_state(nodes);
    s.graph_offset = (0, 5); // pan vertical
    insta::assert_snapshot!(render(&s, 80, 24));
}

#[test]
fn inject_mode_shows_target_in_input_title() {
    let mut s = graph_state(sample_tree());
    let _ = s.handle_key(key('l'));
    let _ = s.handle_key(key('i'));
    insta::assert_snapshot!(render(&s, 80, 24));
}
