use std::path::Path;

/// Trait for LSP backend implementations
///
/// This trait abstracts the communication layer for LSP servers,
/// allowing both external process-based and embedded implementations.
pub trait LspBackend: Send {
    /// Initialize the LSP server with workspace root
    fn initialize(&mut self) -> miette::Result<()>;

    /// Send initialized notification
    fn initialized(&mut self) -> miette::Result<()>;

    /// Notify that a document was opened
    fn did_open(&mut self, file_path: &Path, content: &str) -> miette::Result<()>;

    /// Notify that a document was changed
    fn did_change(&mut self, file_path: &Path, version: i32, content: &str) -> miette::Result<()>;

    /// Request semantic tokens for a document
    fn request_semantic_tokens(&mut self, file_path: &Path) -> miette::Result<()>;

    /// Request completion at a position
    fn request_completion(
        &mut self,
        file_path: &Path,
        line: u32,
        character: u32,
        trigger_character: Option<String>,
    ) -> miette::Result<()>;

    /// Request go to definition at a position
    fn request_definition(
        &mut self,
        file_path: &Path,
        line: u32,
        character: u32,
    ) -> miette::Result<()>;

    /// Request find references at a position
    fn request_references(
        &mut self,
        file_path: &Path,
        line: u32,
        character: u32,
        include_declaration: bool,
    ) -> miette::Result<()>;

    /// Shutdown the LSP backend
    fn shutdown(&mut self) -> miette::Result<()>;

    /// Get the language ID for this backend
    fn language_id(&self) -> &str;
}
