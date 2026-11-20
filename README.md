# octo-com

<p align="center">
  <strong>A fast Rust CLI and keyboard-driven Ratatui TUI studio for managing <a href="https://github.com/octopus-harness/octopus">Octopus</a> <code>$$</code> Markdown command definitions.</strong>
</p>

<p align="center">
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/Rust-2021_Edition-dea584?style=flat-square&logo=rust&logoColor=white" alt="Rust 2021"></a>
  <a href="https://ratatui.rs"><img src="https://img.shields.io/badge/Ratatui-0.28-111111?style=flat-square" alt="Ratatui"></a>
  <a href="https://github.com/crossterm-rs/crossterm"><img src="https://img.shields.io/badge/Crossterm-0.28-blue?style=flat-square" alt="Crossterm"></a>
  <img src="https://img.shields.io/badge/Protocol-Octopus_%24%24_Commands-d946ef?style=flat-square" alt="Octopus $$ Commands">
  <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/License-MIT-blue?style=flat-square" alt="License: MIT"></a>
</p>

---

`octo-com` extracts and upgrades Octopus's [`bin/com`](https://github.com/octopus-harness/octopus/blob/main/bin/com) utility into a dual-mode Rust CLI and full-screen Ratatui TUI studio (`com` and `octo-com`).

It gives AI agents zero-latency CLI access (`com get <name>`, `com add <name>`, `com list`) while giving humans an interactive 2-pane terminal studio for browsing, creating, editing, and previewing `./commands/<name>.md` runbooks.

## Features

- **Drop-In `com` CLI Compatibility + Extended Subcommands**:
  - Installs both `com` and `octo-com` binaries.
  - `com get <name>`: Prints `./commands/<name>.md` to stdout (strips optional leading `$$` or `.md` suffix; case-insensitive lookup).
  - `com add <name> [--args <args>] [--desc <desc>] [--blank]`: Creates `./commands/<name>.md` and automatically scaffolds the 3-line Octopus command header (`Usage:`, `Args:`, `Description:`) unless `--blank` is passed.
  - `com list`: Displays an aligned summary table of all commands (`COMMAND`, `ARGS`, `DESCRIPTION`).
  - `com edit <name>`: Opens `./commands/<name>.md` directly in `$EDITOR` / `$VISUAL`.
  - `com rm <name>`: Deletes `./commands/<name>.md`.
  - `com` / `com tui`: Launches the full-screen Ratatui studio.
- **Directory Resolution Chain**:
  1. `--dir <path>` / `-d <path>` CLI flag
  2. `OCTO_COMMANDS_DIR` environment variable
  3. `./commands` in the current working directory (automatically created if missing)
- **2-Pane Master–Detail TUI Studio (Octopus Magenta/Purple Theme)**:
  - **Header Bar**: `🐙 OCTO-COM` badge, active `[Dir: <path>]`, live command/header/step/line statistics, active view mode (`Preview (GFM)` or `Raw`), and search filter indicator.
  - **Pane 1 `[1] Commands`**: Searchable list of `.md` commands showing `$$<name>`, `Args` badge, step counter (`[N steps]`) or `[!header]` warning badge, and one-line description.
  - **Pane 2 `[2] Command Detail [Preview (GFM) | Raw]`**:
    - **3-Line Metadata Card**: Displays `$$<name>` (`<file>.md`), 3-line header validation badge, step/word/line counts, file size, last-modified timestamp, `Usage`, `Args`, and `Description`.
    - **Dual Markdown Views (`v` / `m`)**: Toggle between **Styled GFM Preview** (highlighted numbered runbook steps, inline code/MCP badges, tool-use banners, headings, fenced code blocks, tables) and **Raw Line-Numbered Markdown**.
- **Full In-App CRUD & External `$EDITOR`**:
  - `n`: **New Command Modal** — enter Name, Args, Description, and optional `Tool use is allowed for this command.` starter.
  - `e`: **Edit 3-Line Header / Rename Modal** — update `Usage:`, `Args:`, `Description:`, or rename the `.md` file in place while preserving the Markdown body.
  - `a`: **Quick Append Step Modal** — append the next auto-numbered execution step (`N. ...`) or a Markdown paragraph without leaving the TUI.
  - `E`: **External `$EDITOR`** — cleanly suspends the TUI, opens the `.md` file in `$EDITOR` (`nvim`, `vim`, `nano`), and reloads on exit.
  - `d`: **Delete Confirmation Modal**.
  - `y` / `c`: Copy `$$<name>` invocation (`y`) or full Markdown file (`c` / `Y`) to the system clipboard.

## Installation & Usage

```bash
# Build release binaries (target/release/com and target/release/octo-com)
cargo build --release

# Install to ~/.cargo/bin
cargo install --path .
```

### CLI Examples

```bash
# Launch the interactive Ratatui TUI studio
com

# Point to an Octopus repository's commands directory
com --dir ~/src/octopus/commands
export OCTO_COMMANDS_DIR="$HOME/src/octopus/commands"

# List all commands with Args and Description
com list

# Create a command with the 3-line Octopus header scaffolded automatically
com add deep_spec --args "[feature]" --desc "Synthesize Doer, Noteking, Memorable, and Devbook into a spec"

# Create a blank .md file (legacy touch behavior)
com add scratch --blank

# Print a command definition ($$ prefix and .md suffix are optional)
com get content_prompt
com get '$$content_prompt'

# Open in $EDITOR or delete
com edit deep_spec
com rm scratch
```

## Octopus 3-Line Header Specification

Every command file in `./commands/<name>.md` starts with a three-line header followed by its execution instructions:

```markdown
Usage: $$<name> [args]
Args: <args or "-">
Description: <one-line summary>

Tool use is allowed for this command.

1. Step one...
2. Step two...
```

## TUI Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `1`, `2` | Focus Pane 1 (`Commands`) or Pane 2 (`Command Detail`) |
| `Tab` / `BackTab` / `h` / `l` | Switch focus between `Commands` list and `Command Detail` pane |
| `j` / `k` / `↑` / `↓` | Move selection down / up in `Commands` list, or scroll `Detail` pane |
| `J` / `K` / `PgDn` / `PgUp` | Fast-scroll `Command Detail` pane down / up |
| `g` / `G` | Jump to top / bottom of active pane |
| `v` / `m` | Toggle between **Preview (GFM)** and **Raw** (line-numbered) views |
| `n` | Open **New Command** modal (scaffolds 3-line header) |
| `e` | Open **Edit Header / Rename** modal |
| `a` | Open **Quick Append** modal (auto-increments numbered step `N.`) |
| `E` | Open selected `.md` file in external `$EDITOR` |
| `d` | Delete selected command with confirmation |
| `/` | Live search across command names, args, descriptions, and bodies (`Esc` to clear) |
| `y` | Yank `$$<name>` invocation to system clipboard |
| `c` / `Y` | Copy full command Markdown to system clipboard |
| `r` | Reload commands from disk |
| `?` | Toggle Help modal |
| `q` / `Esc` | Clear search filter or quit `octo-com` |
