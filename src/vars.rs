use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_yaml::Value;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Vars {
    pub map: BTreeMap<String, String>,
    pub source_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterpolateResult {
    pub rendered: String,
    pub missing: Vec<String>,
    pub replaced_count: usize,
}

impl Vars {
    pub fn default_path() -> Option<PathBuf> {
        std::env::var("HOME")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(|h| PathBuf::from(h).join(".config").join("octo-com").join("vars.yml"))
    }

    pub fn load() -> Self {
        Self::default_path()
            .map(|p| Self::load_from(&p))
            .unwrap_or_default()
    }

    pub fn load_from(path: &Path) -> Self {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Self {
                map: BTreeMap::new(),
                source_path: Some(path.to_path_buf()),
            },
        };

        let map = Self::parse_yaml_str(&content);
        Self {
            map,
            source_path: Some(path.to_path_buf()),
        }
    }

    pub fn parse_yaml_str(yaml_content: &str) -> BTreeMap<String, String> {
        let mut map = BTreeMap::new();
        let value: Value = match serde_yaml::from_str(yaml_content) {
            Ok(v) => v,
            Err(_) => return map,
        };

        if let Value::Mapping(mapping) = value {
            for (k, v) in mapping {
                let key_str = match k {
                    Value::String(s) => s,
                    Value::Number(n) => n.to_string(),
                    _ => continue,
                };

                let val_str = match v {
                    Value::String(s) => s,
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    Value::Null => String::new(),
                    _ => continue,
                };

                map.insert(key_str, val_str);
            }
        }
        map
    }

    pub fn ensure_template_exists() -> Result<PathBuf> {
        let path = Self::default_path()
            .ok_or_else(|| anyhow::anyhow!("Unable to determine user HOME directory"))?;
        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create directory {}", parent.display())
                })?;
            }
            let starter = "# Shared variables for Octopus commands\n# Define key-value pairs below:\n# research_project_id: 3\n# github_owner: octocat\n";
            fs::write(&path, starter)
                .with_context(|| format!("Failed to create starter vars file at {}", path.display()))?;
        }
        Ok(path)
    }

    pub fn interpolate(&self, template: &str) -> InterpolateResult {
        interpolate(template, &self.map)
    }
}

/// Interpolates occurrences of `{{\s*([a-zA-Z0-9_-]+)\s*}}` in `template` with values in `vars`.
/// Missing variables are left intact as `{{var_name}}` and added to `missing`.
pub fn interpolate(template: &str, vars: &BTreeMap<String, String>) -> InterpolateResult {
    let mut rendered = String::with_capacity(template.len());
    let mut missing = Vec::new();
    let mut replaced_count = 0;

    let mut remaining = template;

    while let Some(open_idx) = remaining.find("{{") {
        rendered.push_str(&remaining[..open_idx]);
        let after_open = &remaining[open_idx + 2..];

        if let Some(close_idx) = after_open.find("}}") {
            let inner = &after_open[..close_idx];
            let var_name = inner.trim();

            if is_valid_var_ident(var_name) {
                if let Some(val) = vars.get(var_name) {
                    rendered.push_str(val);
                    replaced_count += 1;
                } else {
                    // Variable missing: preserve {{var_name}}
                    rendered.push_str("{{");
                    rendered.push_str(var_name);
                    rendered.push_str("}}");
                    if !missing.contains(&var_name.to_string()) {
                        missing.push(var_name.to_string());
                    }
                }
            } else {
                // Not a valid variable identifier, leave original text
                rendered.push_str("{{");
                rendered.push_str(inner);
                rendered.push_str("}}");
            }

            remaining = &after_open[close_idx + 2..];
        } else {
            // No matching closing braces found
            rendered.push_str("{{");
            remaining = after_open;
            break;
        }
    }

    rendered.push_str(remaining);

    InterpolateResult {
        rendered,
        missing,
        replaced_count,
    }
}

fn is_valid_var_ident(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_yaml_str() {
        let yaml = r#"
research_project_id: 3
project_name: "Research"
enabled: true
empty_val: null
"#;
        let map = Vars::parse_yaml_str(yaml);
        assert_eq!(map.get("research_project_id").unwrap(), "3");
        assert_eq!(map.get("project_name").unwrap(), "Research");
        assert_eq!(map.get("enabled").unwrap(), "true");
        assert_eq!(map.get("empty_val").unwrap(), "");
    }

    #[test]
    fn test_interpolate_replacements() {
        let mut vars = BTreeMap::new();
        vars.insert("research_project_id".to_string(), "3".to_string());
        vars.insert("user".to_string(), "Alice".to_string());

        let tpl = "Project: {{research_project_id}} for {{ user }} with missing {{ unassigned_var }}.";
        let res = interpolate(tpl, &vars);

        assert_eq!(
            res.rendered,
            "Project: 3 for Alice with missing {{unassigned_var}}."
        );
        assert_eq!(res.replaced_count, 2);
        assert_eq!(res.missing, vec!["unassigned_var".to_string()]);
    }

    #[test]
    fn test_interpolate_no_match() {
        let vars = BTreeMap::new();
        let tpl = "Plain text without any braces {{}} {{ not valid! }}";
        let res = interpolate(tpl, &vars);
        assert_eq!(res.rendered, tpl);
        assert_eq!(res.missing.len(), 0);
        assert_eq!(res.replaced_count, 0);
    }
}
