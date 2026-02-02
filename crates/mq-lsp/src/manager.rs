use crate::backend::LspBackend;
use crate::client::{LspClient, LspEvent};
use crate::markdown_lsp::MarkdownLsp;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

/// Configuration for a specific LSP server
#[derive(Debug, Clone)]
pub struct LspServerConfig {
    /// Command to start the LSP server (e.g., "rust-analyzer")
    /// Not required for embedded LSP servers
    pub command: String,

    /// Command-line arguments for the server
    pub args: Vec<String>,

    /// Use embedded LSP implementation instead of external process
    /// When true, the command field is ignored
    pub embedded: bool,

    /// Enable code completion
    pub enable_completion: bool,

    /// Enable diagnostics (errors, warnings)
    pub enable_diagnostics: bool,

    /// Enable go-to-definition
    pub enable_goto_definition: bool,
}

impl Default for LspServerConfig {
    fn default() -> Self {
        Self {
            command: String::new(),
            args: Vec::new(),
            embedded: false,
            enable_completion: true,
            enable_diagnostics: true,
            enable_goto_definition: true,
        }
    }
}

/// Manages LSP backends for different languages
pub struct LspManager {
    /// Active LSP backends by language ID
    backends: HashMap<String, Box<dyn LspBackend>>,
    /// Event receivers by language ID
    event_receivers: HashMap<String, mpsc::Receiver<LspEvent>>,
    /// Completion trigger characters by language ID
    trigger_characters: HashMap<String, Vec<String>>,
    /// Server configurations by language ID
    server_configs: HashMap<String, LspServerConfig>,
    /// Workspace root path
    root_path: PathBuf,
}

impl LspManager {
    /// Create a new LSP manager
    pub fn new(server_configs: HashMap<String, LspServerConfig>, root_path: PathBuf) -> Self {
        Self {
            backends: HashMap::new(),
            event_receivers: HashMap::new(),
            trigger_characters: HashMap::new(),
            server_configs,
            root_path,
        }
    }

    /// Get or create an LSP backend for the given language
    pub fn get_or_create_backend(
        &mut self,
        language_id: &str,
    ) -> miette::Result<&mut dyn LspBackend> {
        // If backend already exists, return it
        if self.backends.contains_key(language_id) {
            return Ok(self.backends.get_mut(language_id).unwrap().as_mut());
        }

        // Get server config for this language
        let server_config = self
            .server_configs
            .get(language_id)
            .ok_or_else(|| {
                miette::miette!("No LSP server configured for language: {}", language_id)
            })?
            .clone();

        // Create event channel
        let (event_tx, event_rx) = mpsc::channel();

        // Create backend based on configuration
        let backend: Box<dyn LspBackend> = if server_config.embedded {
            // Create embedded backend
            match language_id {
                "markdown" => Box::new(MarkdownLsp::new(self.root_path.clone(), event_tx)),
                _ => {
                    return Err(miette::miette!(
                        "No embedded LSP implementation for language: {}",
                        language_id
                    ))
                }
            }
        } else {
            // Create external process backend
            let (client, client_event_rx) = LspClient::new(
                &server_config.command,
                &server_config.args,
                language_id.to_string(),
                self.root_path.clone(),
            )?;
            // Use the client's event receiver instead
            self.event_receivers
                .insert(language_id.to_string(), client_event_rx);
            Box::new(client)
        };

        // For embedded backends, store the event receiver we created
        if server_config.embedded {
            self.event_receivers
                .insert(language_id.to_string(), event_rx);
        }

        self.backends.insert(language_id.to_string(), backend);

        // Initialize the backend
        let backend = self.backends.get_mut(language_id).unwrap();
        if let Err(e) = backend.initialize() {
            eprintln!("LSP initialize error: {}", e);
        }
        if let Err(e) = backend.initialized() {
            eprintln!("LSP initialized notification error: {}", e);
        }

        Ok(backend.as_mut())
    }

    /// Notify that a document was opened
    pub fn did_open(
        &mut self,
        language_id: &str,
        file_path: &Path,
        content: &str,
    ) -> miette::Result<()> {
        let backend = self.get_or_create_backend(language_id)?;
        backend.did_open(file_path, content)?;
        Ok(())
    }

    /// Notify that a document was changed
    pub fn did_change(
        &mut self,
        language_id: &str,
        file_path: &Path,
        version: i32,
        content: &str,
    ) -> miette::Result<()> {
        let backend = self.get_or_create_backend(language_id)?;
        backend.did_change(file_path, version, content)?;
        Ok(())
    }

    /// Request semantic tokens for a document
    pub fn request_semantic_tokens(
        &mut self,
        language_id: &str,
        file_path: &Path,
    ) -> miette::Result<()> {
        let backend = self.get_or_create_backend(language_id)?;
        backend.request_semantic_tokens(file_path)?;
        Ok(())
    }

    /// Request completion at a position
    pub fn request_completion(
        &mut self,
        language_id: &str,
        file_path: &Path,
        line: u32,
        character: u32,
        trigger_character: Option<String>,
    ) -> miette::Result<()> {
        let backend = self.get_or_create_backend(language_id)?;
        backend.request_completion(file_path, line, character, trigger_character)?;
        Ok(())
    }

    /// Set trigger characters for a language
    pub fn set_trigger_characters(&mut self, language_id: &str, chars: Vec<String>) {
        self.trigger_characters
            .insert(language_id.to_string(), chars);
    }

    /// Get trigger characters for a language
    pub fn get_trigger_characters(&self, language_id: &str) -> Option<&Vec<String>> {
        self.trigger_characters.get(language_id)
    }

    /// Check if a character is a trigger character for the given language
    pub fn is_trigger_character(&self, language_id: &str, ch: char) -> bool {
        if let Some(chars) = self.trigger_characters.get(language_id) {
            chars.iter().any(|c| c == &ch.to_string())
        } else {
            false
        }
    }

    /// Request go to definition at a position
    pub fn request_definition(
        &mut self,
        language_id: &str,
        file_path: &Path,
        line: u32,
        character: u32,
    ) -> miette::Result<()> {
        let backend = self.get_or_create_backend(language_id)?;
        backend.request_definition(file_path, line, character)?;
        Ok(())
    }

    /// Request find references at a position
    pub fn request_references(
        &mut self,
        language_id: &str,
        file_path: &Path,
        line: u32,
        character: u32,
        include_declaration: bool,
    ) -> miette::Result<()> {
        let backend = self.get_or_create_backend(language_id)?;
        backend.request_references(file_path, line, character, include_declaration)?;
        Ok(())
    }

    /// Poll for events from all LSP backends
    pub fn poll_events(&mut self) -> Vec<(String, LspEvent)> {
        let mut events = Vec::new();

        for (language_id, receiver) in &mut self.event_receivers {
            while let Ok(event) = receiver.try_recv() {
                events.push((language_id.clone(), event));
            }
        }

        events
    }

    /// Check if an LSP backend exists for the given language
    pub fn has_client(&self, language_id: &str) -> bool {
        self.backends.contains_key(language_id)
    }

    /// Check if LSP is enabled for the given language
    pub fn is_enabled(&self, language_id: &str) -> bool {
        self.server_configs.contains_key(language_id)
    }

    /// Shutdown all LSP backends
    pub fn shutdown_all(&mut self) -> miette::Result<()> {
        for (_, mut backend) in self.backends.drain() {
            let _ = backend.shutdown();
        }
        Ok(())
    }
}

impl Drop for LspManager {
    fn drop(&mut self) {
        let _ = self.shutdown_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_default_configs() -> HashMap<String, LspServerConfig> {
        let mut configs = HashMap::new();
        configs.insert(
            "markdown".to_string(),
            LspServerConfig {
                command: String::new(),
                args: vec![],
                embedded: true,
                enable_completion: true,
                enable_diagnostics: false,
                enable_goto_definition: true,
            },
        );
        configs
    }

    #[test]
    fn test_lsp_manager_creation() {
        let configs = create_default_configs();
        let root_path = PathBuf::from("/tmp/test");
        let manager = LspManager::new(configs, root_path);

        assert_eq!(manager.backends.len(), 0);
        assert!(!manager.has_client("rust"));
    }

    #[test]
    fn test_is_enabled() {
        let mut configs = create_default_configs();
        configs.insert(
            "rust".to_string(),
            LspServerConfig {
                command: "rust-analyzer".to_string(),
                args: vec![],
                embedded: false,
                enable_completion: true,
                enable_diagnostics: true,
                enable_goto_definition: true,
            },
        );

        let root_path = PathBuf::from("/tmp/test");
        let manager = LspManager::new(configs, root_path);

        assert!(manager.is_enabled("rust"));
        assert!(manager.is_enabled("markdown"));
        assert!(!manager.is_enabled("unknown"));
    }

    #[test]
    fn test_poll_events_empty() {
        let configs = create_default_configs();
        let root_path = PathBuf::from("/tmp/test");
        let mut manager = LspManager::new(configs, root_path);

        let events = manager.poll_events();
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn test_markdown_embedded_lsp() {
        let configs = create_default_configs();
        let root_path = PathBuf::from("/tmp/test");
        let mut manager = LspManager::new(configs, root_path);

        // Should be able to create markdown backend
        let result = manager.get_or_create_backend("markdown");
        assert!(result.is_ok());

        // Backend should exist now
        assert!(manager.has_client("markdown"));
    }

    #[test]
    fn test_is_trigger_character() {
        let configs = create_default_configs();
        let root_path = PathBuf::from("/tmp/test");
        let mut manager = LspManager::new(configs, root_path);

        // Set trigger characters for markdown
        manager.set_trigger_characters("markdown", vec!["#".to_string(), "[".to_string()]);

        assert!(manager.is_trigger_character("markdown", '#'));
        assert!(manager.is_trigger_character("markdown", '['));
        assert!(!manager.is_trigger_character("markdown", '.'));
        assert!(!manager.is_trigger_character("rust", '#'));
    }
}
