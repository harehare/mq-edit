use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionResponse, GotoDefinitionResponse,
    InsertTextFormat, Location, Position, Range,
};
use mq_markdown::Markdown;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use crate::backend::LspBackend;
use crate::client::LspEvent;

/// Simple heading detection for Markdown
fn is_heading(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix('#') {
        let mut level = 1;
        let mut chars = rest.chars();
        while let Some('#') = chars.next() {
            level += 1;
            if level > 6 {
                break;
            }
        }
        if level <= 6
            && rest
                .chars()
                .nth(level - 1)
                .is_some_and(|c| c.is_whitespace())
        {
            return Some(level);
        }
    }
    None
}

/// Document state for the embedded Markdown LSP
#[allow(dead_code)]
struct MarkdownDocument {
    content: String,
    ast: Option<Markdown>,
    version: i32,
}

/// Embedded Markdown Language Server
pub struct MarkdownLsp {
    documents: HashMap<String, MarkdownDocument>,
    #[allow(dead_code)]
    root_path: PathBuf,
    event_tx: mpsc::Sender<LspEvent>,
}

impl MarkdownLsp {
    /// Create a new Markdown LSP instance
    pub fn new(root_path: PathBuf, event_tx: mpsc::Sender<LspEvent>) -> Self {
        Self {
            documents: HashMap::new(),
            root_path,
            event_tx,
        }
    }

    /// Convert a file path to a URI string
    fn path_to_uri(path: &Path) -> String {
        format!("file://{}", path.display())
    }

    /// Parse a markdown document and store it
    fn parse_document(&mut self, uri: &str, content: &str, version: i32) {
        let ast = Markdown::from_markdown_str(content).ok();
        self.documents.insert(
            uri.to_string(),
            MarkdownDocument {
                content: content.to_string(),
                ast,
                version,
            },
        );
    }

    /// Get completion items at position
    fn get_completions(&self, uri: &str, line: u32, character: u32) -> Vec<CompletionItem> {
        let Some(doc) = self.documents.get(uri) else {
            return vec![];
        };

        let lines: Vec<&str> = doc.content.lines().collect();
        let Some(line_content) = lines.get(line as usize) else {
            return vec![];
        };

        let char_pos = (character as usize).min(line_content.len());
        let prefix = &line_content[..char_pos];

        let mut items = vec![];

        // Heading completions after #
        if prefix.ends_with('#') || prefix.ends_with("# ") {
            for level in 1..=6 {
                items.push(CompletionItem {
                    label: format!("{} Heading {}", "#".repeat(level), level),
                    kind: Some(CompletionItemKind::SNIPPET),
                    insert_text: Some(format!("{} ", "#".repeat(level))),
                    ..Default::default()
                });
            }
        }

        // Link completions after [
        if prefix.ends_with('[') {
            items.push(CompletionItem {
                label: "[text](url)".to_string(),
                detail: Some("Link".to_string()),
                kind: Some(CompletionItemKind::SNIPPET),
                insert_text: Some("$1]($2)".to_string()),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                ..Default::default()
            });
        }

        // Image completions after ![
        if prefix.ends_with("![") {
            items.push(CompletionItem {
                label: "![alt](url)".to_string(),
                detail: Some("Image".to_string()),
                kind: Some(CompletionItemKind::SNIPPET),
                insert_text: Some("$1]($2)".to_string()),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                ..Default::default()
            });
        }

        // Task list completions after -
        if prefix.trim_start().ends_with('-') || prefix.trim_start().ends_with("- ") {
            items.push(CompletionItem {
                label: "- [ ] Task".to_string(),
                detail: Some("Task list item".to_string()),
                kind: Some(CompletionItemKind::SNIPPET),
                insert_text: Some(" [ ] ".to_string()),
                ..Default::default()
            });
            items.push(CompletionItem {
                label: "- [x] Completed task".to_string(),
                detail: Some("Completed task item".to_string()),
                kind: Some(CompletionItemKind::SNIPPET),
                insert_text: Some(" [x] ".to_string()),
                ..Default::default()
            });
        }

        // Code fence completion after `
        if prefix.ends_with("``") {
            for lang in [
                "rust",
                "python",
                "javascript",
                "typescript",
                "json",
                "yaml",
                "bash",
                "go",
                "java",
                "c",
                "cpp",
            ] {
                items.push(CompletionItem {
                    label: format!("`{}", lang),
                    detail: Some(format!("{} code block", lang)),
                    kind: Some(CompletionItemKind::SNIPPET),
                    insert_text: Some(format!("`{}\n$1\n```", lang)),
                    insert_text_format: Some(InsertTextFormat::SNIPPET),
                    ..Default::default()
                });
            }
        }

        // Heading anchor completions for links to same document
        if prefix.contains("](#") {
            for (i, l) in lines.iter().enumerate() {
                if let Some(level) = is_heading(l) {
                    let heading_text = l.trim_start_matches('#').trim();
                    let slug = Self::make_heading_slug(heading_text);
                    items.push(CompletionItem {
                        label: format!("#{}", slug),
                        detail: Some(format!("H{}: {}", level, heading_text)),
                        kind: Some(CompletionItemKind::REFERENCE),
                        insert_text: Some(slug),
                        sort_text: Some(format!("{:04}", i)),
                        ..Default::default()
                    });
                }
            }
        }

        items
    }

    /// Create a URL-safe heading slug
    fn make_heading_slug(text: &str) -> String {
        text.to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-")
    }

    /// Get definition location for links
    fn get_definition(&self, uri: &str, line: u32, character: u32) -> Option<GotoDefinitionResponse> {
        let doc = self.documents.get(uri)?;
        let lines: Vec<&str> = doc.content.lines().collect();
        let line_content = lines.get(line as usize)?;

        let char_pos = character as usize;

        // Find link at cursor position: [text](target)
        if let Some(link_start) = line_content[..char_pos.min(line_content.len())].rfind('[') {
            let rest = &line_content[link_start..];
            if let Some(paren_start) = rest.find("](")
                && let Some(paren_end) = rest[paren_start..].find(')')
            {
                let link_target = &rest[paren_start + 2..paren_start + paren_end];

                // Handle heading links (#heading-slug)
                if let Some(anchor) = link_target.strip_prefix('#') {
                    // Find matching heading
                    for (i, l) in lines.iter().enumerate() {
                        if is_heading(l).is_some() {
                            let heading_text = l.trim_start_matches('#').trim();
                            let heading_slug = Self::make_heading_slug(heading_text);
                            if heading_slug == anchor {
                                return Some(GotoDefinitionResponse::Scalar(Location {
                                    uri: uri.parse().ok()?,
                                    range: Range {
                                        start: Position {
                                            line: i as u32,
                                            character: 0,
                                        },
                                        end: Position {
                                            line: i as u32,
                                            character: l.len() as u32,
                                        },
                                    },
                                }));
                            }
                        }
                    }
                }

                // Handle relative file links
                if !link_target.starts_with("http://") && !link_target.starts_with("https://") {
                    // Extract file path (remove anchor if present)
                    let file_part = link_target.split('#').next().unwrap_or(link_target);
                    if !file_part.is_empty() {
                        let uri_path = uri.strip_prefix("file://").unwrap_or(uri);
                        let base_path = Path::new(uri_path);
                        let parent = base_path.parent().unwrap_or(Path::new("."));
                        let target_path = parent.join(file_part);

                        if target_path.exists() {
                            let target_uri = format!("file://{}", target_path.display());
                            return Some(GotoDefinitionResponse::Scalar(Location {
                                uri: target_uri.parse().ok()?,
                                range: Range::default(),
                            }));
                        }
                    }
                }
            }
        }

        None
    }
}

impl LspBackend for MarkdownLsp {
    fn initialize(&mut self) -> miette::Result<()> {
        // Send initialized event with trigger characters
        let _ = self.event_tx.send(LspEvent::Initialized(vec![
            "#".to_string(),
            "[".to_string(),
            "!".to_string(),
            "`".to_string(),
            "-".to_string(),
        ]));
        Ok(())
    }

    fn initialized(&mut self) -> miette::Result<()> {
        Ok(())
    }

    fn did_open(&mut self, file_path: &Path, content: &str) -> miette::Result<()> {
        let uri = Self::path_to_uri(file_path);
        self.parse_document(&uri, content, 1);
        Ok(())
    }

    fn did_change(&mut self, file_path: &Path, version: i32, content: &str) -> miette::Result<()> {
        let uri = Self::path_to_uri(file_path);
        self.parse_document(&uri, content, version);
        Ok(())
    }

    fn request_semantic_tokens(&mut self, _file_path: &Path) -> miette::Result<()> {
        // Markdown LSP doesn't provide semantic tokens currently
        Ok(())
    }

    fn request_completion(
        &mut self,
        file_path: &Path,
        line: u32,
        character: u32,
        _trigger_character: Option<String>,
    ) -> miette::Result<()> {
        let uri = Self::path_to_uri(file_path);
        let items = self.get_completions(&uri, line, character);
        let _ = self
            .event_tx
            .send(LspEvent::Completion(CompletionResponse::Array(items)));
        Ok(())
    }

    fn request_definition(
        &mut self,
        file_path: &Path,
        line: u32,
        character: u32,
    ) -> miette::Result<()> {
        let uri = Self::path_to_uri(file_path);
        if let Some(response) = self.get_definition(&uri, line, character) {
            let _ = self.event_tx.send(LspEvent::Definition(response));
        }
        Ok(())
    }

    fn request_references(
        &mut self,
        _file_path: &Path,
        _line: u32,
        _character: u32,
        _include_declaration: bool,
    ) -> miette::Result<()> {
        // Not implemented yet
        Ok(())
    }

    fn shutdown(&mut self) -> miette::Result<()> {
        self.documents.clear();
        Ok(())
    }

    fn language_id(&self) -> &str {
        "markdown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_lsp() -> (MarkdownLsp, mpsc::Receiver<LspEvent>) {
        let (tx, rx) = mpsc::channel();
        let lsp = MarkdownLsp::new(PathBuf::from("/tmp"), tx);
        (lsp, rx)
    }

    #[test]
    fn test_heading_slug() {
        assert_eq!(MarkdownLsp::make_heading_slug("Hello World"), "hello-world");
        assert_eq!(
            MarkdownLsp::make_heading_slug("Getting Started"),
            "getting-started"
        );
        assert_eq!(
            MarkdownLsp::make_heading_slug("Test - Special & Chars!"),
            "test-special-chars"
        );
    }

    #[test]
    fn test_parse_document() {
        let (mut lsp, _rx) = create_test_lsp();
        let content = "# Hello\n\nWorld";
        lsp.parse_document("file:///test.md", content, 1);

        assert!(lsp.documents.contains_key("file:///test.md"));
        let doc = &lsp.documents["file:///test.md"];
        assert_eq!(doc.content, content);
        assert_eq!(doc.version, 1);
    }

    #[test]
    fn test_heading_completions() {
        let (mut lsp, _rx) = create_test_lsp();
        lsp.parse_document("file:///test.md", "#", 1);

        let completions = lsp.get_completions("file:///test.md", 0, 1);
        assert!(!completions.is_empty());
        assert!(completions.iter().any(|c| c.label.contains("Heading 1")));
    }

    #[test]
    fn test_link_completions() {
        let (mut lsp, _rx) = create_test_lsp();
        lsp.parse_document("file:///test.md", "[", 1);

        let completions = lsp.get_completions("file:///test.md", 0, 1);
        assert!(completions.iter().any(|c| c.label.contains("text](url)")));
    }

    #[test]
    fn test_anchor_completions() {
        let (mut lsp, _rx) = create_test_lsp();
        let content = "# First Heading\n\n## Second Heading\n\n[link](#";
        lsp.parse_document("file:///test.md", content, 1);

        let completions = lsp.get_completions("file:///test.md", 4, 8);
        assert!(completions
            .iter()
            .any(|c| c.label.contains("first-heading")));
        assert!(completions
            .iter()
            .any(|c| c.label.contains("second-heading")));
    }

    #[test]
    fn test_definition_heading_anchor() {
        let (mut lsp, _rx) = create_test_lsp();
        let content = "# First Heading\n\n[link](#first-heading)";
        lsp.parse_document("file:///test.md", content, 1);

        let definition = lsp.get_definition("file:///test.md", 2, 10);
        assert!(definition.is_some());

        if let Some(GotoDefinitionResponse::Scalar(loc)) = definition {
            assert_eq!(loc.range.start.line, 0);
        }
    }

    #[test]
    fn test_is_heading() {
        assert_eq!(is_heading("# Heading 1"), Some(1));
        assert_eq!(is_heading("## Heading 2"), Some(2));
        assert_eq!(is_heading("### Heading 3"), Some(3));
        assert_eq!(is_heading("Not a heading"), None);
        assert_eq!(is_heading("#NoSpace"), None);
    }
}
