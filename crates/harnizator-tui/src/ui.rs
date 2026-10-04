//! Desenho das telas (puro de render: `AppState` → Frame).

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use harnizator_core::agents::AgentStatus;

use crate::graph::{GraphLayout, NODE_H, NODE_W};
use crate::markdown::render_markdown;
use crate::state::{AppState, DisplayRole, InputMode, Screen};

/// Desenha o frame inteiro.
pub fn draw(f: &mut Frame, state: &AppState) {
    let area = f.area();
    if area.width < 4 || area.height < 3 {
        return; // tela degenerada: nada a desenhar
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(area);

    draw_status_bar(f, chunks[0], state);
    match state.screen {
        Screen::Chat | Screen::Help => {
            draw_chat(f, chunks[1], state);
            draw_input(f, chunks[2], state);
        }
        Screen::Graph => {
            draw_graph(f, chunks[1], state);
            draw_input(f, chunks[2], state);
        }
        Screen::Sessions => draw_sessions(f, chunks[1], state),
        Screen::Providers => {
            draw_providers(f, chunks[1], state);
            draw_input(f, chunks[2], state);
        }
        Screen::CommandPalette => {
            draw_command_palette(f, chunks[1], state);
            draw_input(f, chunks[2], state);
        }
    }
    if state.screen == Screen::Help {
        draw_help(f, area);
    }
    if let Some(pending) = &state.awaiting_approval {
        draw_approval_modal(f, area, pending);
    }
}

fn draw_graph(f: &mut Frame, area: Rect, state: &AppState) {
    use ratatui::text::Line;
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" agentes (setas/hjkl navega · Enter chat · m msg · i injeta · x interrompe) ");
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let layout = GraphLayout::compute(&state.graph_nodes, inner.width, inner.height);
    let mut buf: Vec<Vec<(char, Style)>> =
        vec![vec![(' ', Style::default()); inner.width as usize]; inner.height as usize];

    // arestas primeiro (ficam atrás dos nós)
    for n in &state.graph_nodes {
        let (Some(parent_id), Some(child)) = (n.parent.as_ref(), layout.position(&n.id)) else {
            continue;
        };
        let Some(pp) = layout.position(parent_id) else {
            continue;
        };
        let cx = pp.x + NODE_W / 2;
        for y in pp.y + NODE_H..child.y {
            plot(
                &mut buf,
                inner,
                (cx, y),
                '│',
                Style::default().fg(Color::DarkGray),
                state,
            );
        }
    }

    // nós
    for (i, n) in state.graph_nodes.iter().enumerate() {
        let Some(pos) = layout.position(&n.id) else {
            continue;
        };
        let (dot, style) = match n.status {
            AgentStatus::Running => ("●", Style::default().fg(Color::Green)),
            AgentStatus::Queued => ("○", Style::default().fg(Color::DarkGray)),
            AgentStatus::Idle => ("◆", Style::default().fg(Color::Cyan)),
            AgentStatus::Done => ("✓", Style::default().fg(Color::Blue)),
            AgentStatus::Failed => ("✗", Style::default().fg(Color::Red)),
            AgentStatus::Interrupted => ("■", Style::default().fg(Color::Yellow)),
        };
        let style = if i == state.graph_selected {
            style.add_modifier(Modifier::REVERSED)
        } else {
            style
        };
        let label: String = n.label.chars().take(NODE_W as usize - 4).collect();
        let text = format!("{} {}", dot, label);
        for (dx, ch) in text.chars().enumerate() {
            plot(
                &mut buf,
                inner,
                (pos.x + 1 + dx as u16, pos.y + 1),
                ch,
                style,
                state,
            );
        }
        plot(&mut buf, inner, (pos.x, pos.y), '┌', style, state);
        plot(
            &mut buf,
            inner,
            (pos.x + NODE_W - 1, pos.y),
            '┐',
            style,
            state,
        );
        plot(
            &mut buf,
            inner,
            (pos.x, pos.y + NODE_H - 1),
            '└',
            style,
            state,
        );
        plot(
            &mut buf,
            inner,
            (pos.x + NODE_W - 1, pos.y + NODE_H - 1),
            '┘',
            style,
            state,
        );
        for dx in 1..NODE_W - 1 {
            plot(&mut buf, inner, (pos.x + dx, pos.y), '─', style, state);
            plot(
                &mut buf,
                inner,
                (pos.x + dx, pos.y + NODE_H - 1),
                '─',
                style,
                state,
            );
        }
        plot(&mut buf, inner, (pos.x, pos.y + 1), '│', style, state);
        plot(
            &mut buf,
            inner,
            (pos.x + NODE_W - 1, pos.y + 1),
            '│',
            style,
            state,
        );
    }

    let lines: Vec<Line> = buf
        .into_iter()
        .map(|row| {
            Line::from(
                row.into_iter()
                    .map(|(ch, st)| Span::styled(ch.to_string(), st))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

fn plot(
    buf: &mut [Vec<(char, Style)>],
    area: Rect,
    (x, y): (u16, u16),
    ch: char,
    style: Style,
    state: &AppState,
) {
    let (px, py) = state.graph_offset;
    let (x, y) = (x as i32 - px as i32, y as i32 - py as i32);
    if x < 0 || y < 0 || x >= area.width as i32 || y >= area.height as i32 {
        return;
    }
    buf[y as usize][x as usize] = (ch, style);
}

fn draw_sessions(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" sessões (Enter resume · Esc volta) ");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let lines: Vec<Line> = state
        .sessions
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let style = if i == state.sessions_selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            Line::from(Span::styled(
                format!(
                    " {} │ {} │ {} · {}in/{}out",
                    m.id, m.title, m.model, m.usage.input, m.usage.output
                ),
                style,
            ))
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_providers(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default().borders(Borders::ALL).title(
        " providers (j/↑↓ navega · k chave · a adicionar · t testar · d remover · Esc volta) ",
    );
    let inner = block.inner(area);
    f.render_widget(block, area);
    let mut lines: Vec<Line> = state
        .providers
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let style = if i == state.providers_selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            let mark = if p.configured { "●" } else { "○" };
            Line::from(Span::styled(
                format!(" {} {} │ {} │ {}", mark, p.id, p.kind, p.base_url),
                style,
            ))
        })
        .collect();
    if !state.providers_status.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            state.providers_status.clone(),
            Style::default().fg(Color::Yellow),
        )));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_help(f: &mut Frame, area: Rect) {
    let rect = Rect::new(
        area.x + area.width / 6,
        area.y + area.height / 6,
        area.width * 2 / 3,
        area.height * 2 / 3,
    );
    f.render_widget(Clear, rect);
    let text = vec![
        Line::from(
            "Tab/Shift+Tab troca de tela · F1 chat · F2 grafo · F3 ajuda · F4 sessões · F5 providers",
        ),
        Line::from("Enter envia · Esc cancela geração (ou sai do modo msg/i)"),
        Line::from("grafo: setas/hjkl navegam · m mensagem · i injeção · x interrompe"),
        Line::from("aprovação: y aprova · a aprova+allowlist · n nega"),
    ];
    f.render_widget(
        Paragraph::new(text).block(Block::default().borders(Borders::ALL).title(" ajuda ")),
        rect,
    );
}

fn draw_status_bar(f: &mut Frame, area: Rect, state: &AppState) {
    let left = format!(
        " harnizator │ {} │ {} │ ↑{} ↓{}",
        state.model, state.sandbox, state.usage.input, state.usage.output
    );
    let right = if state.generating {
        "gerando… Esc cancela "
    } else {
        "Enter envia · Tab telas · F3 ajuda "
    };
    let width = area.width as usize;
    let pad = width
        .saturating_sub(left.chars().count() + right.chars().count())
        .max(1);
    let line = Line::from(vec![
        Span::styled(left, Style::default().fg(Color::Cyan)),
        Span::raw(" ".repeat(pad)),
        Span::styled(right, Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(
        Paragraph::new(line).style(Style::default().bg(Color::Black)),
        area,
    );
}

fn draw_chat(f: &mut Frame, area: Rect, state: &AppState) {
    let mut lines: Vec<Line<'static>> = Vec::new();
    for msg in &state.messages {
        let (label, base) = match msg.role {
            DisplayRole::User => ("you", Style::default().fg(Color::Green)),
            DisplayRole::Assistant => ("assistant", Style::default().fg(Color::White)),
            DisplayRole::Tool => ("tool", Style::default().fg(Color::Yellow)),
            DisplayRole::System => ("sys", Style::default().fg(Color::DarkGray)),
        };
        lines.push(Line::from(Span::styled(
            format!("[{label}]"),
            base.add_modifier(Modifier::BOLD),
        )));
        let mut rendered = render_markdown(&msg.content);
        if msg.streaming {
            rendered.lines.push(Line::from(Span::styled(
                "▌",
                Style::default().fg(Color::Cyan),
            )));
        }
        lines.extend(rendered.lines);
    }

    let total = lines.len();
    let view = area.height as usize;
    let offset = if state.follow {
        total.saturating_sub(view)
    } else {
        let max = total.saturating_sub(view);
        // scroll conta "linhas acima do fundo"
        max.saturating_sub(state.scroll as usize)
    } as u16;

    let chat = Paragraph::new(lines)
        .block(Block::default().borders(Borders::NONE))
        .wrap(Wrap { trim: false })
        .scroll((offset, 0));
    f.render_widget(chat, area);
}

fn draw_input(f: &mut Frame, area: Rect, state: &AppState) {
    let title = match &state.input_mode {
        InputMode::Chat if state.generating => "input (gerando…)".to_string(),
        InputMode::Chat => "input".to_string(),
        InputMode::Message(id) => format!("mensagem → {id}"),
        InputMode::Inject(id) => format!("inject contexto → {id}"),
        InputMode::SetKey(id) => format!("api key → {id}"),
        InputMode::AddProviderName => "novo provider: nome".to_string(),
        InputMode::AddProviderKind { .. } => {
            "novo provider: kind (enter = openai-compatible)".to_string()
        }
        InputMode::AddProviderBaseUrl { .. } => "novo provider: base_url".to_string(),
        InputMode::CommandPalette { .. } => "command palette (fuzzy search + :commands)".to_string(),
    };
    let border_style = if state.generating {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::Cyan)
    };
    let display = if matches!(state.input_mode, InputMode::SetKey(_)) {
        "•".repeat(state.input.chars().count())
    } else {
        state.input.clone()
    };
    let input = Paragraph::new(display).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style),
    );
    f.render_widget(input, area);
}

fn draw_approval_modal(f: &mut Frame, area: Rect, pending: &crate::state::PendingApproval) {
    let w = (area.width * 2 / 3).clamp(20, area.width);
    let h = 9u16.min(area.height);
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let rect = Rect::new(x, y, w, h);

    f.render_widget(Clear, rect);
    let args = serde_json::to_string_pretty(&pending.call.args).unwrap_or_default();
    let text = vec![
        Line::from(Span::styled(
            format!("tool: {}", pending.call.name),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(args),
        Line::from(""),
        Line::from(Span::styled(
            pending.reason.clone(),
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "[y] aprovar   [a] aprovar+allowlist   [n/Esc] negar",
            Style::default().fg(Color::Cyan),
        )),
    ];
    let modal = Paragraph::new(text).wrap(Wrap { trim: false }).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" aprovação necessária ")
            .border_style(Style::default().fg(Color::Yellow)),
    );
    f.render_widget(modal, rect);
}

/// Desenha a command palette (Ctrl+P / :) com busca fuzzy de providers/modelos.
fn draw_command_palette(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default().borders(Borders::ALL).title(
        " command palette (Ctrl+P / : · type to filter · Enter select · Esc close) ",
    );
    let inner = block.inner(area);
    f.render_widget(block, area);
    
    // Get query from input mode
    let query = if let InputMode::CommandPalette { query } = &state.input_mode {
        query.as_str()
    } else {
        ""
    };
    
    // Filter providers based on query
    let filtered: Vec<_> = state.providers.iter()
        .filter(|p| {
            if query.is_empty() {
                true
            } else {
                let haystack = format!("{} {}", p.id, p.kind).to_lowercase();
                haystack.contains(&query.to_lowercase())
            }
        })
        .collect();
    
    let mut lines: Vec<Line> = Vec::new();
    
    // Show query
    lines.push(Line::from(Span::styled(
        format!("> {}", query),
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));
    // reservadas no topo: prompt (2 linhas) + header de modelos (1) quando aplicável
    let header_h: u16 = if state.palette_provider.is_some() { 3 } else { 2 };
    
    // Estágio de modelos: provider escolhido → lista modelos reais
    if let Some(provider) = &state.palette_provider {
        lines.push(Line::from(Span::styled(
            format!(" modelos de {} (↑↓ navega · Enter seleciona · Esc volta) ", provider),
            Style::default().fg(Color::Yellow),
        )));
        for (i, m) in state.palette_models.iter().enumerate() {
            let style = if i == state.palette_selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(format!("   {}", m), style)));
        }
    } else {
        // Show filtered providers (sem modelo: Enter busca a lista real)
        for (i, p) in filtered.iter().enumerate() {
            let style = if i == state.palette_selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            let mark = if p.configured { "●" } else { "○" };
            lines.push(Line::from(Span::styled(
                format!(" {} {} ({})", mark, p.kind, p.id),
                style,
            )));
        }
    }
    
    // Show colon commands
    if query.starts_with(':') {
        lines.push(Line::from(""));
        let cmd = &query[1..];
        let commands = vec![
            ("export", "exporta template providers.toml (sem chaves)"),
            ("help", "abre tela de ajuda"),
        ];
        for (c, desc) in commands {
            let style = if c.starts_with(cmd) {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(
                format!(" :{} — {}", c, desc),
                style,
            )));
        }
    }
    
    // Show status if any
    if !state.providers_status.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            state.providers_status.clone(),
            Style::default().fg(Color::Yellow),
        )));
    }
    
    // scroll mantém o item selecionado visível na área
    let visible = inner.height.saturating_sub(header_h) as usize;
    let item_pos = header_h as usize + state.palette_selected; // linha do selecionado
    let offset = if item_pos >= visible + header_h as usize {
        (item_pos - visible - header_h as usize + 1) as u16
    } else {
        0
    };
    f.render_widget(Paragraph::new(lines).scroll((offset, 0)), inner);
}
