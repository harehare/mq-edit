use std::path::Path;

/// File type classification
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileType {
    /// Markdown file
    Markdown,
    /// Code file with language identifier
    Code(String),
    /// Plain text file
    PlainText,
}

impl FileType {
    /// Detect file type from file extension
    pub fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|e| e.to_str()) {
            Some("md") | Some("markdown") => FileType::Markdown,
            Some("mq") => FileType::Code("mq".to_string()),
            Some("rs") => FileType::Code("rust".to_string()),
            Some("py") => FileType::Code("python".to_string()),
            Some("js") => FileType::Code("javascript".to_string()),
            Some("ts") => FileType::Code("typescript".to_string()),
            Some("tsx") => FileType::Code("typescriptreact".to_string()),
            Some("jsx") => FileType::Code("javascriptreact".to_string()),
            Some("go") => FileType::Code("go".to_string()),
            Some("java") => FileType::Code("java".to_string()),
            Some("cpp") | Some("cc") | Some("cxx") | Some("c++") => {
                FileType::Code("cpp".to_string())
            }
            Some("c") => FileType::Code("c".to_string()),
            Some("h") | Some("hpp") | Some("hxx") => FileType::Code("cpp".to_string()),
            Some("json") => FileType::Code("json".to_string()),
            Some("toml") => FileType::Code("toml".to_string()),
            Some("yaml") | Some("yml") => FileType::Code("yaml".to_string()),
            Some("html") => FileType::Code("html".to_string()),
            Some("css") => FileType::Code("css".to_string()),
            Some("xml") => FileType::Code("xml".to_string()),
            Some("sh") | Some("bash") => FileType::Code("bash".to_string()),
            Some("rb") => FileType::Code("ruby".to_string()),
            Some("php") => FileType::Code("php".to_string()),
            Some("swift") => FileType::Code("swift".to_string()),
            Some("kt") | Some("kts") => FileType::Code("kotlin".to_string()),
            Some("scala") => FileType::Code("scala".to_string()),
            Some("hs") => FileType::Code("haskell".to_string()),
            Some("elm") => FileType::Code("elm".to_string()),
            Some("vim") => FileType::Code("vim".to_string()),
            Some("lua") => FileType::Code("lua".to_string()),
            Some("txt") | Some("text") => FileType::PlainText,
            _ => FileType::PlainText,
        }
    }

    /// Get LSP language identifier
    ///
    /// Returns the language identifier that should be used with LSP servers.
    /// This follows the LSP specification for language identifiers.
    pub fn lsp_language_id(&self) -> Option<&str> {
        match self {
            FileType::Code(lang) => Some(lang.as_str()),
            FileType::Markdown => Some("markdown"),
            FileType::PlainText => None,
        }
    }

    /// Get human-readable name for this file type
    pub fn display_name(&self) -> &str {
        match self {
            FileType::Markdown => "Markdown",
            FileType::Code(lang) => lang.as_str(),
            FileType::PlainText => "Plain Text",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_markdown_detection() {
        assert_eq!(
            FileType::from_path(&PathBuf::from("file.md")),
            FileType::Markdown
        );
        assert_eq!(
            FileType::from_path(&PathBuf::from("README.markdown")),
            FileType::Markdown
        );
    }

    #[test]
    fn test_code_detection() {
        assert_eq!(
            FileType::from_path(&PathBuf::from("main.rs")),
            FileType::Code("rust".to_string())
        );
        assert_eq!(
            FileType::from_path(&PathBuf::from("script.py")),
            FileType::Code("python".to_string())
        );
        assert_eq!(
            FileType::from_path(&PathBuf::from("app.ts")),
            FileType::Code("typescript".to_string())
        );
        assert_eq!(
            FileType::from_path(&PathBuf::from("query.mq")),
            FileType::Code("mq".to_string())
        );
    }

    #[test]
    fn test_plain_text_detection() {
        assert_eq!(
            FileType::from_path(&PathBuf::from("notes.txt")),
            FileType::PlainText
        );
        assert_eq!(
            FileType::from_path(&PathBuf::from("file.unknown")),
            FileType::PlainText
        );
    }

    #[test]
    fn test_lsp_language_id() {
        assert_eq!(
            FileType::Code("rust".to_string()).lsp_language_id(),
            Some("rust")
        );
        assert_eq!(FileType::Markdown.lsp_language_id(), Some("markdown"));
        assert_eq!(FileType::PlainText.lsp_language_id(), None);
    }
}
