//! Markdown → `ratatui::text::Text` (spec/05, D12): pulldown-cmark → spans estilizados.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

/// Renderiza markdown comum a um `Text` estático (independe da tela).
pub fn render_markdown(src: &str) -> Text<'static> {
    let mut out = MarkdownRenderer::default();
    out.run(src);
    Text::from(out.lines)
}

#[derive(Default)]
struct MarkdownRenderer {
    lines: Vec<Line<'static>>,
    cur: Vec<Span<'static>>,
    style: Vec<Style>,
    in_code_block: bool,
    list_depth: usize,
    in_blockquote: bool,
}

impl MarkdownRenderer {
    fn style(&self) -> Style {
        self.style.last().copied().unwrap_or_default()
    }

    fn push_span(&mut self, text: String) {
        let style = if self.in_code_block {
            Style::default().bg(Color::DarkGray)
        } else {
            self.style()
        };
        self.cur.push(Span::styled(text, style));
    }

    fn flush_line(&mut self) {
        if self.in_blockquote {
            self.cur
                .insert(0, Span::styled("│ ", Style::default().fg(Color::DarkGray)));
        }
        self.lines.push(Line::from(std::mem::take(&mut self.cur)));
    }

    fn blank_line(&mut self) {
        if self
            .lines
            .last()
            .is_some_and(|l: &Line| !l.spans.is_empty())
        {
            self.lines.push(Line::default());
        }
    }

    fn run(&mut self, src: &str) {
        let parser = Parser::new_ext(src, Options::empty());
        for event in parser {
            match event {
                Event::Start(tag) => match tag {
                    Tag::Heading { level, .. } => {
                        self.blank_line();
                        let mods = Modifier::BOLD
                            | if level == HeadingLevel::H1 {
                                Modifier::UNDERLINED
                            } else {
                                Modifier::empty()
                            };
                        self.style.push(Style::default().add_modifier(mods));
                        let prefix = "#".repeat(level as usize) + " ";
                        self.push_span(prefix);
                    }
                    Tag::Strong => self.style.push(self.style().add_modifier(Modifier::BOLD)),
                    Tag::Emphasis => self.style.push(self.style().add_modifier(Modifier::ITALIC)),
                    Tag::CodeBlock(_) => {
                        self.blank_line();
                        self.in_code_block = true;
                    }
                    Tag::BlockQuote(_) => self.in_blockquote = true,
                    Tag::List(_) => self.list_depth += 1,
                    Tag::Item => {
                        self.flush_line();
                        let indent = "  ".repeat(self.list_depth.saturating_sub(1));
                        self.cur.push(Span::raw(format!("{indent}• ")));
                    }
                    Tag::Link { .. } => self
                        .style
                        .push(self.style().add_modifier(Modifier::UNDERLINED)),
                    Tag::Paragraph => self.blank_line(),
                    _ => {}
                },
                Event::End(tag) => match tag {
                    TagEnd::Heading(_) => {
                        self.style.pop();
                        self.flush_line();
                        self.blank_line();
                    }
                    TagEnd::Strong | TagEnd::Emphasis | TagEnd::Link => {
                        self.style.pop();
                    }
                    TagEnd::CodeBlock => {
                        self.in_code_block = false;
                        self.flush_line();
                        self.blank_line();
                    }
                    TagEnd::BlockQuote(_) => {
                        self.in_blockquote = false;
                        self.flush_line();
                    }
                    TagEnd::List(_) => self.list_depth = self.list_depth.saturating_sub(1),
                    TagEnd::Paragraph => self.flush_line(),
                    _ => {}
                },
                Event::Text(t) => {
                    if self.in_code_block {
                        for line in t.as_ref().lines() {
                            self.push_span(line.to_string());
                            self.flush_line();
                        }
                    } else {
                        self.push_span(t.into_string());
                    }
                }
                Event::Code(c) => {
                    self.cur.push(Span::styled(
                        c.into_string(),
                        self.style().bg(Color::DarkGray),
                    ));
                }
                Event::SoftBreak => {
                    self.push_span(" ".into());
                }
                Event::HardBreak => self.flush_line(),
                Event::Rule => {
                    self.cur.push(Span::styled(
                        "─".repeat(40),
                        Style::default().fg(Color::DarkGray),
                    ));
                    self.flush_line();
                }
                _ => {}
            }
        }
        self.flush_line();
    }
}
