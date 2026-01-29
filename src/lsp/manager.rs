use super::client::{LspClient, LspEvent};
use crate::config::Config;
use crate::document::FileType;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

/// Manages LSP clients for different languages
pub struct LspManager {
    /// Active LSP clients by language ID
    clients: HashMap<String, LspClient>,
    /// Event receivers by language ID
    event_receivers: HashMap<String, mpsc::Receiver<LspEvent>>,
    /// Completion trigger characters by language ID
    trigger_characters: HashMap<String, Vec<String>>,
    /// Configuration
    config: Config,
    /// Workspace root path
    root_path: PathBuf,
}

impl LspManager {
    /// Create a new LSP manager
    pub fn new(config: Config, root_path: PathBuf) -> Self {
        Self {
            clients: HashMap::new(),
            event_receivers: HashMap::new(),
            trigger_characters: HashMap::new(),
            config,
            root_path,
        }
    }

    /// Get or create an LSP client for the given language
    pub fn get_or_create_client(&mut self, language_id: &str) -> miette::Result<&mut LspClient> {
        // If client already exists, return it
        if self.clients.contains_key(language_id) {
            return Ok(self.clients.get_mut(language_id).unwrap());
        }

        // Get server config for this language
        let server_config = self
            .config
            .lsp
            .servers
            .get(language_id)
            .ok_or_else(|| {
                miette::miette!("No LSP server configured for language: {}", language_id)
            })?
            .clone();

        // Create new client
        let (mut client, event_rx) = LspClient::new(
            &server_config.command,
            &server_config.args,
            language_id.to_string(),
            self.root_path.clone(),
        )?;

        // Initialize the client asynchronously (don't wait for response)
        // The client will send the initialize request and handle the response in the background
        if let Err(e) = client.initialize() {
            eprintln!("LSP initialize error: {}", e);
        }
        if let Err(e) = client.initialized() {
            eprintln!("LSP initialized notification error: {}", e);
        }

        // Store the client and event receiver
        self.event_receivers
            .insert(language_id.to_string(), event_rx);
        self.clients.insert(language_id.to_string(), client);

        Ok(self.clients.get_mut(language_id).unwrap())
    }

    /// Get the LSP client for a file type
    pub fn get_client_for_file_type(
        &mut self,
        file_type: &FileType,
    ) -> miette::Result<&mut LspClient> {
        match file_type {
            FileType::Code(language_id) => self.get_or_create_client(language_id),
            _ => Err(miette::miette!("No LSP support for this file type")),
        }
    }

    /// Notify that a document was opened
    pub fn did_open(
        &mut self,
        file_type: &FileType,
        file_path: &Path,
        content: &str,
    ) -> miette::Result<()> {
        let client = self.get_client_for_file_type(file_type)?;
        client.did_open(file_path, content)?;
        Ok(())
    }

    /// Notify that a document was changed
    pub fn did_change(
        &mut self,
        file_type: &FileType,
        file_path: &Path,
        version: i32,
        content: &str,
    ) -> miette::Result<()> {
        let client = self.get_client_for_file_type(file_type)?;
        client.did_change(file_path, version, content)?;
        Ok(())
    }

    /// Request semantic tokens for a document
    pub fn request_semantic_tokens(
        &mut self,
        file_type: &FileType,
        file_path: &Path,
    ) -> miette::Result<()> {
        let client = self.get_client_for_file_type(file_type)?;
        client.request_semantic_tokens(file_path)?;
        Ok(())
    }

    /// Request completion at a position
    pub fn request_completion(
        &mut self,
        file_type: &FileType,
        file_path: &Path,
        line: u32,
        character: u32,
        trigger_character: Option<String>,
    ) -> miette::Result<()> {
        let client = self.get_client_for_file_type(file_type)?;
        client.request_completion(file_path, line, character, trigger_character)?;
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

    /// Check if a character is a trigger character for the given file type
    pub fn is_trigger_character(&self, file_type: &FileType, ch: char) -> bool {
        match file_type {
            FileType::Code(language_id) => {
                if let Some(chars) = self.trigger_characters.get(language_id) {
                    chars.iter().any(|c| c == &ch.to_string())
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Request go to definition at a position
    pub fn request_definition(
        &mut self,
        file_type: &FileType,
        file_path: &Path,
        line: u32,
        character: u32,
    ) -> miette::Result<()> {
        let client = self.get_client_for_file_type(file_type)?;
        client.request_definition(file_path, line, character)?;
        Ok(())
    }

    /// Request find references at a position
    pub fn request_references(
        &mut self,
        file_type: &FileType,
        file_path: &Path,
        line: u32,
        character: u32,
        include_declaration: bool,
    ) -> miette::Result<()> {
        let client = self.get_client_for_file_type(file_type)?;
        client.request_references(file_path, line, character, include_declaration)?;
        Ok(())
    }

    /// Poll for events from all LSP clients
    pub fn poll_events(&mut self) -> Vec<(String, LspEvent)> {
        let mut events = Vec::new();

        for (language_id, receiver) in &mut self.event_receivers {
            while let Ok(event) = receiver.try_recv() {
                events.push((language_id.clone(), event));
            }
        }

        events
    }

    /// Check if an LSP client exists for the given language
    pub fn has_client(&self, language_id: &str) -> bool {
        self.clients.contains_key(language_id)
    }

    /// Check if LSP is enabled for the given file type
    pub fn is_enabled_for_file_type(&self, file_type: &FileType) -> bool {
        match file_type {
            FileType::Code(language_id) => self.config.lsp.servers.contains_key(language_id),
            _ => false,
        }
    }

    /// Shutdown all LSP clients
    pub fn shutdown_all(&mut self) -> miette::Result<()> {
        for (_, mut client) in self.clients.drain() {
            let _ = client.shutdown();
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
    use crate::config::LspServerConfig;

    #[test]
    fn test_lsp_manager_creation() {
        let config = Config::default();
        let root_path = PathBuf::from("/tmp/test");
        let manager = LspManager::new(config, root_path);

        assert_eq!(manager.clients.len(), 0);
        assert!(!manager.has_client("rust"));
    }

    #[test]
    fn test_is_enabled_for_file_type() {
        let mut config = Config::default();
        config.lsp.servers.insert(
            "rust".to_string(),
            LspServerConfig {
                command: "rust-analyzer".to_string(),
                args: vec![],
                enable_completion: true,
                enable_diagnostics: true,
                enable_goto_definition: true,
            },
        );

        let root_path = PathBuf::from("/tmp/test");
        let manager = LspManager::new(config, root_path);

        assert!(manager.is_enabled_for_file_type(&FileType::Code("rust".to_string())));
        assert!(!manager.is_enabled_for_file_type(&FileType::Code("unknown".to_string())));
        assert!(!manager.is_enabled_for_file_type(&FileType::Markdown));
        assert!(!manager.is_enabled_for_file_type(&FileType::PlainText));
    }

    #[test]
    fn test_poll_events_empty() {
        let config = Config::default();
        let root_path = PathBuf::from("/tmp/test");
        let mut manager = LspManager::new(config, root_path);

        let events = manager.poll_events();
        assert_eq!(events.len(), 0);
    }
}
