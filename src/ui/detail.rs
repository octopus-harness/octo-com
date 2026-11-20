use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, Pane, ViewMode};
use crate::format::{format_bytes, word_and_line_count};
use crate::models::parse_numbered_step_prefix;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focused_pane == Pane::Detail;
    let border_color = if is_focused {
        Color::Magenta
    } else {
        Color::DarkGray
    };

    let title_style = if is_focused {
        Style::default()
            .fg(Color::LightMagenta)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };

    let block_title = format!(" [2] Command Detail [{}] ", app.view_mode.label());

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(block_title, title_style));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let doc = match app.current_command() {
        Some(d) => d,
        None => {
            let empty = Paragraph::new(Line::from(Span::styled(
                "No command selected",
                Style::default().fg(Color::DarkGray),
            )));
            f.render_widget(empty, inner);
            return;
        }
    };

    // Split inner area into: Header Card (3 lines), Divider (1 line), Body (Min(0))
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    let (words, total_lines) = word_and_line_count(&doc.raw_content);
    let steps = doc.step_count();
    let header_badge = if doc.has_valid_header {
        Span::styled(" ✓ 3-line Header ", Style::default().fg(Color::Green))
    } else {
        Span::styled(
            " ⚠ Missing 3-line Header ('e' to fix) ",
            Style::default().fg(Color::Yellow),
        )
    };

    let mut meta_lines = Vec::new();
    meta_lines.push(Line::from(vec![
        Span::styled(
            format!("$${}", doc.name),
            Style::default()
                .fg(Color::LightMagenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" ({})", doc.filename),
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw(" · "),
        header_badge,
        Span::styled(
            format!(
                "· {} steps · {}w / {}L · {} · {}",
                steps,
                words,
                total_lines,
                format_bytes(doc.size_bytes),
                doc.updated_at
            ),
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    let args_display = doc.display_args().unwrap_or("-");
    meta_lines.push(Line::from(vec![
        Span::styled("Usage: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            doc.display_usage(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("   Args: ", Style::default().fg(Color::DarkGray)),
        Span::styled(args_display, Style::default().fg(Color::Cyan)),
    ]));

    meta_lines.push(Line::from(vec![
        Span::styled("Description: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            doc.display_description(),
            Style::default().fg(Color::LightYellow),
        ),
    ]));

    f.render_widget(
        Paragraph::new(meta_lines).wrap(Wrap { trim: false }),
        chunks[0],
    );

    let divider = Paragraph::new(Line::from(Span::styled(
        "─".repeat(chunks[1].width as usize),
        Style::default().fg(Color::DarkGray),
    )));
    f.render_widget(divider, chunks[1]);

    let rendered_lines = match app.view_mode {
        ViewMode::Preview => render_markdown_preview(&doc.body, chunks[2].width as usize),
        ViewMode::Raw => render_markdown_raw(&doc.raw_content),
    };

    let scroll_y = app.detail_scroll as u16;
    let body_paragraph = Paragraph::new(rendered_lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll_y, 0));
    f.render_widget(body_paragraph, chunks[2]);
}

fn render_markdown_raw(text: &str) -> Vec<Line<'_>> {
    if text.trim().is_empty() {
        return vec![Line::from(Span::styled(
            "(empty command file — press 'e' to add header, 'a' to append a step, or 'E' to open in $EDITOR)",
            Style::default().fg(Color::DarkGray),
        ))];
    }

    text.lines()
        .enumerate()
        .map(|(idx, raw_line)| {
            let line_style = if idx == 0 && raw_line.starts_with("Usage:")
                || idx == 1 && raw_line.starts_with("Args:")
                || idx == 2 && raw_line.starts_with("Description:")
            {
                Style::default().fg(Color::LightMagenta)
            } else {
                Style::default().fg(Color::White)
            };
            Line::from(vec![
                Span::styled(
                    format!("{:>4} │ ", idx + 1),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(raw_line, line_style),
            ])
        })
        .collect()
}

fn render_markdown_preview(text: &str, width: usize) -> Vec<Line<'_>> {
    if text.trim().is_empty() {
        return vec![Line::from(Span::styled(
            "(no execution instructions yet — press 'a' to append a numbered step or 'E' to edit in $EDITOR)",
            Style::default().fg(Color::DarkGray),
        ))];
    }

    let mut out = Vec::new();
    let mut in_code_block = false;

    for raw_line in text.lines() {
        let trimmed = raw_line.trim_start();

        // Fenced code blocks
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code_block = !in_code_block;
            out.push(Line::from(Span::styled(
                raw_line,
                Style::default().fg(Color::DarkGray),
            )));
            continue;
        }

        if in_code_block {
            out.push(Line::from(Span::styled(
                format!("  {}", raw_line),
                Style::default().fg(Color::Green),
            )));
            continue;
        }

        // Horizontal rules
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            out.push(Line::from(Span::styled(
                "─".repeat(width.saturating_sub(2).max(10)),
                Style::default().fg(Color::DarkGray),
            )));
            continue;
        }

        // Tool use permission banner common in Octopus commands
        if trimmed.starts_with("Tool use is allowed") {
            out.push(Line::from(vec![
                Span::styled(
                    "⚡ ",
                    Style::default()
                        .fg(Color::LightMagenta)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    trimmed,
                    Style::default()
                        .fg(Color::LightMagenta)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]));
            continue;
        }

        // Numbered runbook steps (1. Step one...)
        if let Some((num, rest)) = parse_numbered_step_prefix(trimmed) {
            let indent = &raw_line[..(raw_line.len() - trimmed.len())];
            let mut spans = vec![
                Span::raw(indent),
                Span::styled(
                    format!("{}. ", num),
                    Style::default()
                        .fg(Color::LightMagenta)
                        .add_modifier(Modifier::BOLD),
                ),
            ];
            spans.extend(style_inline_code(rest, Style::default().fg(Color::White)));
            out.push(Line::from(spans));
            continue;
        }

        // Headings
        if trimmed.starts_with("# ") {
            out.push(Line::from(Span::styled(
                raw_line,
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            )));
        } else if trimmed.starts_with("## ") {
            out.push(Line::from(Span::styled(
                raw_line,
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )));
        } else if trimmed.starts_with("### ") || trimmed.starts_with("#### ") {
            out.push(Line::from(Span::styled(
                raw_line,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
        } else if trimmed.starts_with('>') {
            out.push(Line::from(Span::styled(
                raw_line,
                Style::default()
                    .fg(Color::LightBlue)
                    .add_modifier(Modifier::ITALIC),
            )));
        } else if trimmed.starts_with('|') && trimmed.ends_with('|') {
            out.push(Line::from(Span::styled(
                raw_line,
                Style::default().fg(Color::LightCyan),
            )));
        } else if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            let indent = &raw_line[..(raw_line.len() - trimmed.len())];
            let mut spans = vec![
                Span::raw(indent),
                Span::styled("• ", Style::default().fg(Color::Magenta)),
            ];
            spans.extend(style_inline_code(rest, Style::default().fg(Color::White)));
            out.push(Line::from(spans));
        } else {
            out.push(Line::from(style_inline_code(
                raw_line,
                Style::default().fg(Color::White),
            )));
        }
    }

    out
}

fn style_inline_code(line: &str, default_style: Style) -> Vec<Span<'_>> {
    let mut spans = Vec::new();
    let mut in_backtick = false;
    let mut start = 0;

    for (idx, ch) in line.char_indices() {
        if ch == '`' {
            let segment = &line[start..idx];
            if !segment.is_empty() {
                if in_backtick {
                    spans.push(Span::styled(
                        segment,
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else {
                    spans.push(Span::styled(segment, default_style));
                }
            }
            in_backtick = !in_backtick;
            start = idx + 1;
        }
    }

    if start < line.len() {
        spans.push(Span::styled(&line[start..], default_style));
    }

    if spans.is_empty() {
        spans.push(Span::raw(""));
    }

    spans
}
