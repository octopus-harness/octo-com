use std::path::PathBuf;

use anyhow::{bail, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuiLaunchMode {
    Browse,
    CreateModal,
    SelectForGet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subcommand {
    Tui {
        mode: TuiLaunchMode,
    },
    Add {
        name: Option<String>,
        args: Option<String>,
        desc: Option<String>,
        blank: bool,
    },
    Get {
        name: Option<String>,
    },
    List,
    Edit {
        name: String,
    },
    Remove {
        name: String,
    },
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub commands_dir: PathBuf,
    pub subcommand: Subcommand,
}

impl Config {
    pub fn from_env_and_args() -> Result<Self> {
        let args: Vec<String> = std::env::args().collect();
        let env_dir = std::env::var("OCTO_COMMANDS_DIR").ok();
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::parse(&args, env_dir.as_deref(), &cwd)
    }

    pub fn parse(args: &[String], env_dir: Option<&str>, cwd: &std::path::Path) -> Result<Self> {
        let mut dir_override: Option<PathBuf> = None;
        let mut remaining: Vec<String> = Vec::new();

        let mut i = 1;
        while i < args.len() {
            let arg = &args[i];
            match arg.as_str() {
                "--dir" | "-d" => {
                    if i + 1 >= args.len() {
                        bail!("Option '{}' requires a directory path.", arg);
                    }
                    dir_override = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                }
                s if s.starts_with("--dir=") => {
                    let val = s.trim_start_matches("--dir=");
                    if val.trim().is_empty() {
                        bail!("Option '--dir=' requires a non-empty directory path.");
                    }
                    dir_override = Some(PathBuf::from(val));
                    i += 1;
                }
                _ => {
                    remaining.push(arg.clone());
                    i += 1;
                }
            }
        }

        let commands_dir = if let Some(d) = dir_override {
            d
        } else if let Some(env_val) = env_dir.map(str::trim).filter(|s| !s.is_empty()) {
            PathBuf::from(env_val)
        } else {
            cwd.join("commands")
        };

        if remaining.is_empty() {
            return Ok(Self {
                commands_dir,
                subcommand: Subcommand::Tui {
                    mode: TuiLaunchMode::Browse,
                },
            });
        }

        let sub = remaining[0].as_str();
        let subcommand = match sub {
            "-h" | "--help" | "help" => Subcommand::Help,
            "tui" => Subcommand::Tui {
                mode: TuiLaunchMode::Browse,
            },
            "list" | "ls" => Subcommand::List,
            "get" => {
                let name = remaining.get(1).cloned();
                Subcommand::Get { name }
            }
            "add" => parse_add_args(&remaining[1..])?,
            "edit" => {
                let name = remaining
                    .get(1)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("Command name cannot be empty."))?;
                Subcommand::Edit { name }
            }
            "rm" | "remove" | "delete" => {
                let name = remaining
                    .get(1)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("Command name cannot be empty."))?;
                Subcommand::Remove { name }
            }
            other => {
                bail!("Unknown command: '{}'. Run 'com help' for usage.", other);
            }
        };

        Ok(Self {
            commands_dir,
            subcommand,
        })
    }
}

fn parse_add_args(tokens: &[String]) -> Result<Subcommand> {
    let mut name: Option<String> = None;
    let mut args: Option<String> = None;
    let mut desc: Option<String> = None;
    let mut blank = false;

    let mut i = 0;
    while i < tokens.len() {
        let tok = &tokens[i];
        match tok.as_str() {
            "--blank" | "-b" => {
                blank = true;
                i += 1;
            }
            "--args" | "-a" => {
                if i + 1 >= tokens.len() {
                    bail!("Option '{}' requires an argument value.", tok);
                }
                args = Some(tokens[i + 1].clone());
                i += 2;
            }
            s if s.starts_with("--args=") => {
                args = Some(s.trim_start_matches("--args=").to_string());
                i += 1;
            }
            "--desc" | "--description" | "-m" => {
                if i + 1 >= tokens.len() {
                    bail!("Option '{}' requires a description string.", tok);
                }
                desc = Some(tokens[i + 1].clone());
                i += 2;
            }
            s if s.starts_with("--desc=") => {
                desc = Some(s.trim_start_matches("--desc=").to_string());
                i += 1;
            }
            s if s.starts_with('-') => {
                bail!(
                    "Unknown option for 'add': '{}'. Run 'com help' for usage.",
                    s
                );
            }
            _ => {
                if name.is_none() {
                    name = Some(tok.clone());
                } else {
                    bail!("Unexpected extra argument '{}' for 'com add'.", tok);
                }
                i += 1;
            }
        }
    }

    Ok(Subcommand::Add {
        name,
        args,
        desc,
        blank,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn vec_s(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_dir_resolution_precedence() {
        let cwd = Path::new("/workspace/octopus");

        // Default to ./commands in CWD
        let cfg = Config::parse(&vec_s(&["com"]), None, cwd).unwrap();
        assert_eq!(
            cfg.commands_dir,
            PathBuf::from("/workspace/octopus/commands")
        );
        assert_eq!(
            cfg.subcommand,
            Subcommand::Tui {
                mode: TuiLaunchMode::Browse
            }
        );

        // Env var overrides default
        let cfg_env = Config::parse(&vec_s(&["com"]), Some("/env/commands"), cwd).unwrap();
        assert_eq!(cfg_env.commands_dir, PathBuf::from("/env/commands"));

        // --dir overrides env var
        let cfg_flag = Config::parse(
            &vec_s(&["com", "--dir", "/flag/commands", "list"]),
            Some("/env/commands"),
            cwd,
        )
        .unwrap();
        assert_eq!(cfg_flag.commands_dir, PathBuf::from("/flag/commands"));
        assert_eq!(cfg_flag.subcommand, Subcommand::List);
    }

    #[test]
    fn test_subcommand_parsing() {
        let cwd = Path::new("/tmp");

        let add = Config::parse(
            &vec_s(&[
                "com",
                "add",
                "my_cmd",
                "--args",
                "[url]",
                "--desc",
                "Fetch URL",
            ]),
            None,
            cwd,
        )
        .unwrap();
        assert_eq!(
            add.subcommand,
            Subcommand::Add {
                name: Some("my_cmd".to_string()),
                args: Some("[url]".to_string()),
                desc: Some("Fetch URL".to_string()),
                blank: false,
            }
        );

        let get = Config::parse(&vec_s(&["com", "get", "help"]), None, cwd).unwrap();
        assert_eq!(
            get.subcommand,
            Subcommand::Get {
                name: Some("help".to_string())
            }
        );

        let unknown = Config::parse(&vec_s(&["com", "bogus"]), None, cwd);
        assert!(unknown.is_err());
    }
}
