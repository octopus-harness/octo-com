use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::format::format_system_time;
use crate::models::{
    format_usage_line, highest_numbered_step, scaffold_command_markdown,
    serialize_command_markdown, CommandDoc,
};

/// Validates a raw command name (stripping optional leading `$$` and `.md` suffix)
/// and returns `(stem, filename)`.
pub fn validate_and_normalize_name(raw: &str) -> Result<(String, String)> {
    let trimmed = raw.trim();
    let without_prefix = trimmed.strip_prefix("$$").unwrap_or(trimmed);

    if without_prefix.is_empty() {
        bail!("Command name cannot be empty.");
    }
    if without_prefix == "." || without_prefix == ".." {
        bail!("Invalid command name: '{}'.", without_prefix);
    }
    if without_prefix.contains('/')
        || without_prefix.contains('\\')
        || without_prefix.contains("..")
    {
        bail!("Command name cannot contain '/' or '..'.");
    }

    let stem = without_prefix
        .strip_suffix(".md")
        .unwrap_or(without_prefix)
        .to_string();

    if stem.is_empty() || stem == "." || stem == ".." {
        bail!("Invalid command name: '{}'.", without_prefix);
    }

    let filename = format!("{}.md", stem);
    Ok((stem, filename))
}

#[derive(Debug, Clone)]
pub struct CommandStore {
    pub dir: PathBuf,
}

impl CommandStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn ensure_dir(&self) -> Result<()> {
        if !self.dir.exists() {
            fs::create_dir_all(&self.dir).with_context(|| {
                format!(
                    "Failed to create commands directory at {}",
                    self.dir.display()
                )
            })?;
        }
        Ok(())
    }

    pub fn list_commands(&self) -> Result<Vec<CommandDoc>> {
        self.ensure_dir()?;
        let mut docs = Vec::new();

        let entries = fs::read_dir(&self.dir)
            .with_context(|| format!("Failed to read directory {}", self.dir.display()))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let filename = match path.file_name().and_then(|n| n.to_str()) {
                Some(f) if f.ends_with(".md") && !f.starts_with('.') => f.to_string(),
                _ => continue,
            };
            if let Ok(doc) = self.load_doc_from_path(&path, &filename) {
                docs.push(doc);
            }
        }

        docs.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.name.cmp(&b.name))
        });

        Ok(docs)
    }

    pub fn get_command(&self, raw_name: &str) -> Result<CommandDoc> {
        self.ensure_dir()?;
        let (stem, filename) = validate_and_normalize_name(raw_name)?;
        let target = self.dir.join(&filename);

        if target.is_file() {
            return self.load_doc_from_path(&target, &filename);
        }

        // Case-insensitive fallback lookup per COMMANDS.md ("Command names are case-insensitive")
        if let Ok(entries) = fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                if let Some(fname) = path.file_name().and_then(|s| s.to_str()) {
                    if fname.eq_ignore_ascii_case(&filename) {
                        return self.load_doc_from_path(&path, fname);
                    }
                }
            }
        }

        let _ = stem;
        bail!(
            "Command file '{}' does not exist in {}.",
            filename,
            self.dir.display()
        )
    }

    pub fn create_command(
        &self,
        raw_name: &str,
        category: Option<&str>,
        args: Option<&str>,
        description: Option<&str>,
        body: Option<&str>,
        blank: bool,
    ) -> Result<CommandDoc> {
        self.ensure_dir()?;
        let (stem, filename) = validate_and_normalize_name(raw_name)?;
        let target = self.dir.join(&filename);

        if target.exists() {
            bail!(
                "Command file '{}' already exists at {}.",
                filename,
                target.display()
            );
        }

        let content = if blank {
            String::new()
        } else {
            scaffold_command_markdown(&stem, category, args, description, body)
        };

        fs::write(&target, &content)
            .with_context(|| format!("Failed to write {}", target.display()))?;

        self.load_doc_from_path(&target, &filename)
    }

    pub fn update_command_header(
        &self,
        old_name: &str,
        new_name: &str,
        category: &str,
        args: &str,
        description: &str,
    ) -> Result<CommandDoc> {
        self.ensure_dir()?;
        let existing = self.get_command(old_name)?;
        let (new_stem, new_filename) = validate_and_normalize_name(new_name)?;
        let new_target = self.dir.join(&new_filename);

        if new_filename != existing.filename && new_target.exists() {
            bail!(
                "Command file '{}' already exists at {}.",
                new_filename,
                new_target.display()
            );
        }

        let clean_args = if args.trim().is_empty() {
            "-"
        } else {
            args.trim()
        };
        let usage = format_usage_line(&new_stem, clean_args);
        let new_raw = serialize_command_markdown(&usage, clean_args, description, category, &existing.body);

        fs::write(&new_target, &new_raw)
            .with_context(|| format!("Failed to write {}", new_target.display()))?;

        if new_target != existing.path && existing.path.exists() {
            fs::remove_file(&existing.path).with_context(|| {
                format!("Failed to remove old file {}", existing.path.display())
            })?;
        }

        self.load_doc_from_path(&new_target, &new_filename)
    }

    pub fn append_to_command(
        &self,
        raw_name: &str,
        text: &str,
        as_numbered_step: bool,
    ) -> Result<CommandDoc> {
        let existing = self.get_command(raw_name)?;
        let trimmed_input = text.trim();
        if trimmed_input.is_empty() {
            bail!("Content to append cannot be empty.");
        }

        let addition = if as_numbered_step {
            let next_num = highest_numbered_step(&existing.body) + 1;
            format!("{}. {}", next_num, trimmed_input)
        } else {
            trimmed_input.to_string()
        };

        let mut new_raw = existing.raw_content.clone();
        if !new_raw.is_empty() && !new_raw.ends_with('\n') {
            new_raw.push('\n');
        }

        if existing.body.trim().is_empty() && existing.has_valid_header {
            new_raw.push('\n');
            new_raw.push_str(&addition);
            new_raw.push('\n');
        } else if as_numbered_step {
            new_raw.push_str(&addition);
            new_raw.push('\n');
        } else {
            if !new_raw.is_empty() {
                new_raw.push('\n');
            }
            new_raw.push_str(&addition);
            new_raw.push('\n');
        }

        fs::write(&existing.path, &new_raw)
            .with_context(|| format!("Failed to write {}", existing.path.display()))?;

        self.load_doc_from_path(&existing.path, &existing.filename)
    }

    pub fn delete_command(&self, raw_name: &str) -> Result<PathBuf> {
        let existing = self.get_command(raw_name)?;
        fs::remove_file(&existing.path)
            .with_context(|| format!("Failed to delete {}", existing.path.display()))?;
        Ok(existing.path)
    }

    fn load_doc_from_path(&self, path: &Path, filename: &str) -> Result<CommandDoc> {
        let raw_content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let meta =
            fs::metadata(path).with_context(|| format!("Failed to stat {}", path.display()))?;
        let updated_at = meta
            .modified()
            .map(format_system_time)
            .unwrap_or_else(|_| "unknown".to_string());
        let size_bytes = meta.len();
        let name = filename.strip_suffix(".md").unwrap_or(filename).to_string();

        Ok(CommandDoc::from_raw(
            name,
            filename.to_string(),
            path.to_path_buf(),
            raw_content,
            updated_at,
            size_bytes,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_validate_and_normalize_name() {
        assert_eq!(
            validate_and_normalize_name("hello").unwrap(),
            ("hello".to_string(), "hello.md".to_string())
        );
        assert_eq!(
            validate_and_normalize_name("$$hello.md").unwrap(),
            ("hello".to_string(), "hello.md".to_string())
        );
        assert!(validate_and_normalize_name("").is_err());
        assert!(validate_and_normalize_name("$$").is_err());
        assert!(validate_and_normalize_name(".").is_err());
        assert!(validate_and_normalize_name("..").is_err());
        assert!(validate_and_normalize_name("../secret").is_err());
        assert!(validate_and_normalize_name("sub/cmd").is_err());
    }

    #[test]
    fn test_store_crud_and_append() {
        let tmp = TempDir::new().unwrap();
        let store = CommandStore::new(tmp.path().join("commands"));

        let created = store
            .create_command("my_cmd", Some("web"), Some("[url]"), Some("Fetch URL"), None, false)
            .unwrap();
        assert_eq!(created.name, "my_cmd");
        assert!(created.has_valid_header);
        assert_eq!(created.usage.as_deref(), Some("$$my_cmd [url]"));
        assert_eq!(created.args.as_deref(), Some("[url]"));
        assert_eq!(created.description.as_deref(), Some("Fetch URL"));
        assert_eq!(created.display_category(), "web");

        // Duplicate should fail
        assert!(store
            .create_command("my_cmd", None, None, None, None, false)
            .is_err());

        // Append step 1 and step 2
        let updated = store
            .append_to_command("my_cmd", "Fetch the webpage", true)
            .unwrap();
        assert_eq!(updated.step_count(), 1);
        let updated2 = store
            .append_to_command("$$my_cmd", "Summarize markdown", true)
            .unwrap();
        assert_eq!(updated2.step_count(), 2);
        assert!(updated2.body.contains("1. Fetch the webpage"));
        assert!(updated2.body.contains("2. Summarize markdown"));

        // Rename and update header
        let renamed = store
            .update_command_header("my_cmd", "fetch_url", "network", "<url>", "Updated description")
            .unwrap();
        assert_eq!(renamed.name, "fetch_url");
        assert_eq!(renamed.usage.as_deref(), Some("$$fetch_url <url>"));
        assert_eq!(renamed.display_category(), "network");
        assert_eq!(renamed.step_count(), 2);
        assert!(store.get_command("my_cmd").is_err());

        // Delete
        store.delete_command("fetch_url").unwrap();
        assert!(store.list_commands().unwrap().is_empty());
    }
}
