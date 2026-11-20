pub mod command_list;
pub mod detail;
pub mod modals;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::app::App;
use crate::config::TuiLaunchMode;

pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    render_header(f, app, chunks[0]);

    // 2-Pane Master-Detail Body
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(chunks[1]);

    command_list::render(f, app, body_chunks[0]);
    detail::render(f, app, body_chunks[1]);

    render_footer(f, app, chunks[2]);

    // Modals overlay
    modals::render(f, app);
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let dir_badge = format!(" [Dir: {}] ", app.store.dir.display());
    let stats = &app.stats;
    let stats_text = format!(
        "Commands: {} (✓ {} headers) · Steps: {} · Lines: {}",
        stats.total_commands, stats.valid_headers, stats.total_steps, stats.total_lines
    );

    let mut spans = vec![
        Span::styled(
            " 🐙 OCTO-COM ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            dir_badge,
            Style::default()
                .fg(Color::LightMagenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" · {}", stats_text),
            Style::default().fg(Color::Gray),
        ),
        Span::styled(
            format!(" · [View: {}]", app.view_mode.label()),
            Style::default().fg(Color::Cyan),
        ),
    ];

    if app.launch_mode == TuiLaunchMode::SelectForGet {
        spans.push(Span::styled(
            " · [GET MODE: Enter to print & exit]",
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(query) = &app.search_query {
        spans.push(Span::styled(
            format!(" · [Filter: \"{}\"] (Esc to clear)", query),
            Style::default()
                .fg(Color::LightYellow)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let p = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(p, area);
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let status_span = if let Some((msg, is_error)) = &app.status {
        let color = if *is_error { Color::Red } else { Color::Green };
        Span::styled(
            format!(" ● {} ", msg),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(" ● Ready ", Style::default().fg(Color::Magenta))
    };

    let hints = " 1/2/Tab:Pane │ j/k:Nav │ v:Preview/Raw │ n:New │ e:Header │ a:Append │ E:$EDITOR │ d:Del │ /:Search │ y:Yank $$ │ c:Copy MD │ ?:Help │ q:Quit";

    let line = Line::from(vec![
        status_span,
        Span::styled("│", Style::default().fg(Color::DarkGray)),
        Span::styled(hints, Style::default().fg(Color::Gray)),
    ]);

    f.render_widget(Paragraph::new(line), area);
}
