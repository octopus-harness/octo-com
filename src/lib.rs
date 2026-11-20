pub mod app;
pub mod cli;
pub mod config;
pub mod format;
pub mod models;
pub mod store;
pub mod ui;

use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::app::App;
use crate::cli::{execute_cli, CliOutcome};
use crate::config::{Config, TuiLaunchMode};

pub fn run() {
    if let Err(e) = try_run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

pub fn try_run() -> anyhow::Result<()> {
    let config = Config::from_env_and_args()?;
    match execute_cli(&config)? {
        CliOutcome::Handled => Ok(()),
        CliOutcome::LaunchTui(mode) => run_tui(config.commands_dir, mode),
    }
}

pub fn run_tui(commands_dir: std::path::PathBuf, mode: TuiLaunchMode) -> anyhow::Result<()> {
    let mut app = App::new(commands_dir, mode);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result?;

    if let Some(out) = app.stdout_on_exit {
        print!("{}", out);
    }

    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> anyhow::Result<()> {
    loop {
        if let Some(path) = &app.pending_editor {
            let path_clone = path.clone();

            disable_raw_mode()?;
            execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture
            )?;
            terminal.show_cursor()?;

            let editor = std::env::var("EDITOR")
                .or_else(|_| std::env::var("VISUAL"))
                .unwrap_or_else(|_| "nano".to_string());

            let _ = std::process::Command::new(editor).arg(&path_clone).status();

            enable_raw_mode()?;
            execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
            terminal.clear()?;

            app.complete_external_editor();
        }

        terminal.draw(|f| ui::render(f, app))?;

        if app.should_quit {
            return Ok(());
        }

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key.code);
                }
            }
        }
    }
}
