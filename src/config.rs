use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuiLaunchMode {
    Browse,
    CreateModal,
    SelectForGet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigAction {
    Show,
    GetDir,
    SetDir(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarsAction {
    Show,
    Edit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subcommand {
    Tui {
        mode: TuiLaunchMode,
    },
    Add {
        name: Option<String>,
        category: Option<String>,
        args: Option<String>,
        desc: Option<String>,
        blank: bool,
    },
    Get {
        name: Option<String>,
        raw: bool,
    },
    List {
        category: Option<String>,
    },
    Edit {
        name: String,
    },
    Remove {
        name: String,
    },
    Config(ConfigAction),
    Vars(VarsAction),
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UserSettings {
    pub commands_dir: Option<PathBuf>,
}

impl UserSettings {
    pub fn default_path() -> Option<PathBuf> {
        std::env::var("HOME")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(|h| PathBuf::from(h).join(".config").join("octo-com").join("config.json"))
    }

    pub fn load() -> Self {
        Self::default_path()
            .map(|p| Self::load_from(&p))
            .unwrap_or_default()
    }

    pub fn load_from(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<PathBuf> {
        let path = Self::default_path()
            .ok_or_else(|| anyhow::anyhow!("Unable to determine user HOME directory"))?;
        self.save_to(&path)?;
        Ok(path)
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}

pub fn expand_tilde(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if s == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home);
        }
    } else if let Some(stripped) = s.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(stripped);
        }
    }
    p.to_path_buf()
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
        let user_settings = UserSettings::load();
        Self::parse_with_settings(
            &args,
            env_dir.as_deref(),
            user_settings.commands_dir.as_deref(),
            &cwd,
        )
    }

    pub fn parse(args: &[String], env_dir: Option<&str>, cwd: &Path) -> Result<Self> {
        Self::parse_with_settings(args, env_dir, None, cwd)
    }

    pub fn parse_with_settings(
        args: &[String],
        env_dir: Option<&str>,
        global_dir: Option<&Path>,
        cwd: &Path,
    ) -> Result<Self> {
        let mut dir_override: Option<PathBuf> = None;
        let mut remaining: Vec<String> = Vec::new();

        let mut i = 1;
        while i < args.len() {
            let arg = &args[i];
            match arg.as_str() {
                "--dir" | "-dir" | "-d" => {
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
                s if s.starts_with("-dir=") => {
                    let val = s.trim_start_matches("-dir=");
                    if val.trim().is_empty() {
                        bail!("Option '-dir=' requires a non-empty directory path.");
                    }
                    dir_override = Some(PathBuf::from(val));
                    i += 1;
                }
                s if s.starts_with("-d=") => {
                    let val = s.trim_start_matches("-d=");
                    if val.trim().is_empty() {
                        bail!("Option '-d=' requires a non-empty directory path.");
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
            expand_tilde(&d)
        } else if let Some(env_val) = env_dir.map(str::trim).filter(|s| !s.is_empty()) {
            expand_tilde(Path::new(env_val))
        } else if let Some(g) = global_dir {
            expand_tilde(g)
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
            "list" | "ls" => parse_list_args(&remaining[1..])?,
            "get" => parse_get_args(&remaining[1..])?,
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
            "config" => parse_config_args(&remaining[1..])?,
            "vars" | "var" => parse_vars_args(&remaining[1..])?,
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

fn parse_get_args(tokens: &[String]) -> Result<Subcommand> {
    let mut name = None;
    let mut raw = false;
    for token in tokens {
        if token == "--raw" || token == "-r" {
            raw = true;
        } else if token.starts_with('-') {
            bail!("Unknown option for 'get': '{}'.", token);
        } else if name.is_none() {
            name = Some(token.clone());
        } else {
            bail!("Unexpected argument for 'get': '{}'.", token);
        }
    }
    Ok(Subcommand::Get { name, raw })
}

fn parse_vars_args(tokens: &[String]) -> Result<Subcommand> {
    if tokens.is_empty() {
        return Ok(Subcommand::Vars(VarsAction::Show));
    }
    match tokens[0].as_str() {
        "show" | "list" | "ls" => Ok(Subcommand::Vars(VarsAction::Show)),
        "edit" | "e" => Ok(Subcommand::Vars(VarsAction::Edit)),
        other => {
            bail!("Unknown vars command: '{}'. Usage: com vars [edit]", other);
        }
    }
}

fn parse_config_args(tokens: &[String]) -> Result<Subcommand> {
    if tokens.is_empty() {
        return Ok(Subcommand::Config(ConfigAction::Show));
    }
    match tokens[0].as_str() {
        "show" => Ok(Subcommand::Config(ConfigAction::Show)),
        "get-dir" | "get" => Ok(Subcommand::Config(ConfigAction::GetDir)),
        "set-dir" => {
            if tokens.len() < 2 || tokens[1].trim().is_empty() {
                bail!("Subcommand 'set-dir' requires a directory path. Usage: com config set-dir <path>");
            }
            let path = expand_tilde(Path::new(&tokens[1]));
            Ok(Subcommand::Config(ConfigAction::SetDir(path)))
        }
        "set" => {
            if tokens.len() >= 3 && tokens[1] == "dir" {
                if tokens[2].trim().is_empty() {
                    bail!("Subcommand 'set dir' requires a directory path.");
                }
                let path = expand_tilde(Path::new(&tokens[2]));
                Ok(Subcommand::Config(ConfigAction::SetDir(path)))
            } else if tokens.len() >= 2 && tokens[1] != "dir" {
                let path = expand_tilde(Path::new(&tokens[1]));
                Ok(Subcommand::Config(ConfigAction::SetDir(path)))
            } else {
                bail!("Usage: com config set-dir <path>");
            }
        }
        other => {
            bail!(
                "Unknown config command: '{}'. Usage: com config [show|get-dir|set-dir <path>]",
                other
            );
        }
    }
}

fn parse_list_args(tokens: &[String]) -> Result<Subcommand> {
    let mut category: Option<String> = None;
    let mut i = 0;
    while i < tokens.len() {
        let tok = &tokens[i];
        match tok.as_str() {
            "--category" | "-c" => {
                if i + 1 >= tokens.len() {
                    bail!("Option '{}' requires a category value.", tok);
                }
                category = Some(tokens[i + 1].clone());
                i += 2;
            }
            s if s.starts_with("--category=") => {
                category = Some(s.trim_start_matches("--category=").to_string());
                i += 1;
            }
            s if s.starts_with("-c=") => {
                category = Some(s.trim_start_matches("-c=").to_string());
                i += 1;
            }
            s if s.starts_with('-') => {
                bail!(
                    "Unknown option for 'list': '{}'. Run 'com help' for usage.",
                    s
                );
            }
            _ => {
                if category.is_none() {
                    category = Some(tok.clone());
                } else {
                    bail!("Unexpected extra argument '{}' for 'com list'.", tok);
                }
                i += 1;
            }
        }
    }
    Ok(Subcommand::List { category })
}

fn parse_add_args(tokens: &[String]) -> Result<Subcommand> {
    let mut name: Option<String> = None;
    let mut category: Option<String> = None;
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
            "--category" | "-c" => {
                if i + 1 >= tokens.len() {
                    bail!("Option '{}' requires a category value.", tok);
                }
                category = Some(tokens[i + 1].clone());
                i += 2;
            }
            s if s.starts_with("--category=") => {
                category = Some(s.trim_start_matches("--category=").to_string());
                i += 1;
            }
            s if s.starts_with("-c=") => {
                category = Some(s.trim_start_matches("-c=").to_string());
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
        category,
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

        // 1. Default to ./commands in CWD when nothing else specified
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

        // 2. Global settings override default CWD
        let cfg_global = Config::parse_with_settings(
            &vec_s(&["com"]),
            None,
            Some(Path::new("/global/commands")),
            cwd,
        )
        .unwrap();
        assert_eq!(cfg_global.commands_dir, PathBuf::from("/global/commands"));

        // 3. Env var overrides global settings
        let cfg_env = Config::parse_with_settings(
            &vec_s(&["com"]),
            Some("/env/commands"),
            Some(Path::new("/global/commands")),
            cwd,
        )
        .unwrap();
        assert_eq!(cfg_env.commands_dir, PathBuf::from("/env/commands"));

        // 4. CLI flag (--dir, -dir, -d) overrides env var and global settings
        let cfg_flag = Config::parse_with_settings(
            &vec_s(&["com", "--dir", "/flag/commands", "list"]),
            Some("/env/commands"),
            Some(Path::new("/global/commands")),
            cwd,
        )
        .unwrap();
        assert_eq!(cfg_flag.commands_dir, PathBuf::from("/flag/commands"));
        assert_eq!(cfg_flag.subcommand, Subcommand::List { category: None });

        // Check -dir flag syntax
        let cfg_single_dash = Config::parse_with_settings(
            &vec_s(&["com", "-dir", "/single/dash/commands", "list"]),
            Some("/env/commands"),
            Some(Path::new("/global/commands")),
            cwd,
        )
        .unwrap();
        assert_eq!(
            cfg_single_dash.commands_dir,
            PathBuf::from("/single/dash/commands")
        );

        // Check -dir= flag syntax
        let cfg_single_dash_eq = Config::parse_with_settings(
            &vec_s(&["com", "-dir=/single/dash/eq", "list"]),
            None,
            None,
            cwd,
        )
        .unwrap();
        assert_eq!(
            cfg_single_dash_eq.commands_dir,
            PathBuf::from("/single/dash/eq")
        );

        // Check add and list with category
        let cfg_add = Config::parse(
            &vec_s(&["com", "add", "my_cmd", "--category", "ops", "--args", "[env]", "--desc", "Deploy app"]),
            None,
            cwd,
        )
        .unwrap();
        assert_eq!(
            cfg_add.subcommand,
            Subcommand::Add {
                name: Some("my_cmd".to_string()),
                category: Some("ops".to_string()),
                args: Some("[env]".to_string()),
                desc: Some("Deploy app".to_string()),
                blank: false,
            }
        );

        let cfg_list_cat = Config::parse(
            &vec_s(&["com", "list", "-c", "ops"]),
            None,
            cwd,
        )
        .unwrap();
        assert_eq!(
            cfg_list_cat.subcommand,
            Subcommand::List {
                category: Some("ops".to_string()),
            }
        );
    }

    #[test]
    fn test_config_subcommands() {
        let cwd = Path::new("/tmp");

        let cfg_show = Config::parse(&vec_s(&["com", "config"]), None, cwd).unwrap();
        assert_eq!(cfg_show.subcommand, Subcommand::Config(ConfigAction::Show));

        let cfg_get = Config::parse(&vec_s(&["com", "config", "get-dir"]), None, cwd).unwrap();
        assert_eq!(cfg_get.subcommand, Subcommand::Config(ConfigAction::GetDir));

        let cfg_set = Config::parse(
            &vec_s(&["com", "config", "set-dir", "/custom/commands"]),
            None,
            cwd,
        )
        .unwrap();
        assert_eq!(
            cfg_set.subcommand,
            Subcommand::Config(ConfigAction::SetDir(PathBuf::from("/custom/commands")))
        );

        let cfg_set2 = Config::parse(
            &vec_s(&["com", "config", "set", "dir", "/custom/commands2"]),
            None,
            cwd,
        )
        .unwrap();
        assert_eq!(
            cfg_set2.subcommand,
            Subcommand::Config(ConfigAction::SetDir(PathBuf::from("/custom/commands2")))
        );

        // Test vars subcommand parsing
        let cfg_vars = Config::parse(&vec_s(&["com", "vars"]), None, cwd).unwrap();
        assert_eq!(cfg_vars.subcommand, Subcommand::Vars(VarsAction::Show));

        let cfg_vars_edit = Config::parse(&vec_s(&["com", "vars", "edit"]), None, cwd).unwrap();
        assert_eq!(cfg_vars_edit.subcommand, Subcommand::Vars(VarsAction::Edit));

        // Test get subcommand parsing with and without --raw
        let cfg_get_normal = Config::parse(&vec_s(&["com", "get", "add_research"]), None, cwd).unwrap();
        assert_eq!(
            cfg_get_normal.subcommand,
            Subcommand::Get {
                name: Some("add_research".to_string()),
                raw: false,
            }
        );

        let cfg_get_raw = Config::parse(&vec_s(&["com", "get", "add_research", "--raw"]), None, cwd).unwrap();
        assert_eq!(
            cfg_get_raw.subcommand,
            Subcommand::Get {
                name: Some("add_research".to_string()),
                raw: true,
            }
        );
    }

    #[test]
    fn test_user_settings_serialization() {
        let tmp = tempfile::tempdir().unwrap();
        let config_file = tmp.path().join("config.json");

        let settings = UserSettings {
            commands_dir: Some(PathBuf::from("/path/to/commands")),
        };
        settings.save_to(&config_file).unwrap();

        let loaded = UserSettings::load_from(&config_file);
        assert_eq!(loaded.commands_dir, Some(PathBuf::from("/path/to/commands")));
    }
}
