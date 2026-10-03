//! Desenho das telas (puro de render: `AppState` → Frame).

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::markdown::render_markdown;
use crate::state::{AppState, DisplayRole};

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
    draw_chat(f, chunks[1], state);
    draw_input(f, chunks[2], state);
    if let Some(pending) = &state.awaiting_approval {
        draw_approval_modal(f, area, pending);
    }
}

fn draw_status_bar(f: &mut Frame, area: Rect, state: &AppState) {
    let left = format!(
        " harness-rs │ {} │ {} │ ↑{} ↓{}",
        state.model, state.sandbox, state.usage.input, state.usage.output
    );
    let right = if state.generating {
        "generating… Esc cancela "
    } else {
        "Enter envia · ? help · Ctrl+C sai "
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
    let title = if state.generating {
        "input (gerando…)"
    } else {
        "input"
    };
    let border_style = if state.generating {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::Cyan)
    };
    let input = Paragraph::new(state.input.clone()).block(
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
