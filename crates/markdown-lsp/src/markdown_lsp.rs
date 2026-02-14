use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionResponse, Diagnostic, DiagnosticSeverity,
    GotoDefinitionResponse, InsertTextFormat, Location, Position, Range, Uri as LspUri,
};
use mq_markdown::{Heading, Link, Markdown, Node, Position as MdPosition};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use crate::backend::LspBackend;
use crate::client::LspEvent;

/// Document state for the embedded Markdown LSP
struct MarkdownDocument {
    content: String,
    ast: Option<Markdown>,
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
    fn parse_document(&mut self, uri: &str, content: &str) {
        let ast = Markdown::from_markdown_str(content).ok();

        // Generate diagnostics for broken links
        if let Some(ref markdown) = ast {
            self.generate_diagnostics(uri, markdown);
        }

        self.documents.insert(
            uri.to_string(),
            MarkdownDocument {
                content: content.to_string(),
                ast,
            },
        );
    }

    /// Find node at a specific position
    fn find_node_at_position<'a>(
        &self,
        ast: &'a Markdown,
        line: u32,
        character: u32,
    ) -> Option<&'a Node> {
        for node in &ast.nodes {
            if let Some(pos) = node.position() {
                // Convert to 0-indexed for comparison (LSP uses 0-indexed)
                if pos.start.line.saturating_sub(1) <= line as usize
                    && (line as usize) < pos.end.line
                    && pos.start.column.saturating_sub(1) <= character as usize
                    && (character as usize) < pos.end.column
                {
                    return Some(node);
                }
            }
        }
        None
    }

    /// Check if position is inside a code block
    fn is_inside_code_block(&self, ast: &Markdown, line: u32, _character: u32) -> bool {
        for node in &ast.nodes {
            if let Node::Code(code) = node
                && let Some(pos) = &code.position
            {
                // Check if line is within code block
                if pos.start.line.saturating_sub(1) <= line as usize
                    && (line as usize) < pos.end.line
                {
                    return true;
                }
            }
        }
        false
    }

    /// Collect all headings from the AST
    fn collect_headings<'a>(&self, ast: &'a Markdown) -> Vec<(&'a Node, &'a Heading)> {
        ast.nodes
            .iter()
            .filter_map(|node| {
                if let Node::Heading(heading) = node {
                    Some((node, heading))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Convert markdown position to LSP range
    fn position_to_lsp_range(pos: &MdPosition) -> Range {
        Range {
            start: Position {
                line: (pos.start.line - 1) as u32,
                character: (pos.start.column - 1) as u32,
            },
            end: Position {
                line: (pos.end.line - 1) as u32,
                character: (pos.end.column - 1) as u32,
            },
        }
    }

    /// Generate diagnostics for broken links
    fn generate_diagnostics(&self, uri: &str, ast: &Markdown) {
        let mut diagnostics = Vec::new();

        // Collect all heading slugs
        let headings = self.collect_headings(ast);
        let heading_slugs: Vec<String> = headings
            .iter()
            .map(|(_, heading)| {
                let text = heading.values.iter().map(|n| n.value()).collect::<String>();
                Self::make_heading_slug(&text)
            })
            .collect();

        // Check for broken anchor links
        for node in &ast.nodes {
            if let Some((anchor, pos)) = Self::extract_anchor_link(node)
                && !heading_slugs.contains(&anchor)
                && let Ok(_lsp_uri) = uri.parse::<LspUri>()
            {
                diagnostics.push(Diagnostic {
                    range: Self::position_to_lsp_range(pos),
                    severity: Some(DiagnosticSeverity::WARNING),
                    message: format!("Broken link: heading '{}' not found", anchor),
                    ..Default::default()
                });
            }
        }

        if !diagnostics.is_empty()
            && let Ok(lsp_uri) = uri.parse::<LspUri>()
        {
            let _ =
                self.event_tx
                    .send(LspEvent::Diagnostics(lsp_types::PublishDiagnosticsParams {
                        uri: lsp_uri,
                        diagnostics,
                        version: None,
                    }));
        }
    }

    /// Extract anchor link from node if it's an anchor link
    fn extract_anchor_link(node: &Node) -> Option<(String, &MdPosition)> {
        match node {
            Node::Link(Link {
                url,
                position: Some(pos),
                ..
            }) => {
                let url_str = url.as_str();
                if let Some(anchor) = url_str.strip_prefix('#') {
                    Some((anchor.to_string(), pos))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Get completion items at position
    fn get_completions(&self, uri: &str, line: u32, character: u32) -> Vec<CompletionItem> {
        let Some(doc) = self.documents.get(uri) else {
            return vec![];
        };

        // Check if inside code block - suppress completions
        if let Some(ref ast) = doc.ast
            && self.is_inside_code_block(ast, line, character)
        {
            return vec![];
        }

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
                "ruby",
                "elixir",
                "clojure",
                "haskell",
                "scala",
                "sh",
                "jq",
                "mq",
                // Added languages
                "shell",
                "powershell",
                "zsh",
                "fish",
                "perl",
                "php",
                "swift",
                "kotlin",
                "dart",
                "lua",
                "groovy",
                "objective-c",
                "ocaml",
                "r",
                "matlab",
                "fortran",
                "assembly",
                "pascal",
                "vb",
                "fsharp",
                "erlang",
                "nim",
                "crystal",
                "julia",
                "prolog",
                "smalltalk",
                "tcl",
                "coffee",
                "typescriptreact",
                "jsx",
                "tsx",
                "css",
                "scss",
                "less",
                "html",
                "xml",
                "markdown",
                "plaintext",
                "sql",
                "sqlite",
                "postgres",
                "mysql",
                "redis",
                "dockerfile",
                "terraform",
                "ansible",
                "puppet",
                "makefile",
                "cmake",
                "gradle",
                "ini",
                "toml",
                "csv",
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

        // Heading anchor completions for links to same document (AST-based)
        if prefix.contains("](#")
            && let Some(ref ast) = doc.ast
        {
            let headings = self.collect_headings(ast);
            for (i, (_node, heading)) in headings.iter().enumerate() {
                let heading_text = heading.values.iter().map(|n| n.value()).collect::<String>();
                let slug = Self::make_heading_slug(&heading_text);
                items.push(CompletionItem {
                    label: format!("#{}", slug),
                    detail: Some(format!("H{}: {}", heading.depth, heading_text)),
                    kind: Some(CompletionItemKind::REFERENCE),
                    insert_text: Some(slug),
                    sort_text: Some(format!("{:04}", i)),
                    ..Default::default()
                });
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
    fn get_definition(
        &self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Option<GotoDefinitionResponse> {
        let doc = self.documents.get(uri)?;
        let ast = doc.ast.as_ref()?;

        // Find node at cursor position
        let node = self.find_node_at_position(ast, line, character)?;

        // Handle Link nodes
        if let Node::Link(link) = node {
            let url_str = link.url.as_str();

            // Handle heading anchor links (#heading-slug)
            if let Some(anchor) = url_str.strip_prefix('#') {
                let headings = self.collect_headings(ast);
                for (heading_node, heading) in headings {
                    let heading_text = heading.values.iter().map(|n| n.value()).collect::<String>();
                    let heading_slug = Self::make_heading_slug(&heading_text);
                    if heading_slug == anchor
                        && let Some(pos) = heading_node.position()
                    {
                        return Some(GotoDefinitionResponse::Scalar(Location {
                            uri: uri.parse().ok()?,
                            range: Self::position_to_lsp_range(&pos),
                        }));
                    }
                }
            }

            // Handle relative file links
            if !url_str.starts_with("http://") && !url_str.starts_with("https://") {
                // Extract file path (remove anchor if present)
                let file_part = url_str.split('#').next().unwrap_or(url_str);
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

        None
    }

    /// Get references for headings and links
    fn get_references(
        &self,
        uri: &str,
        line: u32,
        character: u32,
        include_declaration: bool,
    ) -> Vec<Location> {
        let Some(doc) = self.documents.get(uri) else {
            return vec![];
        };
        let Some(ast) = doc.ast.as_ref() else {
            return vec![];
        };
        let Some(node) = self.find_node_at_position(ast, line, character) else {
            return vec![];
        };
        let lsp_uri: LspUri = match uri.parse() {
            Ok(u) => u,
            Err(_) => return vec![],
        };

        let mut locations = Vec::new();

        match node {
            Node::Heading(heading) => {
                let heading_text = heading.values.iter().map(|n| n.value()).collect::<String>();
                let slug = Self::make_heading_slug(&heading_text);

                // Include the heading itself as declaration
                if include_declaration && let Some(pos) = node.position() {
                    locations.push(Location {
                        uri: lsp_uri.clone(),
                        range: Self::position_to_lsp_range(&pos),
                    });
                }

                // Find all links referencing this heading
                for n in &ast.nodes {
                    if let Some((anchor, pos)) = Self::extract_anchor_link(n)
                        && anchor == slug
                    {
                        locations.push(Location {
                            uri: lsp_uri.clone(),
                            range: Self::position_to_lsp_range(pos),
                        });
                    }
                }
            }
            Node::Link(link) => {
                let url_str = link.url.as_str();

                if let Some(anchor) = url_str.strip_prefix('#') {
                    // Include the target heading as declaration
                    if include_declaration {
                        let headings = self.collect_headings(ast);
                        for (heading_node, heading) in &headings {
                            let heading_text =
                                heading.values.iter().map(|n| n.value()).collect::<String>();
                            if Self::make_heading_slug(&heading_text) == anchor
                                && let Some(pos) = heading_node.position()
                            {
                                locations.push(Location {
                                    uri: lsp_uri.clone(),
                                    range: Self::position_to_lsp_range(&pos),
                                });
                            }
                        }
                    }

                    // Find all links with the same anchor
                    for n in &ast.nodes {
                        if let Some((other_anchor, pos)) = Self::extract_anchor_link(n)
                            && other_anchor == anchor
                        {
                            locations.push(Location {
                                uri: lsp_uri.clone(),
                                range: Self::position_to_lsp_range(pos),
                            });
                        }
                    }
                }
            }
            _ => {}
        }

        locations
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
        self.parse_document(&uri, content);
        Ok(())
    }

    fn did_change(&mut self, file_path: &Path, _version: i32, content: &str) -> miette::Result<()> {
        let uri = Self::path_to_uri(file_path);
        self.parse_document(&uri, content);
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
        file_path: &Path,
        line: u32,
        character: u32,
        include_declaration: bool,
    ) -> miette::Result<()> {
        let uri = Self::path_to_uri(file_path);
        let locations = self.get_references(&uri, line, character, include_declaration);
        if !locations.is_empty() {
            let _ = self.event_tx.send(LspEvent::References(locations));
        }
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
        lsp.parse_document("file:///test.md", content);

        assert!(lsp.documents.contains_key("file:///test.md"));
        let doc = &lsp.documents["file:///test.md"];
        assert_eq!(doc.content, content);
        assert!(doc.ast.is_some());
    }

    #[test]
    fn test_heading_completions() {
        let (mut lsp, _rx) = create_test_lsp();
        lsp.parse_document("file:///test.md", "#");

        let completions = lsp.get_completions("file:///test.md", 0, 1);
        assert!(!completions.is_empty());
        assert!(completions.iter().any(|c| c.label.contains("Heading 1")));
    }

    #[test]
    fn test_link_completions() {
        let (mut lsp, _rx) = create_test_lsp();
        lsp.parse_document("file:///test.md", "[");

        let completions = lsp.get_completions("file:///test.md", 0, 1);
        assert!(completions.iter().any(|c| c.label.contains("text](url)")));
    }

    #[test]
    fn test_anchor_completions() {
        let (mut lsp, _rx) = create_test_lsp();
        let content = "# First Heading\n\n## Second Heading\n\n[link](#";
        lsp.parse_document("file:///test.md", content);

        let completions = lsp.get_completions("file:///test.md", 4, 8);
        assert!(
            completions
                .iter()
                .any(|c| c.label.contains("first-heading"))
        );
        assert!(
            completions
                .iter()
                .any(|c| c.label.contains("second-heading"))
        );
    }

    #[test]
    fn test_references_from_heading() {
        let (mut lsp, _rx) = create_test_lsp();
        let content = "# First Heading\n\n[link1](#first-heading)\n\n[link2](#first-heading)";
        lsp.parse_document("file:///test.md", content);

        // Cursor on the heading (line 0)
        let refs = lsp.get_references("file:///test.md", 0, 5, true);
        // Should include the heading itself + 2 links
        assert_eq!(refs.len(), 3);
        assert_eq!(refs[0].range.start.line, 0); // heading declaration
    }

    #[test]
    fn test_references_from_heading_without_declaration() {
        let (mut lsp, _rx) = create_test_lsp();
        let content = "# First Heading\n\n[link1](#first-heading)\n\n[link2](#first-heading)";
        lsp.parse_document("file:///test.md", content);

        let refs = lsp.get_references("file:///test.md", 0, 5, false);
        // Should include only the 2 links, not the heading itself
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn test_references_from_link() {
        let (mut lsp, _rx) = create_test_lsp();
        let content = "# First Heading\n\n[link1](#first-heading)\n\n[link2](#first-heading)";
        lsp.parse_document("file:///test.md", content);

        // Cursor on the first link (line 2)
        let refs = lsp.get_references("file:///test.md", 2, 10, true);
        // Should include the heading + 2 links
        assert_eq!(refs.len(), 3);
    }

    #[test]
    fn test_references_no_matches() {
        let (mut lsp, _rx) = create_test_lsp();
        let content = "Just some text\n\nMore text";
        lsp.parse_document("file:///test.md", content);

        let refs = lsp.get_references("file:///test.md", 0, 5, true);
        assert!(refs.is_empty());
    }

    #[test]
    fn test_definition_heading_anchor() {
        let (mut lsp, _rx) = create_test_lsp();
        let content = "# First Heading\n\n[link](#first-heading)";
        lsp.parse_document("file:///test.md", content);

        let definition = lsp.get_definition("file:///test.md", 2, 10);
        assert!(definition.is_some());

        if let Some(GotoDefinitionResponse::Scalar(loc)) = definition {
            assert_eq!(loc.range.start.line, 0);
        }
    }
}
