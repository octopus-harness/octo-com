use ratatui::{backend::TestBackend, Terminal};

use octo_com::app::{App, Pane, ViewMode};
use octo_com::config::TuiLaunchMode;
use octo_com::store::CommandStore;
use octo_com::ui;
use tempfile::TempDir;

#[test]
fn test_tui_render_smoke() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("commands");
    let store = CommandStore::new(&dir);

    store
        .create_command(
            "content_prompt",
            Some("content"),
            None,
            Some("Pull todos and draft article prompt"),
            Some("Tool use is allowed for this command.\n\n1. Pull todos from `doer`.\n2. Merge into `devbook`."),
            false,
        )
        .unwrap();

    let mut app = App::new(dir, TuiLaunchMode::Browse);
    let backend = TestBackend::new(120, 35);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal.draw(|f| ui::render(f, &app)).unwrap();

    app.focused_pane = Pane::Detail;
    app.view_mode = ViewMode::Raw;
    terminal.draw(|f| ui::render(f, &app)).unwrap();
}
