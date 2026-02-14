use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lsp_types::CompletionItem;
use markdown_lsp::{DiagnosticsManager, LspEvent, LspManager};
use miette::Result;

use crate::config::Config;
use crate::document::{CursorMovement, DocumentBuffer, FileType};
use crate::navigation::{FileLocation, NavigationHistory};
use crate::renderer::{CodeRenderer, ImageManager};
use crate::ui::{FileTree, SearchField, SearchMode};

/// Main application state
pub struct App {
    /// Document buffer
    buffer: DocumentBuffer,
    /// Whether the app should quit
    should_quit: bool,
    /// Scroll offset for viewport
    scroll_offset: usize,
    /// Status message (for errors, info, etc.)
    status_message: Option<String>,
    /// File browser tree
    file_tree: Option<FileTree>,
    /// Whether file browser is visible
    show_file_browser: bool,
    /// Current working directory
    current_dir: PathBuf,
    /// Application configuration
    config: Config,
    /// Image manager for rendering images
    image_manager: ImageManager,
    /// Code renderer for syntax highlighting
    code_renderer: CodeRenderer,
    /// LSP manager for language server protocol support
    lsp_manager: Option<LspManager>,
    /// Diagnostics manager for LSP diagnostics
    diagnostics_manager: DiagnosticsManager,
    /// Document version for LSP synchronization
    document_version: i32,
    /// Navigation history for jump operations
    navigation_history: NavigationHistory,
    /// Pending definition request location (for adding to history)
    pending_definition_request: Option<(PathBuf, usize, usize)>,
    /// Completion items from LSP (original unfiltered list)
    completion_items: Vec<CompletionItem>,
    /// Filtered completion items based on user input
    filtered_completion_items: Vec<CompletionItem>,
    /// Selected completion item index
    completion_selected: usize,
    /// Whether completion popup is visible
    show_completion: bool,
    /// Column position where completion started
    completion_start_column: usize,
    /// Whether quit confirmation is pending (user pressed quit with unsaved changes)
    quit_confirm_pending: bool,
    /// Whether quit confirmation dialog is visible
    show_quit_dialog: bool,
    /// Whether line numbers are visible
    show_line_numbers: bool,
    /// Whether current line highlight is visible
    show_current_line_highlight: bool,
    /// Whether search dialog is visible
    show_search_dialog: bool,
    /// Search query
    search_query: String,
    /// Replace query
    replace_query: String,
    /// Search mode (Find or Replace)
    search_mode: SearchMode,
    /// Current search results (line, column)
    search_results: Vec<(usize, usize)>,
    /// Current search result index
    search_index: Option<usize>,
    /// Active search field
    search_active_field: SearchField,
    /// Whether save-as dialog is visible
    show_save_as_dialog: bool,
    /// Filename input for save-as dialog
    save_as_filename: String,
    /// Whether goto line dialog is visible
    show_goto_line_dialog: bool,
    /// Line number input for goto line dialog
    goto_line_input: String,
    /// Whether mq query dialog is visible
    show_mq_query_dialog: bool,
    /// mq query input string
    mq_query_input: String,
    /// mq query result (output or error message)
    mq_query_result: Option<String>,
    /// Whether the app is running in pipe mode (stdin/stdout piped)
    pipe_mode: bool,
}

impl App {
    /// Create a new app with an empty buffer
    pub fn new() -> Self {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let config = Config::load_or_default();
        let mut image_manager = ImageManager::new();
        image_manager.set_base_path(current_dir.clone());

        // Create LSP manager
        let lsp_manager = Some(LspManager::new(
            config.lsp_server_configs(),
            current_dir.clone(),
        ));

        let show_line_numbers = config.editor.show_line_numbers;
        let show_current_line_highlight = config.editor.show_current_line_highlight;
        let mut code_renderer = CodeRenderer::with_theme(&config.editor.theme);
        code_renderer.set_use_semantic_tokens(config.editor.use_semantic_tokens);

        Self {
            buffer: DocumentBuffer::new(),
            should_quit: false,
            scroll_offset: 0,
            status_message: None,
            file_tree: None,
            show_file_browser: false,
            current_dir,
            config,
            image_manager,
            code_renderer,
            lsp_manager,
            diagnostics_manager: DiagnosticsManager::new(),
            document_version: 0,
            navigation_history: NavigationHistory::new(),
            pending_definition_request: None,
            completion_items: Vec::new(),
            filtered_completion_items: Vec::new(),
            completion_selected: 0,
            show_completion: false,
            completion_start_column: 0,
            quit_confirm_pending: false,
            show_quit_dialog: false,
            show_line_numbers,
            show_current_line_highlight,
            show_search_dialog: false,
            search_query: String::new(),
            replace_query: String::new(),
            search_mode: SearchMode::Find,
            search_results: Vec::new(),
            search_index: None,
            search_active_field: SearchField::Search,
            show_save_as_dialog: false,
            save_as_filename: String::new(),
            show_goto_line_dialog: false,
            goto_line_input: String::new(),
            show_mq_query_dialog: false,
            mq_query_input: String::new(),
            mq_query_result: None,
            pipe_mode: false,
        }
    }

    /// Create app from a file
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let buffer = DocumentBuffer::from_file(path)?;
        let current_dir = path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let config = Config::load_or_default();
        let mut image_manager = ImageManager::new();
        image_manager.set_base_path(path.to_path_buf());

        // Create LSP manager
        let mut lsp_manager = Some(LspManager::new(
            config.lsp_server_configs(),
            current_dir.clone(),
        ));

        // Notify LSP that a document was opened
        let diagnostics_manager = DiagnosticsManager::new();
        let language_id = file_type_to_language_id(buffer.file_type());
        if let Some(ref mut lsp) = lsp_manager
            && let Some(lang_id) = &language_id
            && lsp.is_enabled(lang_id)
        {
            let content = buffer.content();
            if let Err(e) = lsp.did_open(lang_id, path, &content) {
                eprintln!("LSP did_open error: {}", e);
            }
        }

        let show_line_numbers = config.editor.show_line_numbers;
        let show_current_line_highlight = config.editor.show_current_line_highlight;
        let mut code_renderer = CodeRenderer::with_theme(&config.editor.theme);
        code_renderer.set_use_semantic_tokens(config.editor.use_semantic_tokens);

        Ok(Self {
            buffer,
            should_quit: false,
            scroll_offset: 0,
            status_message: None,
            file_tree: None,
            show_file_browser: false,
            current_dir,
            config,
            image_manager,
            code_renderer,
            lsp_manager,
            diagnostics_manager,
            document_version: 1,
            navigation_history: NavigationHistory::new(),
            pending_definition_request: None,
            completion_items: Vec::new(),
            filtered_completion_items: Vec::new(),
            completion_selected: 0,
            show_completion: false,
            completion_start_column: 0,
            quit_confirm_pending: false,
            show_quit_dialog: false,
            show_line_numbers,
            show_current_line_highlight,
            show_search_dialog: false,
            search_query: String::new(),
            replace_query: String::new(),
            search_mode: SearchMode::Find,
            search_results: Vec::new(),
            search_index: None,
            search_active_field: SearchField::Search,
            show_save_as_dialog: false,
            save_as_filename: String::new(),
            show_goto_line_dialog: false,
            goto_line_input: String::new(),
            show_mq_query_dialog: false,
            mq_query_input: String::new(),
            mq_query_result: None,
            pipe_mode: false,
        })
    }

    /// Create app from a string content (for pipe mode)
    pub fn from_string(content: &str) -> Result<Self> {
        let buffer = DocumentBuffer::from_string(content)?;
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let config = Config::load_or_default();
        let mut image_manager = ImageManager::new();
        image_manager.set_base_path(current_dir.clone());

        let lsp_manager = Some(LspManager::new(
            config.lsp_server_configs(),
            current_dir.clone(),
        ));

        let show_line_numbers = config.editor.show_line_numbers;
        let show_current_line_highlight = config.editor.show_current_line_highlight;
        let mut code_renderer = CodeRenderer::with_theme(&config.editor.theme);
        code_renderer.set_use_semantic_tokens(config.editor.use_semantic_tokens);

        Ok(Self {
            buffer,
            should_quit: false,
            scroll_offset: 0,
            status_message: None,
            file_tree: None,
            show_file_browser: false,
            current_dir,
            config,
            image_manager,
            code_renderer,
            lsp_manager,
            diagnostics_manager: DiagnosticsManager::new(),
            document_version: 0,
            navigation_history: NavigationHistory::new(),
            pending_definition_request: None,
            completion_items: Vec::new(),
            filtered_completion_items: Vec::new(),
            completion_selected: 0,
            show_completion: false,
            completion_start_column: 0,
            quit_confirm_pending: false,
            show_quit_dialog: false,
            show_line_numbers,
            show_current_line_highlight,
            show_search_dialog: false,
            search_query: String::new(),
            replace_query: String::new(),
            search_mode: SearchMode::Find,
            search_results: Vec::new(),
            search_index: None,
            search_active_field: SearchField::Search,
            show_save_as_dialog: false,
            save_as_filename: String::new(),
            show_goto_line_dialog: false,
            goto_line_input: String::new(),
            show_mq_query_dialog: false,
            mq_query_input: String::new(),
            mq_query_result: None,
            pipe_mode: false,
        })
    }

    /// Get the document buffer
    pub fn buffer(&self) -> &DocumentBuffer {
        &self.buffer
    }

    /// Get mutable document buffer
    pub fn buffer_mut(&mut self) -> &mut DocumentBuffer {
        &mut self.buffer
    }

    /// Set pipe mode (stdin/stdout piped)
    pub fn set_pipe_mode(&mut self, pipe_mode: bool) {
        self.pipe_mode = pipe_mode;
    }

    /// Check if the app should quit
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Get scroll offset
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Get image manager
    pub fn image_manager(&self) -> &ImageManager {
        &self.image_manager
    }

    /// Get mutable image manager
    pub fn image_manager_mut(&mut self) -> &mut ImageManager {
        &mut self.image_manager
    }

    /// Get diagnostics manager
    pub fn diagnostics_manager(&self) -> &DiagnosticsManager {
        &self.diagnostics_manager
    }

    /// Get code renderer
    pub fn code_renderer(&self) -> &CodeRenderer {
        &self.code_renderer
    }

    /// Get mutable code renderer
    pub fn code_renderer_mut(&mut self) -> &mut CodeRenderer {
        &mut self.code_renderer
    }

    /// Get status message
    pub fn status_message(&self) -> Option<&str> {
        self.status_message.as_deref()
    }

    /// Set status message
    pub fn set_status_message(&mut self, message: String) {
        self.status_message = Some(message);
    }

    /// Clear status message
    pub fn clear_status_message(&mut self) {
        self.status_message = None;
    }

    /// Get completion items (filtered by current input)
    pub fn filtered_completion_items(&self) -> &[CompletionItem] {
        &self.filtered_completion_items
    }

    /// Get selected completion index
    pub fn completion_selected(&self) -> usize {
        self.completion_selected
    }

    /// Check if completion popup is visible
    pub fn show_completion(&self) -> bool {
        self.show_completion
    }

    /// Get the current completion prefix (text typed since completion started)
    fn get_completion_prefix(&self) -> String {
        let cursor = self.buffer.cursor();
        let line = self.buffer.line(cursor.line).unwrap_or_default();
        if cursor.column > self.completion_start_column {
            line.chars()
                .skip(self.completion_start_column)
                .take(cursor.column - self.completion_start_column)
                .collect()
        } else {
            String::new()
        }
    }

    /// Filter completion items based on user input
    fn filter_completion_items(&mut self) {
        let prefix = self.get_completion_prefix().to_lowercase();
        self.filtered_completion_items = self
            .completion_items
            .iter()
            .filter(|item| {
                if prefix.is_empty() {
                    true
                } else {
                    item.label.to_lowercase().contains(&prefix)
                }
            })
            .cloned()
            .collect();

        // Reset selection if it's out of bounds
        if self.filtered_completion_items.is_empty()
            || self.completion_selected >= self.filtered_completion_items.len()
        {
            self.completion_selected = 0;
        }

        // Hide completion if no items match
        if self.filtered_completion_items.is_empty() && !prefix.is_empty() {
            self.show_completion = false;
        }
    }

    /// Get file tree
    pub fn file_tree(&self) -> Option<&FileTree> {
        self.file_tree.as_ref()
    }

    /// Get mutable file tree
    pub fn file_tree_mut(&mut self) -> Option<&mut FileTree> {
        self.file_tree.as_mut()
    }

    /// Check if file browser is visible
    pub fn is_file_browser_visible(&self) -> bool {
        self.show_file_browser
    }

    /// Toggle file browser visibility
    pub fn toggle_file_browser(&mut self) {
        self.show_file_browser = !self.show_file_browser;

        // Initialize file tree if it doesn't exist
        if self.show_file_browser && self.file_tree.is_none() {
            self.file_tree = Some(FileTree::new(&self.current_dir));
        }
    }

    /// Check if line numbers are visible
    pub fn show_line_numbers(&self) -> bool {
        self.show_line_numbers
    }

    /// Toggle line numbers visibility
    pub fn toggle_line_numbers(&mut self) {
        self.show_line_numbers = !self.show_line_numbers;
    }

    /// Check if current line highlight is visible
    pub fn show_current_line_highlight(&self) -> bool {
        self.show_current_line_highlight
    }

    /// Toggle current line highlight visibility
    pub fn toggle_current_line_highlight(&mut self) {
        self.show_current_line_highlight = !self.show_current_line_highlight;
    }

    /// Calculate the width of line number gutter (including separator)
    pub fn line_number_gutter_width(&self) -> u16 {
        if self.show_line_numbers {
            let total_lines = self.buffer.line_count();
            let digits = if total_lines == 0 {
                1
            } else {
                ((total_lines as f64).log10().floor() as usize) + 1
            }
            .max(3);
            // digits + " │ " (3 characters: space + vertical bar + space)
            (digits + 3) as u16
        } else {
            0
        }
    }

    /// Open file from path
    pub fn open_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        self.buffer = DocumentBuffer::from_file(path)?;

        if let Some(parent) = path.parent() {
            self.current_dir = parent.to_path_buf();
        }

        self.scroll_offset = 0;
        self.document_version = 1;

        // Notify LSP that a document was opened
        let language_id = file_type_to_language_id(self.buffer.file_type());
        if let Some(ref mut lsp) = self.lsp_manager
            && let Some(lang_id) = &language_id
            && lsp.is_enabled(lang_id)
        {
            let content = self.buffer.content();
            if let Err(e) = lsp.did_open(lang_id, path, &content) {
                eprintln!("LSP did_open error: {}", e);
            }
        }

        Ok(())
    }

    /// Poll LSP events and update diagnostics
    pub fn poll_lsp_events(&mut self) {
        // Collect events first (separate the borrow from processing)
        let events = if let Some(ref mut lsp) = self.lsp_manager {
            lsp.poll_events()
        } else {
            Vec::new()
        };

        // Process events without holding the lsp_manager borrow
        for (_language_id, event) in events {
            match event {
                LspEvent::Diagnostics(params) => {
                    // Convert LSP diagnostics to our format and update the manager
                    self.diagnostics_manager.update(params.diagnostics);
                }
                LspEvent::SemanticTokens(_uri, tokens) => {
                    // Semantic tokens received - decode and store in CodeRenderer
                    let content = self.buffer.content();
                    let decoded_tokens =
                        crate::renderer::code::decode_semantic_tokens(&tokens, &content);
                    self.code_renderer.set_semantic_tokens(decoded_tokens);
                }
                LspEvent::Completion(completion) => {
                    // Completion items received - display popup
                    use lsp_types::CompletionResponse;

                    let items = match completion {
                        CompletionResponse::Array(items) => items,
                        CompletionResponse::List(list) => list.items,
                    };

                    if !items.is_empty() {
                        self.completion_items = items;
                        // Note: completion_start_column is set in request_completion()
                        self.completion_selected = 0;
                        self.show_completion = true;
                        self.filter_completion_items();
                    }
                }
                LspEvent::Definition(response) => {
                    // Definition response received
                    // Save current location to history if we had a pending request
                    if let Some((file_path, line, column)) = self.pending_definition_request.take()
                    {
                        self.navigation_history
                            .push(FileLocation::new(file_path, line, column));
                    }

                    // Jump to the definition location
                    if let Err(e) = self.jump_to_definition(response) {
                        self.set_status_message(format!("Failed to jump to definition: {}", e));
                    }
                }
                LspEvent::References(_locations) => {
                    // References response received
                    // TODO: Display references list
                }
                LspEvent::Initialized(trigger_chars) => {
                    // Server initialized - save trigger characters and request semantic tokens
                    if let Some(ref mut lsp) = self.lsp_manager {
                        // Save trigger characters for this language
                        if let FileType::Code(lang_id) = self.buffer.file_type() {
                            lsp.set_trigger_characters(lang_id, trigger_chars);
                        } else if let FileType::Markdown = self.buffer.file_type() {
                            lsp.set_trigger_characters("markdown", trigger_chars);
                        }

                        let language_id = file_type_to_language_id(self.buffer.file_type());
                        if let Some(file_path) = self.buffer.file_path()
                            && let Some(lang_id) = &language_id
                            && lsp.is_enabled(lang_id)
                        {
                            let path_buf = file_path.to_path_buf();
                            if let Err(e) = lsp.request_semantic_tokens(lang_id, &path_buf) {
                                eprintln!("Failed to request semantic tokens: {}", e);
                            }
                        }
                    }
                }
                LspEvent::Error(err) => {
                    eprintln!("LSP error: {}", err);
                }
            }
        }
    }

    /// Notify LSP that the document has changed
    fn notify_lsp_document_change(&mut self) {
        let language_id = file_type_to_language_id(self.buffer.file_type());
        if let Some(ref mut lsp) = self.lsp_manager
            && let Some(file_path) = self.buffer.file_path()
            && let Some(lang_id) = &language_id
            && lsp.is_enabled(lang_id)
        {
            self.document_version += 1;
            let content = self.buffer.content();
            let path_buf = file_path.to_path_buf();
            if let Err(e) = lsp.did_change(lang_id, &path_buf, self.document_version, &content) {
                eprintln!("LSP did_change error: {}", e);
            }

            // Request new semantic tokens after document change
            if let Err(e) = lsp.request_semantic_tokens(lang_id, &path_buf) {
                eprintln!("Failed to request semantic tokens: {}", e);
            }
        }
    }

    /// Request go to definition at current cursor position
    pub fn request_go_to_definition(&mut self) -> Result<()> {
        let language_id = file_type_to_language_id(self.buffer.file_type());
        if let Some(ref mut lsp) = self.lsp_manager {
            if let Some(file_path) = self.buffer.file_path() {
                if let Some(lang_id) = &language_id
                    && lsp.is_enabled(lang_id)
                {
                    let cursor = self.buffer.cursor();
                    let path_buf = file_path.to_path_buf();

                    // Store current location for history
                    self.pending_definition_request =
                        Some((path_buf.clone(), cursor.line, cursor.column));

                    lsp.request_definition(
                        lang_id,
                        &path_buf,
                        cursor.line as u32,
                        cursor.column as u32,
                    )
                    .map_err(|e| miette::miette!("Failed to request definition: {}", e))?;

                    self.set_status_message("Requesting definition...".to_string());
                } else {
                    self.set_status_message("LSP not available for this file type".to_string());
                }
            } else {
                self.set_status_message("No file open".to_string());
            }
        } else {
            self.set_status_message("LSP not initialized".to_string());
        }
        Ok(())
    }

    /// Jump to a definition response location
    fn jump_to_definition(&mut self, response: lsp_types::GotoDefinitionResponse) -> Result<()> {
        use lsp_types::GotoDefinitionResponse;

        // Extract the first location from the response
        let location = match response {
            GotoDefinitionResponse::Scalar(loc) => Some(loc),
            GotoDefinitionResponse::Array(locs) => locs.into_iter().next(),
            GotoDefinitionResponse::Link(links) => {
                // Convert LocationLink to Location
                links.into_iter().next().map(|link| lsp_types::Location {
                    uri: link.target_uri,
                    range: link.target_selection_range,
                })
            }
        };

        if let Some(loc) = location {
            // Convert URI to file path - strip file:// prefix if present
            let path_str = loc.uri.path().as_str();
            let path = PathBuf::from(path_str);

            // Jump to the location
            self.jump_to_location(
                &path,
                loc.range.start.line as usize,
                loc.range.start.character as usize,
            )?;
            self.set_status_message(format!(
                "Jumped to {}:{}:{}",
                path.display(),
                loc.range.start.line + 1,
                loc.range.start.character + 1
            ));
        } else {
            self.set_status_message("No definition found".to_string());
        }

        Ok(())
    }

    /// Jump to a specific location in a file
    fn jump_to_location(&mut self, file_path: &Path, line: usize, column: usize) -> Result<()> {
        // If it's a different file, open it
        if self
            .buffer
            .file_path()
            .map(|p| p != file_path)
            .unwrap_or(true)
        {
            self.open_file(file_path)?;
        }

        // Move cursor to the position
        let cursor = self.buffer.cursor_mut();
        cursor.line = line;
        cursor.column = column;
        self.adjust_scroll();

        Ok(())
    }

    /// Navigate back in history
    pub fn navigate_back(&mut self) -> Result<()> {
        // Try to go back
        if let Some(prev) = self.navigation_history.back() {
            let prev = prev.clone(); // Clone to avoid borrow issues
            self.jump_to_location(&prev.path, prev.line, prev.column)?;
            self.set_status_message(format!(
                "Back to {}:{}:{}",
                prev.path.display(),
                prev.line + 1,
                prev.column + 1
            ));
        } else {
            self.set_status_message("No previous location in history".to_string());
        }

        Ok(())
    }

    /// Navigate forward in history
    pub fn navigate_forward(&mut self) -> Result<()> {
        // Try to go forward
        if let Some(next) = self.navigation_history.forward() {
            let next = next.clone(); // Clone to avoid borrow issues
            self.jump_to_location(&next.path, next.line, next.column)?;
            self.set_status_message(format!(
                "Forward to {}:{}:{}",
                next.path.display(),
                next.line + 1,
                next.column + 1
            ));
        } else {
            self.set_status_message("No next location in history".to_string());
        }

        Ok(())
    }

    /// Request code completion at current cursor position
    pub fn request_completion(&mut self, trigger_character: Option<String>) -> Result<()> {
        let language_id = file_type_to_language_id(self.buffer.file_type());
        if let Some(ref mut lsp) = self.lsp_manager {
            if let Some(file_path) = self.buffer.file_path() {
                if let Some(lang_id) = &language_id
                    && lsp.is_enabled(lang_id)
                {
                    let cursor = self.buffer.cursor();
                    let path_buf = file_path.to_path_buf();

                    // Set completion_start_column to the beginning of the current word
                    // This ensures we replace the entire word being typed when applying completion
                    self.completion_start_column =
                        self.buffer.word_start_column(cursor.line, cursor.column);

                    lsp.request_completion(
                        lang_id,
                        &path_buf,
                        cursor.line as u32,
                        cursor.column as u32,
                        trigger_character,
                    )
                    .map_err(|e| miette::miette!("Failed to request completion: {}", e))?;
                } else {
                    self.set_status_message("LSP not available for this file type".to_string());
                }
            } else {
                self.set_status_message("No file open".to_string());
            }
        } else {
            self.set_status_message("LSP not initialized".to_string());
        }
        Ok(())
    }

    /// Apply the selected completion item
    pub fn apply_completion(&mut self) -> Result<()> {
        if self.show_completion && !self.filtered_completion_items.is_empty() {
            let item = self.filtered_completion_items[self.completion_selected].clone();

            // Determine the text to insert (prefer insert_text over label)
            let insert_text = item.insert_text.as_ref().unwrap_or(&item.label).clone();

            // Delete the prefix that was typed since completion started (from completion_start_column to cursor)
            self.buffer.delete_range(self.completion_start_column);

            // Insert the completion text
            self.buffer.insert_str(&insert_text);

            // Close completion popup
            self.show_completion = false;
            self.completion_items.clear();
            self.filtered_completion_items.clear();

            // Notify LSP of document change
            self.notify_lsp_document_change();
        }
        Ok(())
    }

    /// Move completion selection up
    pub fn completion_up(&mut self) {
        if self.show_completion && !self.filtered_completion_items.is_empty() {
            if self.completion_selected > 0 {
                self.completion_selected -= 1;
            } else {
                self.completion_selected = self.filtered_completion_items.len() - 1;
            }
        }
    }

    /// Move completion selection down
    pub fn completion_down(&mut self) {
        if self.show_completion && !self.filtered_completion_items.is_empty() {
            if self.completion_selected < self.filtered_completion_items.len() - 1 {
                self.completion_selected += 1;
            } else {
                self.completion_selected = 0;
            }
        }
    }

    /// Cancel completion popup
    pub fn cancel_completion(&mut self) {
        self.show_completion = false;
        self.completion_items.clear();
        self.filtered_completion_items.clear();
        self.completion_selected = 0;
    }

    /// Check if quit dialog is visible
    pub fn show_quit_dialog(&self) -> bool {
        self.show_quit_dialog
    }

    /// Confirm quit (called when user confirms in dialog)
    pub fn confirm_quit(&mut self) {
        self.should_quit = true;
        self.show_quit_dialog = false;
    }

    /// Cancel quit dialog
    pub fn cancel_quit_dialog(&mut self) {
        self.show_quit_dialog = false;
        self.quit_confirm_pending = false;
    }

    /// Check if search dialog is visible
    pub fn show_search_dialog(&self) -> bool {
        self.show_search_dialog
    }

    /// Get search query
    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    /// Get replace query
    pub fn replace_query(&self) -> &str {
        &self.replace_query
    }

    /// Get search mode
    pub fn search_mode(&self) -> SearchMode {
        self.search_mode
    }

    /// Get search results count
    pub fn search_match_count(&self) -> usize {
        self.search_results.len()
    }

    /// Get current search index
    pub fn search_current_index(&self) -> Option<usize> {
        self.search_index
    }

    /// Get active search field
    pub fn search_active_field(&self) -> SearchField {
        self.search_active_field
    }

    /// Check if save-as dialog is visible
    pub fn show_save_as_dialog(&self) -> bool {
        self.show_save_as_dialog
    }

    /// Get save-as filename
    pub fn save_as_filename(&self) -> &str {
        &self.save_as_filename
    }

    /// Check if goto line dialog is visible
    pub fn show_goto_line_dialog(&self) -> bool {
        self.show_goto_line_dialog
    }

    /// Get goto line input
    pub fn goto_line_input(&self) -> &str {
        &self.goto_line_input
    }

    /// Open goto line dialog
    pub fn open_goto_line_dialog(&mut self) {
        self.show_goto_line_dialog = true;
        self.goto_line_input.clear();
    }

    /// Close goto line dialog
    pub fn close_goto_line_dialog(&mut self) {
        self.show_goto_line_dialog = false;
        self.goto_line_input.clear();
    }

    /// Confirm goto line with the entered line number
    pub fn confirm_goto_line(&mut self) -> Result<()> {
        if self.goto_line_input.is_empty() {
            return Err(miette::miette!("Line number cannot be empty"));
        }

        let line_number: usize = self
            .goto_line_input
            .trim()
            .parse()
            .map_err(|_| miette::miette!("Invalid line number"))?;

        if line_number == 0 {
            return Err(miette::miette!("Line number must be greater than 0"));
        }

        // Convert from 1-based to 0-based indexing
        let target_line = line_number.saturating_sub(1);
        let max_line = self.buffer.line_count().saturating_sub(1);

        // Clamp to valid range
        let target_line = target_line.min(max_line);

        // Move cursor to the line
        self.buffer.cursor_mut().line = target_line;
        self.buffer.cursor_mut().column = 0;
        self.adjust_scroll();

        self.set_status_message(format!("Jumped to line {}", line_number));
        self.close_goto_line_dialog();

        Ok(())
    }

    /// Check if mq query dialog is visible
    pub fn show_mq_query_dialog(&self) -> bool {
        self.show_mq_query_dialog
    }

    /// Get mq query input
    pub fn mq_query_input(&self) -> &str {
        &self.mq_query_input
    }

    /// Get mq query result
    pub fn mq_query_result(&self) -> Option<&str> {
        self.mq_query_result.as_deref()
    }

    /// Open mq query dialog
    pub fn open_mq_query_dialog(&mut self) {
        self.show_mq_query_dialog = true;
        self.mq_query_input.clear();
        self.mq_query_result = None;
    }

    /// Close mq query dialog
    pub fn close_mq_query_dialog(&mut self) {
        self.show_mq_query_dialog = false;
        self.mq_query_input.clear();
        self.mq_query_result = None;
    }

    /// Execute mq query against the current buffer content
    pub fn execute_mq_query(&mut self) {
        if self.mq_query_input.is_empty() {
            self.mq_query_result = Some("Error: Query is empty".to_string());
            return;
        }

        let content = self.buffer.content();
        let query = self.mq_query_input.clone();

        let mut engine = mq_lang::DefaultEngine::default();
        engine.load_builtin_module();

        let input = match mq_lang::parse_markdown_input(&content) {
            Ok(input) => input,
            Err(e) => {
                self.mq_query_result = Some(format!("Error: {}", e));
                return;
            }
        };

        match engine.eval(&query, input.into_iter()) {
            Ok(results) => {
                let output = results
                    .into_iter()
                    .filter_map(|v| {
                        if v.is_empty() {
                            None
                        } else {
                            Some(v.to_string())
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let start_line = self.buffer.cursor().line;
                let start_column = self.buffer.cursor().column;
                self.buffer.insert_str(&output);
                self.buffer.cursor_mut().line = start_line;
                self.buffer.cursor_mut().column = start_column;
                self.buffer.cursor_mut().update_desired_column();
                self.adjust_scroll();
                self.notify_lsp_document_change();
                self.set_status_message("mq query executed successfully.".to_string());
                self.close_mq_query_dialog();
            }
            Err(e) => {
                self.mq_query_result = Some(format!("Error: {}", e));
            }
        }
    }

    /// Open save-as dialog
    pub fn open_save_as_dialog(&mut self) {
        self.show_save_as_dialog = true;
        // Pre-fill with suggested name if no file path
        if self.buffer.file_path().is_none() {
            self.save_as_filename = "untitled".to_string();
        } else if let Some(path) = self.buffer.file_path() {
            self.save_as_filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("untitled")
                .to_string();
        }
    }

    /// Close save-as dialog
    pub fn close_save_as_dialog(&mut self) {
        self.show_save_as_dialog = false;
        self.save_as_filename.clear();
    }

    /// Confirm save-as with the entered filename
    pub fn confirm_save_as(&mut self) -> Result<()> {
        if self.save_as_filename.is_empty() {
            return Err(miette::miette!("Filename cannot be empty"));
        }

        // Construct the full path
        let path = if self.save_as_filename.contains('/') || self.save_as_filename.contains('\\') {
            // Absolute or relative path provided
            PathBuf::from(&self.save_as_filename)
        } else {
            // Just filename, save in current directory
            self.current_dir.join(&self.save_as_filename)
        };

        // Save the file
        self.buffer.save_as(&path)?;
        self.set_status_message(format!("Saved as: {}", path.display()));
        self.close_save_as_dialog();

        // Update file type based on new extension
        let file_type = FileType::from_path(&path);
        let language_id = file_type_to_language_id(&file_type);
        if let Some(lsp) = &mut self.lsp_manager
            && let Some(lang_id) = &language_id
            && lsp.is_enabled(lang_id)
        {
            // Notify LSP of the new file
            let content = self.buffer.content();
            if let Err(e) = lsp.did_open(lang_id, &path, &content) {
                eprintln!("LSP did_open error: {}", e);
            }
        }

        Ok(())
    }

    /// Open search dialog
    pub fn open_search(&mut self) {
        self.show_search_dialog = true;
        self.search_mode = SearchMode::Find;
        self.search_active_field = SearchField::Search;
    }

    /// Open replace dialog
    pub fn open_replace(&mut self) {
        self.show_search_dialog = true;
        self.search_mode = SearchMode::Replace;
        self.search_active_field = SearchField::Search;
    }

    /// Close search dialog
    pub fn close_search(&mut self) {
        self.show_search_dialog = false;
    }

    /// Update search results
    fn update_search_results(&mut self) {
        self.search_results = self.buffer.find_all(&self.search_query);
        if self.search_results.is_empty() {
            self.search_index = None;
        } else if self.search_index.is_none()
            || self.search_index.unwrap() >= self.search_results.len()
        {
            self.search_index = Some(0);
        }
    }

    /// Find next match
    pub fn search_next(&mut self) {
        if self.search_results.is_empty() {
            return;
        }
        if let Some(idx) = self.search_index {
            self.search_index = Some((idx + 1) % self.search_results.len());
        } else {
            self.search_index = Some(0);
        }
        self.jump_to_current_match();
    }

    /// Find previous match
    pub fn search_prev(&mut self) {
        if self.search_results.is_empty() {
            return;
        }
        if let Some(idx) = self.search_index {
            if idx == 0 {
                self.search_index = Some(self.search_results.len() - 1);
            } else {
                self.search_index = Some(idx - 1);
            }
        } else {
            self.search_index = Some(self.search_results.len() - 1);
        }
        self.jump_to_current_match();
    }

    /// Jump to current match
    fn jump_to_current_match(&mut self) {
        if let Some(idx) = self.search_index
            && let Some(&(line, column)) = self.search_results.get(idx)
        {
            self.buffer.cursor_mut().line = line;
            self.buffer.cursor_mut().column = column;
            self.adjust_scroll();
        }
    }

    /// Replace current match
    pub fn replace_current(&mut self) {
        if let Some(idx) = self.search_index
            && let Some(&(line, column)) = self.search_results.get(idx)
            && self
                .buffer
                .replace_at(line, column, &self.search_query, &self.replace_query)
        {
            self.notify_lsp_document_change();
            self.update_search_results();
            // Move to next match or stay at current position
            if !self.search_results.is_empty() {
                if idx >= self.search_results.len() {
                    self.search_index = Some(0);
                }
                self.jump_to_current_match();
            }
        }
    }

    /// Replace all matches
    pub fn replace_all(&mut self) {
        let count = self
            .buffer
            .replace_all(&self.search_query, &self.replace_query);
        if count > 0 {
            self.notify_lsp_document_change();
            self.set_status_message(format!("Replaced {} occurrences", count));
            self.update_search_results();
        }
    }

    /// Toggle between search and replace fields
    pub fn toggle_search_field(&mut self) {
        if self.search_mode == SearchMode::Replace {
            self.search_active_field = match self.search_active_field {
                SearchField::Search => SearchField::Replace,
                SearchField::Replace => SearchField::Search,
            };
        }
    }

    /// Handle keyboard input
    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        // Clear status message on any key press
        self.clear_status_message();

        // Handle quit confirmation dialog if visible
        if self.show_quit_dialog {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    self.confirm_quit();
                    return Ok(());
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.cancel_quit_dialog();
                    return Ok(());
                }
                _ => {
                    // Ignore other keys when dialog is open
                    return Ok(());
                }
            }
        }

        // Handle search dialog if visible
        if self.show_search_dialog {
            return self.handle_search_key(key);
        }

        // Handle save-as dialog if visible
        if self.show_save_as_dialog {
            return self.handle_save_as_key(key);
        }

        // Handle goto line dialog if visible
        if self.show_goto_line_dialog {
            return self.handle_goto_line_key(key);
        }

        // Handle mq query dialog if visible
        if self.show_mq_query_dialog {
            return self.handle_mq_query_key(key);
        }

        // Handle completion popup if visible
        if self.show_completion {
            match key.code {
                KeyCode::Up => {
                    self.completion_up();
                    return Ok(());
                }
                KeyCode::Down => {
                    self.completion_down();
                    return Ok(());
                }
                KeyCode::Enter => {
                    return self.apply_completion();
                }
                KeyCode::Esc => {
                    self.cancel_completion();
                    return Ok(());
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Continue typing while completion is open - filter will be updated
                    self.buffer.insert_char(c);
                    self.notify_lsp_document_change();
                    self.filter_completion_items();
                    return Ok(());
                }
                KeyCode::Backspace => {
                    // Handle backspace while completion is open
                    let cursor = self.buffer.cursor();
                    if cursor.column <= self.completion_start_column {
                        // Backspace before completion start - close completion
                        self.cancel_completion();
                    }
                    self.buffer.delete_char();
                    self.notify_lsp_document_change();
                    if self.show_completion {
                        self.filter_completion_items();
                    }
                    return Ok(());
                }
                _ => {
                    // Any other key closes completion
                    self.cancel_completion();
                    // Continue to normal key handling below
                }
            }
        }

        // Toggle file browser
        if self.config.keybindings.toggle_file_browser.matches(&key)
            || self
                .config
                .keybindings
                .toggle_file_browser_alt
                .matches(&key)
        {
            self.toggle_file_browser();
            return Ok(());
        }

        // Handle file browser navigation when visible
        if self.show_file_browser {
            return self.handle_file_browser_key(key);
        }

        // Check configured keybindings first
        if self.config.keybindings.quit.matches(&key)
            || self.config.keybindings.quit_alt.matches(&key)
        {
            if self.buffer.is_modified() && !self.pipe_mode {
                // Show quit confirmation dialog
                self.show_quit_dialog = true;
                self.set_status_message("Unsaved changes! Save before quitting?".to_string());
            } else {
                self.should_quit = true;
            }
            return Ok(());
        }

        // Reset quit confirmation if any other key is pressed
        self.quit_confirm_pending = false;

        if self.config.keybindings.save.matches(&key) {
            // Check if file has a path
            if self.buffer.file_path().is_none() {
                // No file path, open save-as dialog
                self.open_save_as_dialog();
            } else {
                // Has file path, save directly
                if let Err(e) = self.buffer.save() {
                    self.set_status_message(format!("Error saving file: {}", e));
                } else {
                    self.set_status_message("File saved successfully.".to_string());
                }
            }
            return Ok(());
        }

        // LSP navigation - go to definition
        if self.config.keybindings.goto_definition.matches(&key) {
            return self.request_go_to_definition();
        }

        // Navigation history - back and forward
        if self.config.keybindings.navigate_back.matches(&key) {
            return self.navigate_back();
        }
        if self.config.keybindings.navigate_forward.matches(&key) {
            return self.navigate_forward();
        }

        // Toggle line numbers
        if self.config.keybindings.toggle_line_numbers.matches(&key) {
            self.toggle_line_numbers();
            return Ok(());
        }

        // Toggle current line highlight
        if self
            .config
            .keybindings
            .toggle_current_line_highlight
            .matches(&key)
        {
            self.toggle_current_line_highlight();
            return Ok(());
        }

        // Search
        if self.config.keybindings.search.matches(&key) {
            self.open_search();
            return Ok(());
        }

        // Replace
        if self.config.keybindings.replace.matches(&key) {
            self.open_replace();
            return Ok(());
        }

        // Go to line
        if self.config.keybindings.goto_line.matches(&key) {
            self.open_goto_line_dialog();
            return Ok(());
        }

        // Execute mq query
        if self.config.keybindings.execute_mq_query.matches(&key) {
            self.open_mq_query_dialog();
            return Ok(());
        }

        // Undo
        if self.config.keybindings.undo.matches(&key) {
            self.buffer.undo();
            self.notify_lsp_document_change();
            self.adjust_scroll();
            return Ok(());
        }

        // Redo
        if self.config.keybindings.redo.matches(&key) {
            self.buffer.redo();
            self.notify_lsp_document_change();
            self.adjust_scroll();
            return Ok(());
        }

        // Code completion - Ctrl+Space
        if key.code == KeyCode::Char(' ') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return self.request_completion(None);
        }

        // Navigation and editing keys (not configurable)
        match key.code {
            // Navigation
            KeyCode::Up => {
                self.buffer.move_cursor(CursorMovement::Up);
                self.adjust_scroll();
            }
            KeyCode::Down => {
                self.buffer.move_cursor(CursorMovement::Down);
                self.adjust_scroll();
            }
            KeyCode::Left => {
                self.buffer.move_cursor(CursorMovement::Left);
            }
            KeyCode::Right => {
                self.buffer.move_cursor(CursorMovement::Right);
            }
            KeyCode::Home => {
                self.buffer.move_cursor(CursorMovement::StartOfLine);
            }
            KeyCode::End => {
                self.buffer.move_cursor(CursorMovement::EndOfLine);
            }
            KeyCode::PageUp => {
                self.buffer.move_cursor(CursorMovement::PageUp);
                self.adjust_scroll();
            }
            KeyCode::PageDown => {
                self.buffer.move_cursor(CursorMovement::PageDown);
                self.adjust_scroll();
            }

            // Editing
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.buffer.insert_char(c);
                self.notify_lsp_document_change();
                // Check if the character is a trigger character for completion
                let language_id = file_type_to_language_id(self.buffer.file_type());
                if let Some(ref lsp) = self.lsp_manager
                    && let Some(lang_id) = &language_id
                    && lsp.is_trigger_character(lang_id, c)
                {
                    // Trigger completion with the character
                    let _ = self.request_completion(Some(c.to_string()));
                }
            }
            KeyCode::Enter => {
                self.buffer.insert_newline();
                self.adjust_scroll();
                self.notify_lsp_document_change();
            }
            KeyCode::Backspace => {
                self.buffer.delete_char();
                self.notify_lsp_document_change();
            }
            KeyCode::Tab => {
                self.buffer.insert_char('\t');
                self.notify_lsp_document_change();
            }

            _ => {}
        }

        Ok(())
    }

    /// Handle paste event (used for IME input and clipboard paste)
    pub fn handle_paste(&mut self, text: String) -> Result<()> {
        // Don't handle paste when file browser is visible
        if self.show_file_browser {
            return Ok(());
        }

        // Insert the pasted text at cursor position
        self.buffer.insert_str(&text);
        self.adjust_scroll();
        self.notify_lsp_document_change();

        Ok(())
    }

    /// Adjust scroll offset to keep cursor visible
    fn adjust_scroll(&mut self) {
        let cursor_line = self.buffer.cursor().line;
        let viewport_height = 20; // TODO: Get actual terminal height

        // Scroll down if cursor is below viewport
        if cursor_line >= self.scroll_offset + viewport_height {
            self.scroll_offset = cursor_line - viewport_height + 1;
        }

        // Scroll up if cursor is above viewport
        if cursor_line < self.scroll_offset {
            self.scroll_offset = cursor_line;
        }
    }

    /// Handle keyboard input when file browser is visible
    fn handle_file_browser_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            // Navigation in file browser
            KeyCode::Up => {
                if let Some(tree) = self.file_tree_mut() {
                    tree.move_up();
                }
            }
            KeyCode::Down => {
                if let Some(tree) = self.file_tree_mut() {
                    tree.move_down();
                }
            }
            KeyCode::Enter => {
                // Open selected file or toggle directory
                if let Some(tree) = self.file_tree_mut()
                    && let Some(item) = tree.selected_item()
                {
                    if item.is_dir {
                        tree.toggle_expand();
                    } else {
                        // Open the file
                        let path = item.path.clone();
                        let _ = tree; // Release borrow
                        if let Err(e) = self.open_file(&path) {
                            self.set_status_message(format!("Error opening file: {}", e));
                        } else {
                            self.show_file_browser = false;
                            self.set_status_message(format!("Opened: {}", path.display()));
                        }
                    }
                }
            }
            KeyCode::Left => {
                // Collapse directory
                if let Some(tree) = self.file_tree_mut()
                    && let Some(item) = tree.selected_item()
                    && item.is_dir
                    && item.expanded
                {
                    tree.toggle_expand();
                }
            }
            KeyCode::Right => {
                // Expand directory
                if let Some(tree) = self.file_tree_mut()
                    && let Some(item) = tree.selected_item()
                    && item.is_dir
                    && !item.expanded
                {
                    tree.toggle_expand();
                }
            }
            _ if self.config.keybindings.close_browser.matches(&key) => {
                // Close file browser
                self.show_file_browser = false;
            }
            _ => {}
        }

        Ok(())
    }

    /// Handle keyboard input when search dialog is visible
    fn handle_search_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.close_search();
            }
            KeyCode::Enter => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.search_prev();
                } else {
                    self.search_next();
                }
            }
            KeyCode::Tab => {
                self.toggle_search_field();
            }
            KeyCode::Backspace => match self.search_active_field {
                SearchField::Search => {
                    self.search_query.pop();
                    self.update_search_results();
                }
                SearchField::Replace => {
                    self.replace_query.pop();
                }
            },
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+R: Replace current
                if self.search_mode == SearchMode::Replace {
                    self.replace_current();
                }
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+A: Replace all
                if self.search_mode == SearchMode::Replace {
                    self.replace_all();
                }
            }
            KeyCode::Char(c) => {
                match self.search_active_field {
                    SearchField::Search => {
                        self.search_query.push(c);
                        self.update_search_results();
                        // Auto-jump to first match
                        if !self.search_results.is_empty() {
                            self.search_index = Some(0);
                            self.jump_to_current_match();
                        }
                    }
                    SearchField::Replace => {
                        self.replace_query.push(c);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle keyboard input when save-as dialog is visible
    fn handle_save_as_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.close_save_as_dialog();
            }
            KeyCode::Enter => {
                if let Err(e) = self.confirm_save_as() {
                    self.set_status_message(format!("Error saving file: {}", e));
                    self.close_save_as_dialog();
                }
            }
            KeyCode::Backspace => {
                self.save_as_filename.pop();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.save_as_filename.push(c);
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle keyboard input when mq query dialog is visible
    fn handle_mq_query_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.close_mq_query_dialog();
            }
            KeyCode::Enter => {
                self.execute_mq_query();
            }
            KeyCode::Backspace => {
                self.mq_query_input.pop();
                self.mq_query_result = None;
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.mq_query_input.push(c);
                self.mq_query_result = None;
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle keyboard input when goto line dialog is visible
    fn handle_goto_line_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.close_goto_line_dialog();
            }
            KeyCode::Enter => {
                if let Err(e) = self.confirm_goto_line() {
                    self.set_status_message(format!("{}", e));
                }
            }
            KeyCode::Backspace => {
                self.goto_line_input.pop();
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                self.goto_line_input.push(c);
            }
            _ => {}
        }
        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert FileType to language ID string for LSP
fn file_type_to_language_id(file_type: &FileType) -> Option<String> {
    match file_type {
        FileType::Code(lang_id) => Some(lang_id.clone()),
        FileType::Markdown => Some("markdown".to_string()),
        FileType::PlainText => None,
    }
}
