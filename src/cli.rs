use std::io::{self, BufRead, IsTerminal, Write};

use anyhow::{bail, Result};

use crate::config::{Config, ConfigAction, Subcommand, TuiLaunchMode, UserSettings, VarsAction};
use crate::store::CommandStore;

pub const USAGE_TEXT: &str = "\
Usage:
  com                                      Launch the interactive Ratatui studio
  com tui                                  Launch the interactive Ratatui studio
  com add [name] [--category <cat>] [--args <args>] [--desc <desc>] [--blank]
                                           Create a .md command under ./commands (with 4-line header)
  com get [name] [--raw]                   Print contents of a .md command (interpolated by default)
  com list [--category <cat>]              List available commands (optionally filtered by category)
  com edit <name>                          Open a command definition in $EDITOR
  com rm <name>                            Delete a command definition from ./commands
  com vars [show]                          Display global variables from ~/.config/octo-com/vars.yml
  com vars edit                            Open ~/.config/octo-com/vars.yml in $EDITOR
  com config [show]                        Display current configuration and resolved directory
  com config get-dir                       Print the active commands directory
  com config set-dir <path>                Set the persistent global commands directory
  com help                                 Show this help message

Global Options:
  -d, --dir, -dir <path>                   Override commands directory (default: $OCTO_COMMANDS_DIR, ~/.config/octo-com/config.json, or ./commands)
";

pub enum CliOutcome {
    Handled,
    LaunchTui(TuiLaunchMode),
}

pub fn execute_cli(config: &Config) -> Result<CliOutcome> {
    let store = CommandStore::new(&config.commands_dir);

    match &config.subcommand {
        Subcommand::Help => {
            print!("{}", USAGE_TEXT);
            Ok(CliOutcome::Handled)
        }
        Subcommand::Tui { mode } => Ok(CliOutcome::LaunchTui(mode.clone())),
        Subcommand::Add {
            name,
            category,
            args,
            desc,
            blank,
        } => {
            let target_name = match name {
                Some(n) => n.clone(),
                None => {
                    if io::stdin().is_terminal() && io::stdout().is_terminal() {
                        return Ok(CliOutcome::LaunchTui(TuiLaunchMode::CreateModal));
                    }
                    eprint!("Command name: ");
                    let _ = io::stderr().flush();
                    let mut line = String::new();
                    io::stdin().lock().read_line(&mut line)?;
                    line.trim_end_matches(&['\r', '\n'][..]).to_string()
                }
            };

            let created = store.create_command(
                &target_name,
                category.as_deref(),
                args.as_deref(),
                desc.as_deref(),
                None,
                *blank,
            )?;
            println!("Created {}", created.path.display());
            Ok(CliOutcome::Handled)
        }
        Subcommand::Get { name, raw } => {
            let target_name = match name {
                Some(n) => n.clone(),
                None => {
                    let docs = store.list_commands()?;
                    if docs.is_empty() {
                        bail!("No commands found in {}.", store.dir.display());
                    }
                    if io::stdin().is_terminal() && io::stdout().is_terminal() {
                        return Ok(CliOutcome::LaunchTui(TuiLaunchMode::SelectForGet));
                    }
                    bail!("Command name cannot be empty.");
                }
            };

            let doc = store.get_command(&target_name)?;
            if *raw {
                print!("{}", doc.raw_content);
            } else {
                let vars = crate::vars::Vars::load();
                let res = vars.interpolate(&doc.raw_content);
                if !res.missing.is_empty() {
                    eprintln!(
                        "Warning: {} undefined variable(s): {}",
                        res.missing.len(),
                        res.missing
                            .iter()
                            .map(|s| format!("{{{{{}}}}}", s))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                print!("{}", res.rendered);
            }
            Ok(CliOutcome::Handled)
        }
        Subcommand::List { category } => {
            let mut docs = store.list_commands()?;
            if let Some(cat) = category {
                let needle = cat.trim().to_lowercase();
                docs.retain(|d| d.display_category().to_lowercase() == needle);
            }
            print!("{}", format_command_list(&docs));
            Ok(CliOutcome::Handled)
        }
        Subcommand::Edit { name } => {
            let doc = store.get_command(name)?;
            let editor = std::env::var("EDITOR")
                .or_else(|_| std::env::var("VISUAL"))
                .unwrap_or_else(|_| "nano".to_string());
            let status = std::process::Command::new(&editor)
                .arg(&doc.path)
                .status()?;
            if !status.success() {
                bail!("Editor '{}' exited with status {}.", editor, status);
            }
            Ok(CliOutcome::Handled)
        }
        Subcommand::Remove { name } => {
            let deleted_path = store.delete_command(name)?;
            println!("Removed {}", deleted_path.display());
            Ok(CliOutcome::Handled)
        }
        Subcommand::Vars(action) => {
            match action {
                VarsAction::Show => {
                    let path = crate::vars::Vars::default_path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    println!("Variables file: {}", path);
                    let vars = crate::vars::Vars::load();
                    if vars.map.is_empty() {
                        println!("No variables defined (or file is empty/missing). Run 'com vars edit' to define variables.");
                    } else {
                        println!("\n{} variables defined:", vars.map.len());
                        for (k, v) in &vars.map {
                            println!("  {}: {}", k, v);
                        }
                    }
                }
                VarsAction::Edit => {
                    let path = crate::vars::Vars::ensure_template_exists()?;
                    let editor = std::env::var("EDITOR")
                        .or_else(|_| std::env::var("VISUAL"))
                        .unwrap_or_else(|_| "nano".to_string());
                    let status = std::process::Command::new(&editor)
                        .arg(&path)
                        .status()?;
                    if !status.success() {
                        bail!("Editor '{}' exited with status {}.", editor, status);
                    }
                }
            }
            Ok(CliOutcome::Handled)
        }
        Subcommand::Config(action) => {
            match action {
                ConfigAction::Show => {
                    let path = UserSettings::default_path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    let user_settings = UserSettings::load();
                    let global_dir_str = user_settings
                        .commands_dir
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "(none)".to_string());

                    println!("Configuration file: {}", path);
                    println!("Global commands directory: {}", global_dir_str);
                    println!("Active commands directory: {}", config.commands_dir.display());
                }
                ConfigAction::GetDir => {
                    println!("{}", config.commands_dir.display());
                }
                ConfigAction::SetDir(path) => {
                    let mut settings = UserSettings::load();
                    settings.commands_dir = Some(path.clone());
                    let saved_file = settings.save()?;
                    println!(
                        "Saved global commands directory: {}\nConfig file: {}",
                        path.display(),
                        saved_file.display()
                    );
                }
            }
            Ok(CliOutcome::Handled)
        }
    }
}

pub fn format_command_list(docs: &[crate::models::CommandDoc]) -> String {
    if docs.is_empty() {
        return "0 commands available.\n".to_string();
    }

    let mut out = format!("{} commands available:\n\n", docs.len());
    let cmd_width = docs
        .iter()
        .map(|d| d.name.len() + 2)
        .max()
        .unwrap_or(7)
        .max(7);
    let cat_width = docs
        .iter()
        .map(|d| d.display_category().len())
        .max()
        .unwrap_or(8)
        .max(8);
    let args_width = docs
        .iter()
        .map(|d| d.display_args().unwrap_or("-").len())
        .max()
        .unwrap_or(4)
        .max(4);

    out.push_str(&format!(
        "{:<cmd_w$}  {:<cat_w$}  {:<args_w$}  DESCRIPTION\n",
        "COMMAND",
        "CATEGORY",
        "ARGS",
        cmd_w = cmd_width,
        cat_w = cat_width,
        args_w = args_width
    ));
    out.push_str(&format!(
        "{:-<cmd_w$}  {:-<cat_w$}  {:-<args_w$}  -----------\n",
        "",
        "",
        "",
        cmd_w = cmd_width,
        cat_w = cat_width,
        args_w = args_width
    ));

    for doc in docs {
        let cmd_col = format!("$${}", doc.name);
        let cat_col = doc.display_category();
        let args_col = doc.display_args().unwrap_or("-");
        let desc_col = doc.display_description();
        out.push_str(&format!(
            "{:<cmd_w$}  {:<cat_w$}  {:<args_w$}  {}\n",
            cmd_col,
            cat_col,
            args_col,
            desc_col,
            cmd_w = cmd_width,
            cat_w = cat_width,
            args_w = args_width
        ));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cli_add_get_list_rm() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("commands");

        let add_cfg = Config {
            commands_dir: dir.clone(),
            subcommand: Subcommand::Add {
                name: Some("hello".to_string()),
                category: Some("greetings".to_string()),
                args: Some("[name]".to_string()),
                desc: Some("Greet a user".to_string()),
                blank: false,
            },
        };
        assert!(matches!(
            execute_cli(&add_cfg).unwrap(),
            CliOutcome::Handled
        ));

        let store = CommandStore::new(&dir);
        let docs = store.list_commands().unwrap();
        assert_eq!(docs.len(), 1);
        let table = format_command_list(&docs);
        assert!(table.contains("1 commands available:"));
        assert!(table.contains("$$hello"));
        assert!(table.contains("greetings"));
        assert!(table.contains("[name]"));
        assert!(table.contains("Greet a user"));

        // Filter by category
        let list_filtered_cfg = Config {
            commands_dir: dir.clone(),
            subcommand: Subcommand::List {
                category: Some("greetings".to_string()),
            },
        };
        assert!(matches!(execute_cli(&list_filtered_cfg).unwrap(), CliOutcome::Handled));

        let list_empty_cfg = Config {
            commands_dir: dir.clone(),
            subcommand: Subcommand::List {
                category: Some("nonexistent".to_string()),
            },
        };
        assert!(matches!(execute_cli(&list_empty_cfg).unwrap(), CliOutcome::Handled));

        let rm_cfg = Config {
            commands_dir: dir.clone(),
            subcommand: Subcommand::Remove {
                name: "hello".to_string(),
            },
        };
        assert!(matches!(execute_cli(&rm_cfg).unwrap(), CliOutcome::Handled));
        assert!(store.list_commands().unwrap().is_empty());
    }

    #[test]
    fn test_cli_get_with_raw_and_interpolation() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("commands");

        let store = CommandStore::new(&dir);
        let content = "Usage: $$greet\nArgs: -\nDescription: Greet\nCategory: -\n\nHello {{name}}!";
        store.ensure_dir().unwrap();
        std::fs::write(dir.join("greet.md"), content).unwrap();

        // Test com get greet --raw
        let get_raw_cfg = Config {
            commands_dir: dir.clone(),
            subcommand: Subcommand::Get {
                name: Some("greet".to_string()),
                raw: true,
            },
        };
        assert!(matches!(execute_cli(&get_raw_cfg).unwrap(), CliOutcome::Handled));

        // Test com get greet (interpolated)
        let get_interp_cfg = Config {
            commands_dir: dir.clone(),
            subcommand: Subcommand::Get {
                name: Some("greet".to_string()),
                raw: false,
            },
        };
        assert!(matches!(execute_cli(&get_interp_cfg).unwrap(), CliOutcome::Handled));
    }
}
