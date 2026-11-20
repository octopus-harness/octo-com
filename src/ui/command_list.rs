use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::{App, Pane};
use crate::format::truncate_width;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focused_pane == Pane::CommandList;
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

    let title = format!(" [1] Commands ({}) ", app.filtered_commands.len());

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(title, title_style));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.filtered_commands.is_empty() {
        let msg = if app.search_query.is_some() {
            "No matching commands (press Esc to clear filter)"
        } else {
            "No commands in directory (press 'n' to create one)"
        };
        let empty = Paragraph::new(Line::from(Span::styled(
            msg,
            Style::default().fg(Color::DarkGray),
        )));
        f.render_widget(empty, inner);
        return;
    }

    let max_w = inner.width as usize;

    let items: Vec<ListItem> = app
        .filtered_commands
        .iter()
        .enumerate()
        .map(|(idx, doc)| {
            let is_selected = idx == app.selected_index;
            let cursor = if is_selected { "▶ " } else { "  " };

            let name_style = if is_selected && is_focused {
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Magenta)
            };

            let mut line1_spans = vec![
                Span::styled(
                    cursor,
                    if is_selected {
                        Style::default()
                            .fg(Color::LightMagenta)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
                Span::styled(format!("$${}", doc.name), name_style),
            ];

            if let Some(args) = doc.display_args() {
                line1_spans.push(Span::raw(" "));
                line1_spans.push(Span::styled(
                    args.to_string(),
                    Style::default().fg(Color::Cyan),
                ));
            }

            if !doc.has_valid_header {
                line1_spans.push(Span::raw(" "));
                line1_spans.push(Span::styled(
                    "[!header]",
                    Style::default().fg(Color::Yellow),
                ));
            } else {
                let steps = doc.step_count();
                if steps > 0 {
                    line1_spans.push(Span::raw(" "));
                    line1_spans.push(Span::styled(
                        format!("[{} steps]", steps),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
            }

            let desc_text = truncate_width(doc.display_description(), max_w.saturating_sub(4));
            let desc_style = if is_selected {
                Style::default().fg(Color::Gray)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let line2 = Line::from(vec![Span::raw("    "), Span::styled(desc_text, desc_style)]);

            ListItem::new(vec![Line::from(line1_spans), line2])
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(app.selected_index));

    let list = List::new(items);
    f.render_stateful_widget(list, inner, &mut state);
}
