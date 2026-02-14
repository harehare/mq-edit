<h1 align="center"><code>mq-edit</code></h1>

A terminal-based Markdown and code editor with WYSIWYG rendering and LSP support.

![demo](assets/demo.gif)

## Overview

`mq-edit` is a Rust-based TUI (Text User Interface) editor that provides:
- **Markdown WYSIWYG**: The line under the cursor displays source, other lines show rich formatted text
- **LSP Integration**: Full Language Server Protocol support for code intelligence
- **Multi-language support**: Rust, Python, MQ (Markdown Query Language), and more

Built on top of the `mq-markdown` parser with full LSP capabilities.

## Installation

```bash
# Latest Development Version
cargo install --git https://github.com/harehare/mq-edit.git
```

## Usage

```bash
# Open a Markdown file
mq-edit README.md

# Open a Rust file (with LSP support)
mq-edit src/main.rs

# Open an MQ file (Markdown Query Language)
mq-edit query.mq

# Create a new file (empty buffer)
mq-edit

# Initialize default configuration file
mq-edit --init-config

# Show help
mq-edit --help
```

### Keyboard Shortcuts

| Key              | Action                                      |
| ---------------- | ------------------------------------------- |
| `Ctrl+S`         | Save file (opens save dialog for new files) |
| `Ctrl+Q` / `Esc` | Quit                                        |
| `Alt+B` / `F2`   | Toggle file browser                         |
| `Ctrl+Space`     | Code completion (LSP)                       |
| `Ctrl+D`         | Go to definition (LSP)                      |
| `Ctrl+B`         | Navigate back                               |
| `Ctrl+F`         | Navigate forward                            |
| `Ctrl+G`         | Go to line                                  |
| `Ctrl+E`         | Execute mq query                            |
| `F3`             | Search                                      |
| `F4`             | Find and replace                            |
| `Up/Down`        | Move cursor / Select completion             |
| `Enter`          | Apply completion                            |

## Pipe Mode

`mq-edit` supports pipe mode, allowing you to read from stdin and write to stdout. This is useful for combining with [mq](https://github.com/harehare/mq) and other command-line tools. In pipe mode, pressing `Esc` exits immediately without a save confirmation dialog — the edited content is written to stdout on exit.

```bash
# Edit mq output interactively, then pass the result to the next command
cat README.md | mq '.h' | mq-edit | mq 'downcase()'

# Extract links, edit them, and save to a file
cat README.md | mq '.link' | mq-edit > links.md

# Transform markdown with mq, review and edit interactively, then write back
mq '.[] | select(.h.depth > 1)' README.md | mq-edit > headings.md

# Edit text from clipboard (macOS)
pbpaste | mq-edit | pbcopy
```

## Configuration

`mq-edit` uses a TOML configuration file for customizing keybindings and LSP servers.

### Config File Location

- **Linux**: `~/.config/mq/edit/config.toml`
- **macOS**: `~/Library/Application Support/mq/edit/config.toml`
- **Windows**: `%APPDATA%\mq/edit\config.toml`

### Initialize Config

Run `mq-edit --init-config` to create a default configuration file with all available keybindings.

### LSP Configuration

Configure language servers for different file types:

```toml
# Rust Language Server
[lsp.servers.rust]
command = "rust-analyzer"
args = []
enable_completion = true
enable_diagnostics = true
enable_goto_definition = true

# MQ Language Server (for .mq files)
[lsp.servers.mq]
command = "mq-lsp"
args = []
enable_completion = true
enable_diagnostics = true
enable_goto_definition = true

# Python Language Server
[lsp.servers.python]
command = "pyright-langserver"
args = ["--stdio"]
enable_completion = true
enable_diagnostics = true
enable_goto_definition = true
```

### Customize Keybindings

Edit the config file to change keybindings. Example:

```toml
[keybindings.quit]
code = "q"
modifiers = ["ctrl"]  # Ctrl+Q to quit

[keybindings.quit_alt]
code = "esc"
modifiers = []  # Esc to quit (alternative)

[keybindings.save]
code = "s"
modifiers = ["ctrl"]  # Ctrl+S to save
```

### Theme Configuration

Choose from built-in syntax highlighting themes:

```toml
[editor]
# Syntax highlighting theme
theme = "base16-ocean.dark"  # Default dark theme

# Other available themes:
# theme = "base16-ocean.light"     # Light ocean theme
# theme = "base16-mocha.dark"      # Mocha dark theme
# theme = "base16-eighties.dark"   # Eighties dark theme
# theme = "InspiredGitHub"         # GitHub-inspired light theme
# theme = "Solarized (dark)"       # Solarized dark
# theme = "Solarized (light)"      # Solarized light
```

All themes are provided by the [syntect](https://github.com/trishume/syntect) library.

## Keybindings

**Note**: Default keybindings have been changed to avoid conflicts with VSCode, Zellij, and other editors. All keybindings are customizable via the config file.

### Navigation

- `Arrow keys` - Move cursor
- `Home` - Start of line
- `End` - End of line
- `Page Up/Down` - Scroll page

### Editing

- `Character keys` - Insert text
- `Enter` - Insert newline
- `Backspace` - Delete character
- `Tab` - Insert tab

### File Operations

- `Ctrl+S` - Save file (opens save-as dialog for new files)
- `Ctrl+Q` or `Esc` - Quit application
- `Alt+B` - Toggle file browser (changed from `Ctrl+B` to avoid VSCode conflict)
- `F2` - Toggle file browser (alternative)

### LSP Features

- `Ctrl+D` - Go to definition (changed from `Ctrl+G` to avoid conflict with goto line)
- `Ctrl+B` - Navigate back in jump history
- `Ctrl+F` - Navigate forward in jump history
- `Ctrl+Space` - Trigger code completion
- `Ctrl+L` - Toggle line numbers
- `Ctrl+Shift+L` - Toggle current line highlight

### mq Query Execution

`mq-edit` has built-in support for [mq (Markdown Query Language)](https://github.com/harehare/mq). Press `Ctrl+E` to open the query dialog and run mq queries against the current document.

- `Ctrl+E` - Open mq query dialog
- Type a query (e.g. `.heading`, `.link`, `.[] | upcase()`)
- `Enter` - Execute query and insert result at cursor position
- `Esc` - Close dialog

The query result is inserted at the current cursor position, and the cursor remains at the insertion start so you can review the output.

### Search and Navigation

- `F3` - Open search dialog
- `F4` - Open find and replace dialog
- `Ctrl+G` - Go to line (opens line number dialog)

### File Browser (when visible)

- `Up/Down` - Navigate files and directories
- `Right` - Expand directory
- `Left` - Collapse directory
- `Enter` - Open file or toggle directory
- `Esc` - Close file browser

## License

Same license as the mq project (MIT).
