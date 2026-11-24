use std::path::PathBuf;

use crossterm::event::KeyCode;

use crate::config::TuiLaunchMode;
use crate::models::CommandDoc;
use crate::store::CommandStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    CommandList,
    Detail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Preview,
    Raw,
}

impl ViewMode {
    pub fn toggle(self) -> Self {
        match self {
            ViewMode::Preview => ViewMode::Raw,
            ViewMode::Raw => ViewMode::Preview,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ViewMode::Preview => "Preview (GFM)",
            ViewMode::Raw => "Raw",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modal {
    Search {
        input: String,
    },
    CreateCommand {
        name_input: String,
        category_input: String,
        args_input: String,
        desc_input: String,
        tool_use_allowed: bool,
        focus_field: usize, // 0: Name, 1: Category, 2: Args, 3: Description, 4: ToolUse checkbox
    },
    EditHeader {
        original_name: String,
        name_input: String,
        category_input: String,
        args_input: String,
        desc_input: String,
        focus_field: usize, // 0: Name, 1: Category, 2: Args, 3: Description
    },
    QuickAppend {
        content_input: String,
        as_step: bool,
        focus_field: usize, // 0: Content, 1: as_step checkbox
    },
    ConfirmDelete {
        name: String,
        filename: String,
    },
    Help,
}

#[derive(Debug, Clone, Default)]
pub struct WorkspaceStats {
    pub total_commands: usize,
    pub valid_headers: usize,
    pub total_steps: usize,
    pub total_lines: usize,
}

pub struct App {
    pub store: CommandStore,
    pub launch_mode: TuiLaunchMode,
    pub all_commands: Vec<CommandDoc>,
    pub filtered_commands: Vec<CommandDoc>,
    pub selected_index: usize,
    pub focused_pane: Pane,
    pub view_mode: ViewMode,
    pub detail_scroll: usize,
    pub search_query: Option<String>,
    pub modal: Option<Modal>,
    pub status: Option<(String, bool)>,
    pub stats: WorkspaceStats,
    pub should_quit: bool,
    pub pending_editor: Option<PathBuf>,
    pub stdout_on_exit: Option<String>,
}

impl App {
    pub fn new(commands_dir: PathBuf, launch_mode: TuiLaunchMode) -> Self {
        let store = CommandStore::new(commands_dir);
        let initial_modal = match launch_mode {
            TuiLaunchMode::CreateModal => Some(Modal::CreateCommand {
                name_input: String::new(),
                category_input: String::new(),
                args_input: "-".to_string(),
                desc_input: String::new(),
                tool_use_allowed: true,
                focus_field: 0,
            }),
            _ => None,
        };

        let mut app = Self {
            store,
            launch_mode,
            all_commands: Vec::new(),
            filtered_commands: Vec::new(),
            selected_index: 0,
            focused_pane: Pane::CommandList,
            view_mode: ViewMode::Preview,
            detail_scroll: 0,
            search_query: None,
            modal: initial_modal,
            status: None,
            stats: WorkspaceStats::default(),
            should_quit: false,
            pending_editor: None,
            stdout_on_exit: None,
        };
        app.reload(None);
        app
    }

    pub fn current_command(&self) -> Option<&CommandDoc> {
        self.filtered_commands.get(self.selected_index)
    }

    pub fn reload(&mut self, select_name: Option<&str>) {
        let prev_name = select_name
            .map(ToOwned::to_owned)
            .or_else(|| self.current_command().map(|c| c.name.clone()));

        match self.store.list_commands() {
            Ok(docs) => {
                self.all_commands = docs;
                self.recompute_stats();
                self.apply_filter(prev_name.as_deref());
            }
            Err(e) => {
                self.set_error(format!("Failed to load commands: {}", e));
            }
        }
    }

    fn recompute_stats(&mut self) {
        let total_commands = self.all_commands.len();
        let valid_headers = self
            .all_commands
            .iter()
            .filter(|c| c.has_valid_header)
            .count();
        let total_steps = self.all_commands.iter().map(|c| c.step_count()).sum();
        let total_lines = self
            .all_commands
            .iter()
            .map(|c| c.raw_content.lines().count())
            .sum();

        self.stats = WorkspaceStats {
            total_commands,
            valid_headers,
            total_steps,
            total_lines,
        };
    }

    fn apply_filter(&mut self, select_name: Option<&str>) {
        if let Some(q) = &self.search_query {
            let needle = q.to_lowercase();
            self.filtered_commands = self
                .all_commands
                .iter()
                .filter(|c| {
                    c.name.to_lowercase().contains(&needle)
                        || c.display_description().to_lowercase().contains(&needle)
                        || c.display_category().to_lowercase().contains(&needle)
                        || c.display_args()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&needle)
                        || c.body.to_lowercase().contains(&needle)
                })
                .cloned()
                .collect();
        } else {
            self.filtered_commands = self.all_commands.clone();
        }

        if let Some(target) = select_name {
            if let Some(pos) = self
                .filtered_commands
                .iter()
                .position(|c| c.name.eq_ignore_ascii_case(target))
            {
                self.selected_index = pos;
                return;
            }
        }

        if self.selected_index >= self.filtered_commands.len() {
            self.selected_index = self.filtered_commands.len().saturating_sub(1);
        }
    }

    pub fn set_info(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), false));
    }

    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), true));
    }

    pub fn complete_external_editor(&mut self) {
        let edited_path = self.pending_editor.take();
        self.reload(None);
        if let Some(path) = edited_path {
            if let Some(fname) = path.file_name().and_then(|s| s.to_str()) {
                self.set_info(format!("Reloaded {}", fname));
            }
        }
    }

    pub fn handle_key(&mut self, code: KeyCode) {
        if self.modal.is_some() {
            self.handle_modal_key(code);
            return;
        }

        match code {
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Esc => {
                if self.search_query.is_some() {
                    let prev = self.current_command().map(|c| c.name.clone());
                    self.search_query = None;
                    self.apply_filter(prev.as_deref());
                    self.set_info("Cleared search filter");
                } else if self.focused_pane == Pane::Detail {
                    self.focused_pane = Pane::CommandList;
                } else {
                    self.should_quit = true;
                }
            }
            KeyCode::Char('1') => {
                self.focused_pane = Pane::CommandList;
            }
            KeyCode::Char('2') => {
                self.focused_pane = Pane::Detail;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.focused_pane = match self.focused_pane {
                    Pane::CommandList => Pane::Detail,
                    Pane::Detail => Pane::CommandList,
                };
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.focused_pane = Pane::CommandList;
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.focused_pane = Pane::Detail;
            }
            KeyCode::Enter => {
                if self.launch_mode == TuiLaunchMode::SelectForGet {
                    if let Some(cmd) = self.current_command() {
                        self.stdout_on_exit = Some(cmd.raw_content.clone());
                        self.should_quit = true;
                    }
                } else if self.focused_pane == Pane::CommandList {
                    self.focused_pane = Pane::Detail;
                }
            }
            KeyCode::Char('j') | KeyCode::Down => match self.focused_pane {
                Pane::CommandList => {
                    if !self.filtered_commands.is_empty() {
                        self.selected_index =
                            (self.selected_index + 1) % self.filtered_commands.len();
                        self.detail_scroll = 0;
                    }
                }
                Pane::Detail => {
                    self.detail_scroll = self.detail_scroll.saturating_add(1);
                }
            },
            KeyCode::Char('k') | KeyCode::Up => match self.focused_pane {
                Pane::CommandList => {
                    if !self.filtered_commands.is_empty() {
                        if self.selected_index == 0 {
                            self.selected_index = self.filtered_commands.len() - 1;
                        } else {
                            self.selected_index -= 1;
                        }
                        self.detail_scroll = 0;
                    }
                }
                Pane::Detail => {
                    self.detail_scroll = self.detail_scroll.saturating_sub(1);
                }
            },
            KeyCode::Char('J') | KeyCode::PageDown => {
                self.detail_scroll = self.detail_scroll.saturating_add(5);
            }
            KeyCode::Char('K') | KeyCode::PageUp => {
                self.detail_scroll = self.detail_scroll.saturating_sub(5);
            }
            KeyCode::Char('g') | KeyCode::Home => {
                if self.focused_pane == Pane::CommandList {
                    self.selected_index = 0;
                    self.detail_scroll = 0;
                } else {
                    self.detail_scroll = 0;
                }
            }
            KeyCode::Char('G') | KeyCode::End => {
                if self.focused_pane == Pane::CommandList {
                    if !self.filtered_commands.is_empty() {
                        self.selected_index = self.filtered_commands.len() - 1;
                        self.detail_scroll = 0;
                    }
                } else if let Some(cmd) = self.current_command() {
                    let line_count = cmd.body.lines().count();
                    self.detail_scroll = line_count.saturating_sub(5);
                }
            }
            KeyCode::Char('v') | KeyCode::Char('m') => {
                self.view_mode = self.view_mode.toggle();
                self.set_info(format!("Switched view to {}", self.view_mode.label()));
            }
            KeyCode::Char('/') => {
                let current = self.search_query.clone().unwrap_or_default();
                self.modal = Some(Modal::Search { input: current });
            }
            KeyCode::Char('n') => {
                self.modal = Some(Modal::CreateCommand {
                    name_input: String::new(),
                    category_input: String::new(),
                    args_input: "-".to_string(),
                    desc_input: String::new(),
                    tool_use_allowed: true,
                    focus_field: 0,
                });
            }
            KeyCode::Char('e') => {
                if let Some(cmd) = self.current_command() {
                    self.modal = Some(Modal::EditHeader {
                        original_name: cmd.name.clone(),
                        name_input: cmd.name.clone(),
                        category_input: cmd.category.clone().unwrap_or_default(),
                        args_input: cmd.args.clone().unwrap_or_else(|| "-".to_string()),
                        desc_input: cmd.description.clone().unwrap_or_default(),
                        focus_field: 0,
                    });
                } else {
                    self.set_error("No command selected to edit");
                }
            }
            KeyCode::Char('a') => {
                if self.current_command().is_some() {
                    self.modal = Some(Modal::QuickAppend {
                        content_input: String::new(),
                        as_step: true,
                        focus_field: 0,
                    });
                } else {
                    self.set_error("No command selected to append to");
                }
            }
            KeyCode::Char('E') => {
                if let Some(cmd) = self.current_command() {
                    self.pending_editor = Some(cmd.path.clone());
                } else {
                    self.set_error("No command selected to open in $EDITOR");
                }
            }
            KeyCode::Char('d') => {
                if let Some(cmd) = self.current_command() {
                    self.modal = Some(Modal::ConfirmDelete {
                        name: cmd.name.clone(),
                        filename: cmd.filename.clone(),
                    });
                } else {
                    self.set_error("No command selected to delete");
                }
            }
            KeyCode::Char('y') => {
                if let Some(cmd) = self.current_command() {
                    let invocation = format!("$${}", cmd.name);
                    self.copy_to_clipboard(
                        &invocation,
                        &format!("Copied '{}' to clipboard", invocation),
                    );
                }
            }
            KeyCode::Char('c') | KeyCode::Char('Y') => {
                if let Some(cmd) = self.current_command() {
                    let raw = cmd.raw_content.clone();
                    let fname = cmd.filename.clone();
                    self.copy_to_clipboard(
                        &raw,
                        &format!("Copied {} Markdown to clipboard", fname),
                    );
                }
            }
            KeyCode::Char('r') => {
                self.reload(None);
                self.set_info("Reloaded commands from disk");
            }
            KeyCode::Char('?') => {
                self.modal = Some(Modal::Help);
            }
            _ => {}
        }
    }

    fn copy_to_clipboard(&mut self, text: &str, success_msg: &str) {
        match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text.to_string())) {
            Ok(_) => self.set_info(success_msg.to_string()),
            Err(e) => self.set_error(format!("Clipboard error: {}", e)),
        }
    }

    fn handle_modal_key(&mut self, code: KeyCode) {
        let modal = match self.modal.take() {
            Some(m) => m,
            None => return,
        };

        match modal {
            Modal::Help => match code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('?') => {}
                _ => {
                    self.modal = Some(Modal::Help);
                }
            },
            Modal::Search { mut input } => match code {
                KeyCode::Esc => {
                    // Cancel search dialog without clearing existing filter
                }
                KeyCode::Enter => {
                    let trimmed = input.trim().to_string();
                    if trimmed.is_empty() {
                        self.search_query = None;
                        self.apply_filter(None);
                        self.set_info("Cleared search filter");
                    } else {
                        self.search_query = Some(trimmed.clone());
                        self.apply_filter(None);
                        self.set_info(format!(
                            "Filtered by \"{}\" ({} matches)",
                            trimmed,
                            self.filtered_commands.len()
                        ));
                    }
                }
                KeyCode::Backspace => {
                    input.pop();
                    self.modal = Some(Modal::Search { input });
                }
                KeyCode::Char(c) => {
                    input.push(c);
                    self.modal = Some(Modal::Search { input });
                }
                _ => {
                    self.modal = Some(Modal::Search { input });
                }
            },
            Modal::CreateCommand {
                mut name_input,
                mut category_input,
                mut args_input,
                mut desc_input,
                mut tool_use_allowed,
                mut focus_field,
            } => match code {
                KeyCode::Esc => {}
                KeyCode::Tab | KeyCode::Down => {
                    focus_field = (focus_field + 1) % 5;
                    self.modal = Some(Modal::CreateCommand {
                        name_input,
                        category_input,
                        args_input,
                        desc_input,
                        tool_use_allowed,
                        focus_field,
                    });
                }
                KeyCode::BackTab | KeyCode::Up => {
                    focus_field = (focus_field + 4) % 5;
                    self.modal = Some(Modal::CreateCommand {
                        name_input,
                        category_input,
                        args_input,
                        desc_input,
                        tool_use_allowed,
                        focus_field,
                    });
                }
                KeyCode::Char(' ') if focus_field == 4 => {
                    tool_use_allowed = !tool_use_allowed;
                    self.modal = Some(Modal::CreateCommand {
                        name_input,
                        category_input,
                        args_input,
                        desc_input,
                        tool_use_allowed,
                        focus_field,
                    });
                }
                KeyCode::Enter => {
                    let starter_body = if tool_use_allowed {
                        Some("Tool use is allowed for this command.\n\n1. ")
                    } else {
                        None
                    };
                    let desc_opt = if desc_input.trim().is_empty() {
                        None
                    } else {
                        Some(desc_input.trim())
                    };
                    let cat_opt = if category_input.trim().is_empty() {
                        None
                    } else {
                        Some(category_input.trim())
                    };
                    match self.store.create_command(
                        &name_input,
                        cat_opt,
                        Some(&args_input),
                        desc_opt,
                        starter_body,
                        false,
                    ) {
                        Ok(doc) => {
                            let created_name = doc.name.clone();
                            self.reload(Some(&created_name));
                            self.set_info(format!("Created $${} ({})", doc.name, doc.filename));
                        }
                        Err(e) => {
                            self.set_error(e.to_string());
                            self.modal = Some(Modal::CreateCommand {
                                name_input,
                                category_input,
                                args_input,
                                desc_input,
                                tool_use_allowed,
                                focus_field,
                            });
                        }
                    }
                }
                KeyCode::Backspace => {
                    match focus_field {
                        0 => {
                            name_input.pop();
                        }
                        1 => {
                            category_input.pop();
                        }
                        2 => {
                            args_input.pop();
                        }
                        3 => {
                            desc_input.pop();
                        }
                        _ => {}
                    }
                    self.modal = Some(Modal::CreateCommand {
                        name_input,
                        category_input,
                        args_input,
                        desc_input,
                        tool_use_allowed,
                        focus_field,
                    });
                }
                KeyCode::Char(c) => {
                    match focus_field {
                        0 => name_input.push(c),
                        1 => category_input.push(c),
                        2 => args_input.push(c),
                        3 => desc_input.push(c),
                        _ => {}
                    }
                    self.modal = Some(Modal::CreateCommand {
                        name_input,
                        category_input,
                        args_input,
                        desc_input,
                        tool_use_allowed,
                        focus_field,
                    });
                }
                _ => {
                    self.modal = Some(Modal::CreateCommand {
                        name_input,
                        category_input,
                        args_input,
                        desc_input,
                        tool_use_allowed,
                        focus_field,
                    });
                }
            },
            Modal::EditHeader {
                original_name,
                mut name_input,
                mut category_input,
                mut args_input,
                mut desc_input,
                mut focus_field,
            } => match code {
                KeyCode::Esc => {}
                KeyCode::Tab | KeyCode::Down => {
                    focus_field = (focus_field + 1) % 4;
                    self.modal = Some(Modal::EditHeader {
                        original_name,
                        name_input,
                        category_input,
                        args_input,
                        desc_input,
                        focus_field,
                    });
                }
                KeyCode::BackTab | KeyCode::Up => {
                    focus_field = (focus_field + 3) % 4;
                    self.modal = Some(Modal::EditHeader {
                        original_name,
                        name_input,
                        category_input,
                        args_input,
                        desc_input,
                        focus_field,
                    });
                }
                KeyCode::Enter => {
                    match self.store.update_command_header(
                        &original_name,
                        &name_input,
                        &category_input,
                        &args_input,
                        &desc_input,
                    ) {
                        Ok(doc) => {
                            let new_name = doc.name.clone();
                            self.reload(Some(&new_name));
                            self.set_info(format!("Updated $${} header", doc.name));
                        }
                        Err(e) => {
                            self.set_error(e.to_string());
                            self.modal = Some(Modal::EditHeader {
                                original_name,
                                name_input,
                                category_input,
                                args_input,
                                desc_input,
                                focus_field,
                            });
                        }
                    }
                }
                KeyCode::Backspace => {
                    match focus_field {
                        0 => {
                            name_input.pop();
                        }
                        1 => {
                            category_input.pop();
                        }
                        2 => {
                            args_input.pop();
                        }
                        3 => {
                            desc_input.pop();
                        }
                        _ => {}
                    }
                    self.modal = Some(Modal::EditHeader {
                        original_name,
                        name_input,
                        category_input,
                        args_input,
                        desc_input,
                        focus_field,
                    });
                }
                KeyCode::Char(c) => {
                    match focus_field {
                        0 => name_input.push(c),
                        1 => category_input.push(c),
                        2 => args_input.push(c),
                        3 => desc_input.push(c),
                        _ => {}
                    }
                    self.modal = Some(Modal::EditHeader {
                        original_name,
                        name_input,
                        category_input,
                        args_input,
                        desc_input,
                        focus_field,
                    });
                }
                _ => {
                    self.modal = Some(Modal::EditHeader {
                        original_name,
                        name_input,
                        category_input,
                        args_input,
                        desc_input,
                        focus_field,
                    });
                }
            },
            Modal::QuickAppend {
                mut content_input,
                mut as_step,
                mut focus_field,
            } => match code {
                KeyCode::Esc => {}
                KeyCode::Tab | KeyCode::BackTab | KeyCode::Down | KeyCode::Up => {
                    focus_field = (focus_field + 1) % 2;
                    self.modal = Some(Modal::QuickAppend {
                        content_input,
                        as_step,
                        focus_field,
                    });
                }
                KeyCode::Char(' ') if focus_field == 1 => {
                    as_step = !as_step;
                    self.modal = Some(Modal::QuickAppend {
                        content_input,
                        as_step,
                        focus_field,
                    });
                }
                KeyCode::Enter => {
                    if let Some(cmd) = self.current_command() {
                        let cmd_name = cmd.name.clone();
                        match self
                            .store
                            .append_to_command(&cmd_name, &content_input, as_step)
                        {
                            Ok(doc) => {
                                self.reload(Some(&cmd_name));
                                self.set_info(format!("Appended to $${}", doc.name));
                            }
                            Err(e) => {
                                self.set_error(e.to_string());
                                self.modal = Some(Modal::QuickAppend {
                                    content_input,
                                    as_step,
                                    focus_field,
                                });
                            }
                        }
                    }
                }
                KeyCode::Backspace => {
                    if focus_field == 0 {
                        content_input.pop();
                    }
                    self.modal = Some(Modal::QuickAppend {
                        content_input,
                        as_step,
                        focus_field,
                    });
                }
                KeyCode::Char(c) => {
                    if focus_field == 0 {
                        content_input.push(c);
                    }
                    self.modal = Some(Modal::QuickAppend {
                        content_input,
                        as_step,
                        focus_field,
                    });
                }
                _ => {
                    self.modal = Some(Modal::QuickAppend {
                        content_input,
                        as_step,
                        focus_field,
                    });
                }
            },
            Modal::ConfirmDelete { name, filename } => match code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    match self.store.delete_command(&name) {
                        Ok(_) => {
                            self.reload(None);
                            self.set_info(format!("Deleted {}", filename));
                        }
                        Err(e) => {
                            self.set_error(e.to_string());
                        }
                    }
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {}
                _ => {
                    self.modal = Some(Modal::ConfirmDelete { name, filename });
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_app_create_search_append_delete_flow() {
        let tmp = TempDir::new().unwrap();
        let mut app = App::new(tmp.path().join("commands"), TuiLaunchMode::Browse);
        assert_eq!(app.all_commands.len(), 0);

        // Press 'n' to open CreateCommand modal
        app.handle_key(KeyCode::Char('n'));
        assert!(matches!(app.modal, Some(Modal::CreateCommand { .. })));

        // Type name "test_flow"
        for ch in "test_flow".chars() {
            app.handle_key(KeyCode::Char(ch));
        }
        // Tab once to Category field
        app.handle_key(KeyCode::Tab);
        for ch in "testing".chars() {
            app.handle_key(KeyCode::Char(ch));
        }
        // Tab twice to Description field (skipping Args)
        app.handle_key(KeyCode::Tab);
        app.handle_key(KeyCode::Tab);
        for ch in "Run test workflow".chars() {
            app.handle_key(KeyCode::Char(ch));
        }
        // Submit
        app.handle_key(KeyCode::Enter);
        assert!(app.modal.is_none());
        assert_eq!(app.all_commands.len(), 1);
        assert_eq!(app.current_command().unwrap().name, "test_flow");
        assert_eq!(app.current_command().unwrap().display_category(), "testing");

        // Append a step via 'a'
        app.handle_key(KeyCode::Char('a'));
        for ch in "Call memorable MCP".chars() {
            app.handle_key(KeyCode::Char(ch));
        }
        app.handle_key(KeyCode::Enter);
        assert!(app
            .current_command()
            .unwrap()
            .body
            .contains("Call memorable MCP"));

        // Toggle view mode
        assert_eq!(app.view_mode, ViewMode::Preview);
        app.handle_key(KeyCode::Char('v'));
        assert_eq!(app.view_mode, ViewMode::Raw);

        // Delete command via 'd' then 'y'
        app.handle_key(KeyCode::Char('d'));
        assert!(matches!(app.modal, Some(Modal::ConfirmDelete { .. })));
        app.handle_key(KeyCode::Char('y'));
        assert_eq!(app.all_commands.len(), 0);
    }
}
