# markdown-lsp

A Language Server Protocol (LSP) implementation for Markdown files, written in Rust.

Part of the [mq-edit](https://github.com/harehare/mq-edit) project.

## Features

### Completion

Triggered by `#`, `[`, `!`, `` ` ``, `-`:

- **Headings** — H1–H6 syntax suggestions
- **Links** — `[text](url)` snippet
- **Images** — `![alt](url)` snippet
- **Task lists** — Checklist item templates
- **Code fences** — 50+ language identifiers for syntax highlighting
- **Anchor links** — Auto-complete `](#heading)` with available headings

### Go to Definition

- Navigate to heading definitions from anchor links
- Resolve relative file links

### Find References

- Find all references to a heading
- Show all links pointing to a heading

### Diagnostics

- Detect broken anchor links (references to non-existent headings)

## Installation

### Cargo

```bash
cargo install markdown-lsp
```

### From Source

```bash
git clone https://github.com/harehare/mq-edit.git
cd mq-edit
cargo build --release -p markdown-lsp
```

## Usage

`markdown-lsp` communicates over stdio using the LSP protocol. Configure it in your editor as a language server for Markdown files.

## License

MIT
