use miette::Result;
use mq_markdown::Markdown;

use crate::document::{FileType, LineMap};

/// Document-specific data based on file type
#[derive(Debug, Clone)]
pub enum DocumentType {
    /// Markdown document with parsed AST and line mapping
    Markdown {
        /// Parsed markdown AST
        ast: Markdown,
        /// Maps visual lines to AST nodes
        line_map: LineMap,
    },
    /// Code file with language identifier
    Code {
        /// Programming language identifier
        language: String,
    },
    /// Plain text file (no special handling)
    PlainText,
}

impl DocumentType {
    /// Create a new Markdown document type
    pub fn new_markdown(content: &str) -> Result<Self> {
        let ast = Markdown::from_markdown_str(content)
            .map_err(|e| miette::miette!("Failed to parse markdown: {}", e))?;
        let line_map = LineMap::from_markdown(&ast);

        Ok(Self::Markdown { ast, line_map })
    }

    /// Create a new Code document type
    pub fn new_code(language: String) -> Self {
        Self::Code { language }
    }

    /// Create a new PlainText document type
    pub fn new_plain_text() -> Self {
        Self::PlainText
    }

    /// Get the file type for this document
    pub fn file_type(&self) -> FileType {
        match self {
            Self::Markdown { .. } => FileType::Markdown,
            Self::Code { language } => FileType::Code(language.clone()),
            Self::PlainText => FileType::PlainText,
        }
    }

    /// Get the Markdown AST if this is a Markdown document
    pub fn markdown_ast(&self) -> Option<&Markdown> {
        match self {
            Self::Markdown { ast, .. } => Some(ast),
            _ => None,
        }
    }

    /// Get the line map if this is a Markdown document
    pub fn line_map(&self) -> Option<&LineMap> {
        match self {
            Self::Markdown { line_map, .. } => Some(line_map),
            _ => None,
        }
    }

    /// Get the language identifier if this is a Code document
    pub fn language(&self) -> Option<&str> {
        match self {
            Self::Code { language } => Some(language.as_str()),
            _ => None,
        }
    }

    /// Rebuild document-specific structures after content change
    ///
    /// For Markdown documents, this reparses the AST and rebuilds the line map.
    /// For Code and PlainText documents, this is a no-op.
    pub fn rebuild(&mut self, content: &str) -> Result<()> {
        match self {
            Self::Markdown { ast, line_map } => {
                let new_ast = Markdown::from_markdown_str(content)
                    .map_err(|e| miette::miette!("Failed to reparse markdown: {}", e))?;
                *ast = new_ast;
                *line_map = LineMap::from_markdown(ast);
                Ok(())
            }
            Self::Code { .. } | Self::PlainText => {
                // No rebuild needed for code/plain text
                Ok(())
            }
        }
    }

    /// Check if this document type supports AST-based operations
    pub fn has_ast(&self) -> bool {
        matches!(self, Self::Markdown { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_document_creation() {
        let doc = DocumentType::new_markdown("# Hello World").unwrap();
        assert!(doc.has_ast());
        assert!(doc.markdown_ast().is_some());
        assert!(doc.line_map().is_some());
        assert_eq!(doc.file_type(), FileType::Markdown);
    }

    #[test]
    fn test_code_document_creation() {
        let doc = DocumentType::new_code("rust".to_string());
        assert!(!doc.has_ast());
        assert_eq!(doc.language(), Some("rust"));
        assert_eq!(doc.file_type(), FileType::Code("rust".to_string()));
    }

    #[test]
    fn test_plain_text_document_creation() {
        let doc = DocumentType::new_plain_text();
        assert!(!doc.has_ast());
        assert_eq!(doc.file_type(), FileType::PlainText);
    }

    #[test]
    fn test_markdown_rebuild() {
        let mut doc = DocumentType::new_markdown("# Title").unwrap();
        doc.rebuild("## Subtitle").unwrap();
        assert!(doc.markdown_ast().is_some());
    }
}
