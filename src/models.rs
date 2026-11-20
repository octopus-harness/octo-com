use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandDoc {
    pub name: String,
    pub filename: String,
    pub path: PathBuf,
    pub usage: Option<String>,
    pub args: Option<String>,
    pub description: Option<String>,
    pub has_valid_header: bool,
    pub body: String,
    pub raw_content: String,
    pub updated_at: String,
    pub size_bytes: u64,
}

impl CommandDoc {
    pub fn from_raw(
        name: String,
        filename: String,
        path: PathBuf,
        raw_content: String,
        updated_at: String,
        size_bytes: u64,
    ) -> Self {
        let parsed = parse_command_markdown(&raw_content);
        Self {
            name,
            filename,
            path,
            usage: parsed.usage,
            args: parsed.args,
            description: parsed.description,
            has_valid_header: parsed.has_valid_header,
            body: parsed.body,
            raw_content,
            updated_at,
            size_bytes,
        }
    }

    pub fn display_usage(&self) -> String {
        if let Some(u) = &self.usage {
            if !u.trim().is_empty() {
                return u.trim().to_string();
            }
        }
        match self.display_args() {
            Some(a) => format!("$${} {}", self.name, a),
            None => format!("$${}", self.name),
        }
    }

    pub fn display_args(&self) -> Option<&str> {
        let a = self.args.as_deref()?.trim();
        if a.is_empty() || a == "-" {
            None
        } else {
            Some(a)
        }
    }

    pub fn display_description(&self) -> &str {
        self.description
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("(no description)")
    }

    pub fn step_count(&self) -> usize {
        count_numbered_steps(&self.body)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCommand {
    pub usage: Option<String>,
    pub args: Option<String>,
    pub description: Option<String>,
    pub has_valid_header: bool,
    pub body: String,
}

pub fn parse_command_markdown(raw: &str) -> ParsedCommand {
    let normalized = raw.replace("\r\n", "\n");
    let mut lines = normalized.lines();

    let l1 = lines.next();
    let l2 = lines.next();
    let l3 = lines.next();

    if let (Some(line1), Some(line2), Some(line3)) = (l1, l2, l3) {
        if let (Some(u), Some(a), Some(d)) = (
            strip_case_insensitive_prefix(line1, "Usage:"),
            strip_case_insensitive_prefix(line2, "Args:"),
            strip_case_insensitive_prefix(line3, "Description:"),
        ) {
            let remaining: Vec<&str> = lines.collect();
            let mut body = remaining.join("\n");
            // Trim single leading empty line after the 3-line header
            if body.starts_with('\n') {
                body.remove(0);
            } else if remaining
                .first()
                .map(|s| s.trim().is_empty())
                .unwrap_or(false)
            {
                body = remaining[1..].join("\n");
            }
            return ParsedCommand {
                usage: Some(u.trim().to_string()),
                args: Some(a.trim().to_string()),
                description: Some(d.trim().to_string()),
                has_valid_header: true,
                body,
            };
        }
    }

    ParsedCommand {
        usage: None,
        args: None,
        description: None,
        has_valid_header: false,
        body: normalized,
    }
}

fn strip_case_insensitive_prefix<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let trimmed = s.trim_start();
    if trimmed.len() >= prefix.len() && trimmed[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&trimmed[prefix.len()..])
    } else {
        None
    }
}

pub fn format_usage_line(name: &str, args: &str) -> String {
    let clean_args = args.trim();
    if clean_args.is_empty() || clean_args == "-" {
        format!("$${}", name)
    } else {
        format!("$${} {}", name, clean_args)
    }
}

pub fn scaffold_command_markdown(
    name: &str,
    args: Option<&str>,
    description: Option<&str>,
    body: Option<&str>,
) -> String {
    let clean_args = args.map(str::trim).filter(|s| !s.is_empty()).unwrap_or("-");
    let usage = format_usage_line(name, clean_args);
    let clean_desc = description
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("TODO: describe $${}", name));

    let mut out = format!(
        "Usage: {}\nArgs: {}\nDescription: {}\n",
        usage, clean_args, clean_desc
    );

    if let Some(b) = body {
        let trimmed_b = b.trim_end();
        if !trimmed_b.is_empty() {
            out.push('\n');
            out.push_str(trimmed_b);
            out.push('\n');
        }
    }

    out
}

pub fn serialize_command_markdown(
    usage: &str,
    args: &str,
    description: &str,
    body: &str,
) -> String {
    let clean_usage = usage.trim();
    let clean_args = if args.trim().is_empty() {
        "-"
    } else {
        args.trim()
    };
    let clean_desc = description.trim();

    let mut out = format!(
        "Usage: {}\nArgs: {}\nDescription: {}\n",
        clean_usage, clean_args, clean_desc
    );

    if !body.trim().is_empty() {
        out.push('\n');
        out.push_str(body.trim_start_matches('\n').trim_end());
        out.push('\n');
    }

    out
}

pub fn count_numbered_steps(body: &str) -> usize {
    let mut in_code_block = false;
    let mut count = 0;
    for line in body.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code_block = !in_code_block;
            continue;
        }
        if !in_code_block && parse_numbered_step_prefix(trimmed).is_some() {
            count += 1;
        }
    }
    count
}

pub fn highest_numbered_step(body: &str) -> usize {
    let mut in_code_block = false;
    let mut max_step = 0;
    for line in body.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code_block = !in_code_block;
            continue;
        }
        if !in_code_block {
            if let Some((num, _)) = parse_numbered_step_prefix(trimmed) {
                if num > max_step {
                    max_step = num;
                }
            }
        }
    }
    max_step
}

pub fn parse_numbered_step_prefix(trimmed: &str) -> Option<(usize, &str)> {
    let dot_idx = trimmed.find(". ")?;
    let num_part = &trimmed[..dot_idx];
    if num_part.is_empty() || !num_part.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let num: usize = num_part.parse().ok()?;
    let rest = &trimmed[dot_idx + 2..];
    Some((num, rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_header() {
        let raw = "Usage: $$content_prompt\nArgs: -\nDescription: Pull todos and draft prompt\n\nTool use is allowed.\n\n1. Step one\n2. Step two\n";
        let parsed = parse_command_markdown(raw);
        assert!(parsed.has_valid_header);
        assert_eq!(parsed.usage.as_deref(), Some("$$content_prompt"));
        assert_eq!(parsed.args.as_deref(), Some("-"));
        assert_eq!(
            parsed.description.as_deref(),
            Some("Pull todos and draft prompt")
        );
        assert_eq!(
            parsed.body,
            "Tool use is allowed.\n\n1. Step one\n2. Step two"
        );
        assert_eq!(count_numbered_steps(&parsed.body), 2);
        assert_eq!(highest_numbered_step(&parsed.body), 2);
    }

    #[test]
    fn test_parse_missing_header() {
        let raw = "# Just some markdown\n\n1. Do something\n";
        let parsed = parse_command_markdown(raw);
        assert!(!parsed.has_valid_header);
        assert!(parsed.usage.is_none());
        assert_eq!(parsed.body, raw);
    }

    #[test]
    fn test_scaffold_command_markdown() {
        let s = scaffold_command_markdown("hello", None, None, None);
        assert_eq!(
            s,
            "Usage: $$hello\nArgs: -\nDescription: TODO: describe $$hello\n"
        );

        let s2 = scaffold_command_markdown(
            "summarize",
            Some("[text]"),
            Some("Summarize text"),
            Some("Tool use is allowed for this command.\n\n1. Summarize input."),
        );
        assert_eq!(
            s2,
            "Usage: $$summarize [text]\nArgs: [text]\nDescription: Summarize text\n\nTool use is allowed for this command.\n\n1. Summarize input.\n"
        );
    }
}
