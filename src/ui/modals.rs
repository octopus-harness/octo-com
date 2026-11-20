use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, Modal};
use crate::models::{format_usage_line, highest_numbered_step};

pub fn render(f: &mut Frame, app: &App) {
    let modal = match &app.modal {
        Some(m) => m,
        None => return,
    };

    match modal {
        Modal::Search { input } => render_search(f, input),
        Modal::CreateCommand {
            name_input,
            args_input,
            desc_input,
            tool_use_allowed,
            focus_field,
        } => render_create_command(
            f,
            name_input,
            args_input,
            desc_input,
            *tool_use_allowed,
            *focus_field,
        ),
        Modal::EditHeader {
            original_name,
            name_input,
            args_input,
            desc_input,
            focus_field,
        } => render_edit_header(
            f,
            original_name,
            name_input,
            args_input,
            desc_input,
            *focus_field,
        ),
        Modal::QuickAppend {
            content_input,
            as_step,
            focus_field,
        } => render_quick_append(f, app, content_input, *as_step, *focus_field),
        Modal::ConfirmDelete { name, filename } => render_confirm_delete(f, name, filename),
        Modal::Help => render_help(f),
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn render_search(f: &mut Frame, input: &str) {
    let area = centered_rect(55, 22, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Search Commands (/) ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Magenta));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let lines = vec![
        Line::from(Span::styled(
            "Filter by command name, args, description, or Markdown body:",
            Style::default().fg(Color::Gray),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                " ▶ ",
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}_", input),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Enter: Apply filter (empty to clear) │ Esc: Cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn render_create_command(
    f: &mut Frame,
    name_input: &str,
    args_input: &str,
    desc_input: &str,
    tool_use_allowed: bool,
    focus_field: usize,
) {
    let area = centered_rect(65, 52, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" New Octopus $$ Command (n) ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Magenta));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let clean_name = name_input
        .trim()
        .strip_prefix("$$")
        .unwrap_or(name_input.trim())
        .strip_suffix(".md")
        .unwrap_or(name_input.trim());
    let preview_name = if clean_name.is_empty() {
        "<name>"
    } else {
        clean_name
    };
    let preview_usage = format_usage_line(preview_name, args_input);

    let field_style = |idx: usize| {
        if focus_field == idx {
            Style::default()
                .fg(Color::LightMagenta)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        }
    };

    let cursor_str = |idx: usize| if focus_field == idx { "_" } else { "" };
    let prefix_str = |idx: usize| if focus_field == idx { " ▶ " } else { "   " };

    let check_mark = if tool_use_allowed { "[x]" } else { "[ ]" };

    let lines = vec![
        Line::from(vec![
            Span::styled("Preview Usage: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                preview_usage,
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  (./commands/{}.md)", preview_name),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(prefix_str(0), field_style(0)),
            Span::styled("Command Name: ", field_style(0)),
            Span::styled(
                format!("{}{}", name_input, cursor_str(0)),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(prefix_str(1), field_style(1)),
            Span::styled("Args (or '-'): ", field_style(1)),
            Span::styled(
                format!("{}{}", args_input, cursor_str(1)),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(prefix_str(2), field_style(2)),
            Span::styled("Description:  ", field_style(2)),
            Span::styled(
                format!("{}{}", desc_input, cursor_str(2)),
                Style::default().fg(Color::LightYellow),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(prefix_str(3), field_style(3)),
            Span::styled(
                format!(
                    "{} Include 'Tool use is allowed for this command.' starter",
                    check_mark
                ),
                field_style(3),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Tab/↑/↓: Next/Prev field │ Space: Toggle checkbox │ Enter: Create │ Esc: Cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn render_edit_header(
    f: &mut Frame,
    original_name: &str,
    name_input: &str,
    args_input: &str,
    desc_input: &str,
    focus_field: usize,
) {
    let area = centered_rect(65, 48, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(format!(" Edit Header / Rename $${} (e) ", original_name))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Magenta));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let clean_name = name_input
        .trim()
        .strip_prefix("$$")
        .unwrap_or(name_input.trim())
        .strip_suffix(".md")
        .unwrap_or(name_input.trim());
    let preview_name = if clean_name.is_empty() {
        original_name
    } else {
        clean_name
    };
    let preview_usage = format_usage_line(preview_name, args_input);

    let field_style = |idx: usize| {
        if focus_field == idx {
            Style::default()
                .fg(Color::LightMagenta)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        }
    };
    let cursor_str = |idx: usize| if focus_field == idx { "_" } else { "" };
    let prefix_str = |idx: usize| if focus_field == idx { " ▶ " } else { "   " };

    let lines = vec![
        Line::from(vec![
            Span::styled("Computed Usage: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                preview_usage,
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(prefix_str(0), field_style(0)),
            Span::styled("Command Name: ", field_style(0)),
            Span::styled(
                format!("{}{}", name_input, cursor_str(0)),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(prefix_str(1), field_style(1)),
            Span::styled("Args (or '-'): ", field_style(1)),
            Span::styled(
                format!("{}{}", args_input, cursor_str(1)),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(prefix_str(2), field_style(2)),
            Span::styled("Description:  ", field_style(2)),
            Span::styled(
                format!("{}{}", desc_input, cursor_str(2)),
                Style::default().fg(Color::LightYellow),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Tab/↑/↓: Switch field │ Enter: Save 3-line header │ Esc: Cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn render_quick_append(
    f: &mut Frame,
    app: &App,
    content_input: &str,
    as_step: bool,
    focus_field: usize,
) {
    let area = centered_rect(65, 40, f.area());
    f.render_widget(Clear, area);

    let (cmd_name, next_step) = match app.current_command() {
        Some(c) => (c.name.as_str(), highest_numbered_step(&c.body) + 1),
        None => ("command", 1),
    };

    let block = Block::default()
        .title(format!(" Quick Append to $${} (a) ", cmd_name))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Magenta));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let field_style = |idx: usize| {
        if focus_field == idx {
            Style::default()
                .fg(Color::LightMagenta)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        }
    };
    let cursor_str = |idx: usize| if focus_field == idx { "_" } else { "" };
    let prefix_str = |idx: usize| if focus_field == idx { " ▶ " } else { "   " };
    let check_mark = if as_step { "[x]" } else { "[ ]" };

    let step_prefix = if as_step {
        format!("{}. ", next_step)
    } else {
        String::new()
    };

    let lines = vec![
        Line::from(Span::styled(
            "Append an instruction step or Markdown line without leaving the TUI:",
            Style::default().fg(Color::Gray),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(prefix_str(0), field_style(0)),
            Span::styled("Instruction: ", field_style(0)),
            Span::styled(
                step_prefix,
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}{}", content_input, cursor_str(0)),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(prefix_str(1), field_style(1)),
            Span::styled(
                format!(
                    "{} Format as numbered step (step #{})",
                    check_mark, next_step
                ),
                field_style(1),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Tab: Switch field │ Space: Toggle numbered step │ Enter: Append │ Esc: Cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn render_confirm_delete(f: &mut Frame, name: &str, filename: &str) {
    let area = centered_rect(50, 25, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Confirm Delete (d) ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Red));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let lines = vec![
        Line::from(vec![
            Span::styled("Delete command ", Style::default().fg(Color::White)),
            Span::styled(
                format!("$${}", name),
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" ({})?", filename),
                Style::default().fg(Color::Gray),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "This will permanently delete the .md file from disk.",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                " [y / Enter] Delete ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled(" [n / Esc] Cancel ", Style::default().fg(Color::Gray)),
        ]),
    ];

    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn render_help(f: &mut Frame) {
    let area = centered_rect(70, 72, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" 🐙 octo-com Keyboard Shortcuts (?) ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Magenta));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let row = |k: &'static str, desc: &'static str| {
        Line::from(vec![
            Span::styled(
                format!("  {:<16}", k),
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(desc, Style::default().fg(Color::White)),
        ])
    };

    let lines = vec![
        Line::from(Span::styled(
            "Navigation & Layout",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        row("1, 2", "Focus Pane 1 (Commands) or Pane 2 (Command Detail)"),
        row(
            "Tab / h / l",
            "Switch focus between Commands list and Detail pane",
        ),
        row(
            "j / k / ↑ / ↓",
            "Move selection in Commands list or scroll Detail pane",
        ),
        row(
            "J / K / PgDn/Up",
            "Fast-scroll Command Detail pane down / up",
        ),
        row("g / G", "Jump to top / bottom of active pane"),
        row(
            "v / m",
            "Toggle between Preview (styled GFM) and Raw Markdown view",
        ),
        Line::from(""),
        Line::from(Span::styled(
            "Command Management (CRUD)",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        row(
            "n",
            "New Command (scaffolds Usage / Args / Description header)",
        ),
        row(
            "e",
            "Edit 3-line Header (Usage, Args, Description) or Rename file",
        ),
        row(
            "a",
            "Quick Append numbered execution step or Markdown paragraph",
        ),
        row(
            "E",
            "Open command .md file in external $EDITOR (nvim/vim/nano)",
        ),
        row("d", "Delete selected command .md file with confirmation"),
        row("r", "Reload commands directory from disk"),
        Line::from(""),
        Line::from(Span::styled(
            "Search & Clipboard",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        row(
            "/",
            "Live search across command names, args, descriptions, and body",
        ),
        row("y", "Yank command invocation ($$name) to system clipboard"),
        row(
            "c / Y",
            "Copy full raw command Markdown to system clipboard",
        ),
        row(
            "? / Esc / q",
            "Toggle Help / Clear search filter / Quit octo-com",
        ),
    ];

    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}
