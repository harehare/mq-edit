use std::path::{Path, PathBuf};

use miette::Result;
use mq_markdown::{Markdown, Node};
use unicode_width::UnicodeWidthChar;

use super::history::{EditAction, EditHistory};
use super::{Cursor, CursorMovement, DocumentType, FileType, LineMap};

/// Document buffer that manages content editing for any file type
#[derive(Debug, Clone)]
pub struct DocumentBuffer {
    /// Document-specific data (AST for Markdown, language info for Code)
    document_type: DocumentType,
    /// File type classification
    file_type: FileType,
    /// Path to the file (if loaded from disk)
    file_path: Option<PathBuf>,
    /// Current cursor position
    cursor: Cursor,
    /// Content as lines (cached for performance)
    lines: Vec<String>,
    /// Whether the buffer has been modified
    modified: bool,
    /// Edit history for undo/redo
    history: EditHistory,
    /// Flag to suppress history recording during undo/redo operations
    recording: bool,
}

impl DocumentBuffer {
    /// Create a new empty buffer (defaults to Markdown)
    pub fn new() -> Self {
        let document_type = DocumentType::new_markdown("").unwrap_or_else(|_| {
            // Create empty markdown if parsing fails
            DocumentType::new_markdown(" ").expect("Failed to create empty markdown")
        });

        Self {
            document_type,
            file_type: FileType::Markdown,
            file_path: None,
            cursor: Cursor::new(),
            lines: vec![String::new()],
            modified: false,
            history: EditHistory::new(),
            recording: true,
        }
    }

    /// Create buffer from file
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .map_err(|e| miette::miette!("Failed to read file: {}", e))?;

        // Detect file type from extension
        let file_type = FileType::from_path(path);

        // Create appropriate document type
        let document_type = match &file_type {
            FileType::Markdown => DocumentType::new_markdown(&content)?,
            FileType::Code(lang) => DocumentType::new_code(lang.clone()),
            FileType::PlainText => DocumentType::new_plain_text(),
        };

        let lines = Self::extract_lines(&content);

        Ok(Self {
            document_type,
            file_type,
            file_path: Some(path.to_path_buf()),
            cursor: Cursor::new(),
            lines,
            modified: false,
            history: EditHistory::new(),
            recording: true,
        })
    }

    /// Create buffer from string (defaults to Markdown)
    pub fn from_string(content: &str) -> Result<Self> {
        let document_type = DocumentType::new_markdown(content)?;
        let lines = Self::extract_lines(content);

        Ok(Self {
            document_type,
            file_type: FileType::Markdown,
            file_path: None,
            cursor: Cursor::new(),
            lines,
            modified: false,
            history: EditHistory::new(),
            recording: true,
        })
    }

    /// Extract lines from content
    fn extract_lines(content: &str) -> Vec<String> {
        if content.is_empty() {
            vec![String::new()]
        } else {
            content.lines().map(|s| s.to_string()).collect()
        }
    }

    /// Get the current cursor position
    pub fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    /// Get mutable cursor
    pub fn cursor_mut(&mut self) -> &mut Cursor {
        &mut self.cursor
    }

    /// Get the file path
    pub fn file_path(&self) -> Option<&Path> {
        self.file_path.as_deref()
    }

    /// Check if buffer has been modified
    pub fn is_modified(&self) -> bool {
        self.modified
    }

    /// Get the file type
    pub fn file_type(&self) -> &FileType {
        &self.file_type
    }

    /// Get the document type
    pub fn document_type(&self) -> &DocumentType {
        &self.document_type
    }

    /// Get the underlying Markdown AST (returns None if not a Markdown document)
    pub fn markdown(&self) -> Option<&Markdown> {
        self.document_type.markdown_ast()
    }

    /// Get the LineMap (returns None if not a Markdown document)
    pub fn line_map(&self) -> Option<&LineMap> {
        self.document_type.line_map()
    }

    /// Get all lines
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Get a specific line
    pub fn line(&self, index: usize) -> Option<&str> {
        self.lines.get(index).map(|s| s.as_str())
    }

    /// Get total line count
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Get the node at current cursor position (only for Markdown documents)
    pub fn node_at_cursor(&self) -> Option<&Node> {
        match &self.document_type {
            DocumentType::Markdown { ast, line_map } => {
                line_map.get_node_at_line(ast, self.cursor.line)
            }
            _ => None,
        }
    }

    /// Convert character position to byte position for a given line
    /// Returns the byte offset in the line string for the given character position
    fn char_to_byte_idx(&self, line_idx: usize, char_pos: usize) -> usize {
        if let Some(line) = self.line(line_idx) {
            line.char_indices()
                .nth(char_pos)
                .map(|(byte_idx, _)| byte_idx)
                .unwrap_or(line.len())
        } else {
            0
        }
    }

    /// Get the character count (not byte count) for a line
    fn line_char_count(&self, line_idx: usize) -> usize {
        self.line(line_idx).map(|s| s.chars().count()).unwrap_or(0)
    }

    /// Move cursor
    pub fn move_cursor(&mut self, movement: CursorMovement) {
        match movement {
            CursorMovement::Up => {
                if self.cursor.line > 0 {
                    self.cursor.line -= 1;
                    self.clamp_cursor_column();
                }
            }
            CursorMovement::Down => {
                if self.cursor.line + 1 < self.line_count() {
                    self.cursor.line += 1;
                    self.clamp_cursor_column();
                }
            }
            CursorMovement::Left => {
                if self.cursor.column > 0 {
                    self.cursor.column -= 1;
                    self.cursor.update_desired_column();
                } else if self.cursor.line > 0 {
                    // Move to end of previous line
                    self.cursor.line -= 1;
                    self.cursor.column = self.line_char_count(self.cursor.line);
                    self.cursor.update_desired_column();
                }
            }
            CursorMovement::Right => {
                let line_len = self.line_char_count(self.cursor.line);
                if self.cursor.column < line_len {
                    self.cursor.column += 1;
                    self.cursor.update_desired_column();
                } else if self.cursor.line + 1 < self.line_count() {
                    // Move to start of next line
                    self.cursor.line += 1;
                    self.cursor.column = 0;
                    self.cursor.update_desired_column();
                }
            }
            CursorMovement::StartOfLine => {
                self.cursor.column = 0;
                self.cursor.update_desired_column();
            }
            CursorMovement::EndOfLine => {
                self.cursor.column = self.line_char_count(self.cursor.line);
                self.cursor.update_desired_column();
            }
            CursorMovement::PageUp => {
                self.cursor.line = self.cursor.line.saturating_sub(20);
                self.clamp_cursor_column();
            }
            CursorMovement::PageDown => {
                self.cursor.line = (self.cursor.line + 20).min(self.line_count().saturating_sub(1));
                self.clamp_cursor_column();
            }
            CursorMovement::StartOfDocument => {
                self.cursor.line = 0;
                self.cursor.column = 0;
                self.cursor.update_desired_column();
            }
            CursorMovement::EndOfDocument => {
                self.cursor.line = self.line_count().saturating_sub(1);
                self.cursor.column = self.line_char_count(self.cursor.line);
                self.cursor.update_desired_column();
            }
        }
    }

    /// Clamp cursor column to line length (preserving desired column for up/down movement)
    fn clamp_cursor_column(&mut self) {
        let line_len = self.line_char_count(self.cursor.line);
        self.cursor.column = self.cursor.desired_column.min(line_len);
    }

    /// Insert a character at cursor position
    pub fn insert_char(&mut self, c: char) {
        if self.recording {
            let cursor_before = self.cursor;
            self.history.push(
                EditAction::InsertChar {
                    line: self.cursor.line,
                    column: self.cursor.column,
                    c,
                },
                cursor_before,
            );
        }
        let byte_idx = self.char_to_byte_idx(self.cursor.line, self.cursor.column);
        if let Some(line) = self.lines.get_mut(self.cursor.line) {
            line.insert(byte_idx, c);
            self.cursor.column += 1;
            self.cursor.update_desired_column();
            self.modified = true;
            self.rebuild_document();
        }
    }

    /// Insert a string at cursor position (useful for IME/paste operations)
    pub fn insert_str(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }

        if self.recording {
            let cursor_before = self.cursor;
            self.history.push(
                EditAction::InsertStr {
                    line: self.cursor.line,
                    column: self.cursor.column,
                    text: s.to_string(),
                },
                cursor_before,
            );
        }

        // Split the string by newlines
        let lines_to_insert: Vec<&str> = s.split('\n').collect();

        if lines_to_insert.len() == 1 {
            // Single line insertion (most common for IME)
            let byte_idx = self.char_to_byte_idx(self.cursor.line, self.cursor.column);
            if let Some(line) = self.lines.get_mut(self.cursor.line) {
                line.insert_str(byte_idx, s);
                // Update cursor by character count, not byte count
                self.cursor.column += s.chars().count();
                self.cursor.update_desired_column();
                self.modified = true;
                self.rebuild_document();
            }
        } else {
            // Multi-line insertion
            let byte_idx = self.char_to_byte_idx(self.cursor.line, self.cursor.column);
            if let Some(current_line) = self.lines.get_mut(self.cursor.line) {
                // Split current line at cursor
                let rest = current_line.split_off(byte_idx);

                // Append first line of paste to current line
                current_line.push_str(lines_to_insert[0]);

                // Insert middle lines
                for (i, &line) in lines_to_insert[1..lines_to_insert.len() - 1]
                    .iter()
                    .enumerate()
                {
                    self.lines
                        .insert(self.cursor.line + 1 + i, line.to_string());
                }

                // Insert last line and append rest of original line
                let last_line_idx = self.cursor.line + lines_to_insert.len() - 1;
                let mut last_line = lines_to_insert.last().unwrap().to_string();
                let new_cursor_column = last_line.chars().count();
                last_line.push_str(&rest);
                self.lines.insert(last_line_idx, last_line);

                // Update cursor position (use character count)
                self.cursor.line = last_line_idx;
                self.cursor.column = new_cursor_column;
                self.cursor.update_desired_column();
                self.modified = true;
                self.rebuild_document();
            }
        }
    }

    /// Delete a range of characters on the current line (from start_col to current cursor position)
    /// Moves cursor to start_col after deletion
    pub fn delete_range(&mut self, start_col: usize) {
        let end_col = self.cursor.column;
        if start_col >= end_col {
            return;
        }

        let line_idx = self.cursor.line;
        if line_idx >= self.lines.len() {
            return;
        }

        // Calculate byte indices first (before mutable borrow)
        let start_byte = self.char_to_byte_idx(line_idx, start_col);
        let end_byte = self.char_to_byte_idx(line_idx, end_col);

        if end_byte <= self.lines[line_idx].len() {
            if self.recording {
                let cursor_before = self.cursor;
                let deleted = self.lines[line_idx][start_byte..end_byte].to_string();
                self.history.push(
                    EditAction::DeleteRange {
                        line: line_idx,
                        start_col,
                        deleted,
                    },
                    cursor_before,
                );
            }
            self.lines[line_idx].replace_range(start_byte..end_byte, "");
            self.cursor.column = start_col;
            self.cursor.update_desired_column();
            self.modified = true;
            self.rebuild_document();
        }
    }

    /// Delete character at cursor (backspace)
    pub fn delete_char(&mut self) {
        if self.cursor.column > 0 {
            let cursor_before = self.cursor;
            self.cursor.column -= 1;
            let byte_idx = self.char_to_byte_idx(self.cursor.line, self.cursor.column);
            if let Some(line) = self.lines.get_mut(self.cursor.line) {
                // Find the character at byte_idx and remove it
                if let Some((_, ch)) = line[byte_idx..].char_indices().next() {
                    if self.recording {
                        self.history.push(
                            EditAction::DeleteChar {
                                line: self.cursor.line,
                                column: self.cursor.column,
                                deleted: ch,
                            },
                            cursor_before,
                        );
                    }
                    line.replace_range(byte_idx..byte_idx + ch.len_utf8(), "");
                }
                self.cursor.update_desired_column();
                self.modified = true;
                self.rebuild_document();
            }
        } else if self.cursor.line > 0 {
            if self.recording {
                let cursor_before = self.cursor;
                self.history.push(
                    EditAction::JoinLines {
                        line: self.cursor.line,
                        column: self.lines[self.cursor.line - 1].chars().count(),
                    },
                    cursor_before,
                );
            }
            // Join with previous line
            let current_line = self.lines.remove(self.cursor.line);
            self.cursor.line -= 1;
            // Use character count, not byte length
            self.cursor.column = self.lines[self.cursor.line].chars().count();
            self.lines[self.cursor.line].push_str(&current_line);
            self.cursor.update_desired_column();
            self.modified = true;
            self.rebuild_document();
        }
    }

    /// Insert newline at cursor
    pub fn insert_newline(&mut self) {
        if self.recording {
            let cursor_before = self.cursor;
            self.history.push(
                EditAction::InsertNewline {
                    line: self.cursor.line,
                    column: self.cursor.column,
                },
                cursor_before,
            );
        }
        let byte_idx = self.char_to_byte_idx(self.cursor.line, self.cursor.column);
        if let Some(line) = self.lines.get_mut(self.cursor.line) {
            let rest = line.split_off(byte_idx);
            self.cursor.line += 1;
            self.lines.insert(self.cursor.line, rest);
            self.cursor.column = 0;
            self.cursor.update_desired_column();
            self.modified = true;
            self.rebuild_document();
        }
    }

    /// Rebuild document-specific structures from current lines content
    fn rebuild_document(&mut self) {
        let content = self.lines.join("\n");
        // Rebuild document type (for Markdown, this reparses the AST)
        let _ = self.document_type.rebuild(&content);
        // If rebuilding fails, keep the old document state (better than crashing)
    }

    /// Undo the last edit action
    pub fn undo(&mut self) {
        if let Some(entry) = self.history.undo() {
            self.recording = false;
            self.apply_reverse(&entry.action);
            self.cursor = entry.cursor_before;
            self.cursor.update_desired_column();
            self.modified = true;
            self.rebuild_document();
            self.recording = true;
        }
    }

    /// Redo the last undone edit action
    pub fn redo(&mut self) {
        if let Some(entry) = self.history.redo() {
            self.recording = false;
            self.apply_forward(&entry.action);
            self.modified = true;
            self.rebuild_document();
            self.recording = true;
        }
    }

    /// Apply the reverse of an edit action (for undo)
    fn apply_reverse(&mut self, action: &EditAction) {
        match action {
            EditAction::InsertChar { line, column, .. } => {
                // Reverse of insert char: delete the char at that position
                let byte_idx = self.char_to_byte_idx(*line, *column);
                if let Some(line_content) = self.lines.get_mut(*line)
                    && let Some((_, ch)) = line_content[byte_idx..].char_indices().next()
                {
                    line_content.replace_range(byte_idx..byte_idx + ch.len_utf8(), "");
                }
            }
            EditAction::InsertStr { line, column, text } => {
                // Reverse of insert str: remove the inserted text
                let inserted_lines: Vec<&str> = text.split('\n').collect();
                if inserted_lines.len() == 1 {
                    let byte_idx = self.char_to_byte_idx(*line, *column);
                    if let Some(line_content) = self.lines.get_mut(*line) {
                        let end_byte = byte_idx + text.len();
                        if end_byte <= line_content.len() {
                            line_content.replace_range(byte_idx..end_byte, "");
                        }
                    }
                } else {
                    // Multi-line: reconstruct the original line
                    let first_line_byte = self.char_to_byte_idx(*line, *column);
                    let last_inserted_line_idx = *line + inserted_lines.len() - 1;
                    let last_inserted_chars = inserted_lines.last().unwrap().chars().count();
                    let last_line_rest_byte =
                        self.char_to_byte_idx(last_inserted_line_idx, last_inserted_chars);

                    // Get the part before insertion on the first line
                    let before = self.lines[*line][..first_line_byte].to_string();
                    // Get the part after insertion on the last line
                    let after =
                        self.lines[last_inserted_line_idx][last_line_rest_byte..].to_string();

                    // Remove the inserted lines (keep the first line)
                    for _ in 0..inserted_lines.len() - 1 {
                        if *line + 1 < self.lines.len() {
                            self.lines.remove(*line + 1);
                        }
                    }

                    // Reconstruct the original line
                    self.lines[*line] = format!("{}{}", before, after);
                }
            }
            EditAction::InsertNewline { line, column } => {
                // Reverse of insert newline: join line+1 back into line at column
                if *line + 1 < self.lines.len() {
                    let next_line = self.lines.remove(*line + 1);
                    let byte_idx = self.char_to_byte_idx(*line, *column);
                    if let Some(line_content) = self.lines.get_mut(*line) {
                        // Truncate at column and append next line
                        line_content.truncate(byte_idx);
                        line_content.push_str(&next_line);
                    }
                }
            }
            EditAction::DeleteChar {
                line,
                column,
                deleted,
            } => {
                // Reverse of delete char: re-insert the deleted char
                let byte_idx = self.char_to_byte_idx(*line, *column);
                if let Some(line_content) = self.lines.get_mut(*line) {
                    line_content.insert(byte_idx, *deleted);
                }
            }
            EditAction::JoinLines { line, column } => {
                // Reverse of join lines: split the line back
                let byte_idx = self.char_to_byte_idx(*line - 1, *column);
                if let Some(prev_line) = self.lines.get_mut(*line - 1) {
                    let rest = prev_line.split_off(byte_idx);
                    self.lines.insert(*line, rest);
                }
            }
            EditAction::DeleteRange {
                line,
                start_col,
                deleted,
            } => {
                // Reverse of delete range: re-insert the deleted text
                let byte_idx = self.char_to_byte_idx(*line, *start_col);
                if let Some(line_content) = self.lines.get_mut(*line) {
                    line_content.insert_str(byte_idx, deleted);
                }
            }
            EditAction::ReplaceAt {
                line,
                column,
                old_text,
                new_text,
            } => {
                // Reverse of replace: replace new_text back with old_text
                let byte_idx = self.char_to_byte_idx(*line, *column);
                if let Some(line_content) = self.lines.get_mut(*line) {
                    let end_byte = byte_idx + new_text.len();
                    if end_byte <= line_content.len() {
                        line_content.replace_range(byte_idx..end_byte, old_text);
                    }
                }
            }
        }
    }

    /// Apply an edit action forward (for redo)
    fn apply_forward(&mut self, action: &EditAction) {
        match action {
            EditAction::InsertChar { line, column, c } => {
                let byte_idx = self.char_to_byte_idx(*line, *column);
                if let Some(line_content) = self.lines.get_mut(*line) {
                    line_content.insert(byte_idx, *c);
                }
                self.cursor.line = *line;
                self.cursor.column = *column + 1;
                self.cursor.update_desired_column();
            }
            EditAction::InsertStr { line, column, text } => {
                // Re-apply the insert_str logic
                let inserted_lines: Vec<&str> = text.split('\n').collect();
                if inserted_lines.len() == 1 {
                    let byte_idx = self.char_to_byte_idx(*line, *column);
                    if let Some(line_content) = self.lines.get_mut(*line) {
                        line_content.insert_str(byte_idx, text);
                    }
                    self.cursor.line = *line;
                    self.cursor.column = *column + text.chars().count();
                } else {
                    let byte_idx = self.char_to_byte_idx(*line, *column);
                    if let Some(current_line) = self.lines.get_mut(*line) {
                        let rest = current_line.split_off(byte_idx);
                        current_line.push_str(inserted_lines[0]);

                        for (i, &il) in inserted_lines[1..inserted_lines.len() - 1]
                            .iter()
                            .enumerate()
                        {
                            self.lines.insert(*line + 1 + i, il.to_string());
                        }

                        let last_line_idx = *line + inserted_lines.len() - 1;
                        let mut last_line = inserted_lines.last().unwrap().to_string();
                        let new_cursor_column = last_line.chars().count();
                        last_line.push_str(&rest);
                        self.lines.insert(last_line_idx, last_line);

                        self.cursor.line = last_line_idx;
                        self.cursor.column = new_cursor_column;
                    }
                }
                self.cursor.update_desired_column();
            }
            EditAction::InsertNewline { line, column } => {
                let byte_idx = self.char_to_byte_idx(*line, *column);
                if let Some(line_content) = self.lines.get_mut(*line) {
                    let rest = line_content.split_off(byte_idx);
                    self.lines.insert(*line + 1, rest);
                }
                self.cursor.line = *line + 1;
                self.cursor.column = 0;
                self.cursor.update_desired_column();
            }
            EditAction::DeleteChar {
                line,
                column,
                deleted,
            } => {
                let byte_idx = self.char_to_byte_idx(*line, *column);
                if let Some(line_content) = self.lines.get_mut(*line) {
                    line_content.replace_range(byte_idx..byte_idx + deleted.len_utf8(), "");
                }
                self.cursor.line = *line;
                self.cursor.column = *column;
                self.cursor.update_desired_column();
            }
            EditAction::JoinLines { line, column } => {
                let current_line = self.lines.remove(*line);
                self.lines[*line - 1].push_str(&current_line);
                self.cursor.line = *line - 1;
                self.cursor.column = *column;
                self.cursor.update_desired_column();
            }
            EditAction::DeleteRange {
                line,
                start_col,
                deleted,
            } => {
                let start_byte = self.char_to_byte_idx(*line, *start_col);
                let end_byte = start_byte + deleted.len();
                if end_byte <= self.lines[*line].len() {
                    self.lines[*line].replace_range(start_byte..end_byte, "");
                }
                self.cursor.line = *line;
                self.cursor.column = *start_col;
                self.cursor.update_desired_column();
            }
            EditAction::ReplaceAt {
                line,
                column,
                old_text,
                new_text,
            } => {
                let byte_idx = self.char_to_byte_idx(*line, *column);
                if let Some(line_content) = self.lines.get_mut(*line) {
                    let end_byte = byte_idx + old_text.len();
                    if end_byte <= line_content.len() {
                        line_content.replace_range(byte_idx..end_byte, new_text);
                    }
                }
            }
        }
    }

    /// Save buffer to file
    pub fn save(&mut self) -> Result<()> {
        if let Some(path) = &self.file_path {
            let content = self.lines.join("\n");
            std::fs::write(path, content)
                .map_err(|e| miette::miette!("Failed to write file: {}", e))?;
            self.modified = false;
            Ok(())
        } else {
            Err(miette::miette!("No file path set"))
        }
    }

    /// Save buffer to a specific file
    pub fn save_as(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let content = self.lines.join("\n");
        std::fs::write(path.as_ref(), content)
            .map_err(|e| miette::miette!("Failed to write file: {}", e))?;
        self.file_path = Some(path.as_ref().to_path_buf());
        self.modified = false;
        Ok(())
    }

    /// Get buffer content as string
    pub fn content(&self) -> String {
        self.lines.join("\n")
    }

    /// Get the start column of the word at cursor position
    /// This is useful for code completion to determine how much text to replace
    pub fn word_start_column(&self, line: usize, column: usize) -> usize {
        if let Some(line_content) = self.line(line) {
            let chars: Vec<char> = line_content.chars().collect();
            if column == 0 || chars.is_empty() {
                return column;
            }

            // Search backwards from current position to find word start
            let mut start = column.min(chars.len());
            while start > 0 {
                let c = chars[start - 1];
                // Word characters: alphanumeric and underscore
                if c.is_alphanumeric() || c == '_' {
                    start -= 1;
                } else {
                    break;
                }
            }
            start
        } else {
            column
        }
    }

    /// Calculate the display width from the start of a line to a given column position
    /// This accounts for wide characters (e.g., CJK characters, emoji) that take 2 columns
    pub fn display_width_to_column(&self, line: usize, column: usize) -> usize {
        if let Some(line_content) = self.line(line) {
            line_content
                .chars()
                .take(column)
                .map(|c| c.width().unwrap_or(0))
                .sum()
        } else {
            0
        }
    }

    /// Find all occurrences of a query string in the buffer
    /// Returns a list of (line, column) positions (0-indexed)
    pub fn find_all(&self, query: &str) -> Vec<(usize, usize)> {
        if query.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();
        for (line_idx, line) in self.lines.iter().enumerate() {
            let mut search_start = 0;
            while let Some(byte_pos) = line[search_start..].find(query) {
                let abs_byte_pos = search_start + byte_pos;
                // Convert byte position to character position
                let char_pos = line[..abs_byte_pos].chars().count();
                results.push((line_idx, char_pos));
                // Move past this match
                search_start = abs_byte_pos + query.len();
            }
        }
        results
    }

    /// Find the next occurrence of a query string starting from a position
    /// Returns the (line, column) position or None if not found
    pub fn find_next(
        &self,
        query: &str,
        from_line: usize,
        from_column: usize,
    ) -> Option<(usize, usize)> {
        if query.is_empty() {
            return None;
        }

        // Search from current line
        for (line_idx, line) in self.lines.iter().enumerate().skip(from_line) {
            let start_col = if line_idx == from_line {
                // Start after current column position
                self.char_to_byte_idx(line_idx, from_column + 1)
            } else {
                0
            };

            if start_col < line.len()
                && let Some(byte_pos) = line[start_col..].find(query)
            {
                let abs_byte_pos = start_col + byte_pos;
                let char_pos = line[..abs_byte_pos].chars().count();
                return Some((line_idx, char_pos));
            }
        }

        // Wrap around to beginning
        for (line_idx, line) in self.lines.iter().enumerate().take(from_line + 1) {
            let end_col = if line_idx == from_line {
                self.char_to_byte_idx(line_idx, from_column)
            } else {
                line.len()
            };

            if let Some(byte_pos) = line[..end_col].find(query) {
                let char_pos = line[..byte_pos].chars().count();
                return Some((line_idx, char_pos));
            }
        }

        None
    }

    /// Find the previous occurrence of a query string starting from a position
    /// Returns the (line, column) position or None if not found
    pub fn find_prev(
        &self,
        query: &str,
        from_line: usize,
        from_column: usize,
    ) -> Option<(usize, usize)> {
        if query.is_empty() {
            return None;
        }

        // Search backwards from current line
        for line_idx in (0..=from_line).rev() {
            let line = &self.lines[line_idx];
            let end_col = if line_idx == from_line {
                self.char_to_byte_idx(line_idx, from_column)
            } else {
                line.len()
            };

            // Find the last occurrence before end_col
            if let Some(byte_pos) = line[..end_col].rfind(query) {
                let char_pos = line[..byte_pos].chars().count();
                return Some((line_idx, char_pos));
            }
        }

        // Wrap around to end
        for line_idx in (from_line..self.lines.len()).rev() {
            let line = &self.lines[line_idx];
            let start_col = if line_idx == from_line {
                self.char_to_byte_idx(line_idx, from_column + 1)
            } else {
                0
            };

            if start_col < line.len()
                && let Some(byte_pos) = line[start_col..].rfind(query)
            {
                let abs_byte_pos = start_col + byte_pos;
                let char_pos = line[..abs_byte_pos].chars().count();
                return Some((line_idx, char_pos));
            }
        }

        None
    }

    /// Replace text at a specific position
    /// Returns true if replacement was successful
    pub fn replace_at(
        &mut self,
        line: usize,
        column: usize,
        old_text: &str,
        new_text: &str,
    ) -> bool {
        if line >= self.lines.len() {
            return false;
        }

        let byte_idx = self.char_to_byte_idx(line, column);
        let line_content = &self.lines[line];

        // Verify the old text exists at this position
        if byte_idx + old_text.len() > line_content.len() {
            return false;
        }
        if &line_content[byte_idx..byte_idx + old_text.len()] != old_text {
            return false;
        }

        if self.recording {
            let cursor_before = self.cursor;
            self.history.push(
                EditAction::ReplaceAt {
                    line,
                    column,
                    old_text: old_text.to_string(),
                    new_text: new_text.to_string(),
                },
                cursor_before,
            );
        }

        // Perform the replacement
        let new_line = format!(
            "{}{}{}",
            &line_content[..byte_idx],
            new_text,
            &line_content[byte_idx + old_text.len()..]
        );
        self.lines[line] = new_line;
        self.modified = true;
        self.rebuild_document();
        true
    }

    /// Replace all occurrences of old_text with new_text
    /// Returns the number of replacements made
    pub fn replace_all(&mut self, old_text: &str, new_text: &str) -> usize {
        if old_text.is_empty() {
            return 0;
        }

        let mut count = 0;
        for line_idx in 0..self.lines.len() {
            let line = &self.lines[line_idx];
            if line.contains(old_text) {
                let new_line = line.replace(old_text, new_text);
                count += line.matches(old_text).count();
                self.lines[line_idx] = new_line;
            }
        }

        if count > 0 {
            self.modified = true;
            self.rebuild_document();
        }

        count
    }
}

impl Default for DocumentBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_buffer() {
        let buffer = DocumentBuffer::new();
        assert_eq!(buffer.line_count(), 1);
        assert_eq!(buffer.cursor().line, 0);
        assert_eq!(buffer.cursor().column, 0);
        assert!(!buffer.is_modified());
    }

    #[test]
    fn test_from_string() {
        let buffer = DocumentBuffer::from_string("# Hello\n\nWorld").unwrap();
        assert_eq!(buffer.line_count(), 3);
        assert_eq!(buffer.line(0), Some("# Hello"));
        assert_eq!(buffer.line(1), Some(""));
        assert_eq!(buffer.line(2), Some("World"));
    }

    #[test]
    fn test_cursor_movement() {
        let mut buffer = DocumentBuffer::from_string("Line 1\nLine 2\nLine 3").unwrap();

        buffer.move_cursor(CursorMovement::Down);
        assert_eq!(buffer.cursor().line, 1);

        buffer.move_cursor(CursorMovement::Right);
        assert_eq!(buffer.cursor().column, 1);

        buffer.move_cursor(CursorMovement::EndOfLine);
        assert_eq!(buffer.cursor().column, 6); // "Line 2".len()
    }

    #[test]
    fn test_insert_char() {
        let mut buffer = DocumentBuffer::from_string("Hello").unwrap();
        buffer.cursor_mut().column = 5;
        buffer.insert_char('!');

        assert_eq!(buffer.line(0), Some("Hello!"));
        assert_eq!(buffer.cursor().column, 6);
        assert!(buffer.is_modified());
    }

    #[test]
    fn test_delete_char() {
        let mut buffer = DocumentBuffer::from_string("Hello!").unwrap();
        buffer.cursor_mut().column = 6;
        buffer.delete_char();

        assert_eq!(buffer.line(0), Some("Hello"));
        assert_eq!(buffer.cursor().column, 5);
        assert!(buffer.is_modified());
    }

    #[test]
    fn test_insert_newline() {
        let mut buffer = DocumentBuffer::from_string("Hello").unwrap();
        buffer.cursor_mut().column = 2;
        buffer.insert_newline();

        assert_eq!(buffer.line_count(), 2);
        assert_eq!(buffer.line(0), Some("He"));
        assert_eq!(buffer.line(1), Some("llo"));
        assert_eq!(buffer.cursor().line, 1);
        assert_eq!(buffer.cursor().column, 0);
    }

    #[test]
    fn test_insert_str_single_line() {
        let mut buffer = DocumentBuffer::from_string("Hello").unwrap();
        buffer.cursor_mut().column = 5;
        buffer.insert_str(" World");

        assert_eq!(buffer.line(0), Some("Hello World"));
        assert_eq!(buffer.cursor().column, 11);
        assert!(buffer.is_modified());
    }

    #[test]
    fn test_insert_str_empty() {
        let mut buffer = DocumentBuffer::from_string("Hello").unwrap();
        buffer.cursor_mut().column = 5;
        buffer.insert_str("");

        assert_eq!(buffer.line(0), Some("Hello"));
        assert_eq!(buffer.cursor().column, 5);
    }

    #[test]
    fn test_insert_str_multi_line() {
        let mut buffer = DocumentBuffer::from_string("Hello").unwrap();
        buffer.cursor_mut().column = 2;
        buffer.insert_str("XXX\nYYY\nZZZ");

        assert_eq!(buffer.line_count(), 3);
        assert_eq!(buffer.line(0), Some("HeXXX"));
        assert_eq!(buffer.line(1), Some("YYY"));
        assert_eq!(buffer.line(2), Some("ZZZllo"));
        assert_eq!(buffer.cursor().line, 2);
        assert_eq!(buffer.cursor().column, 3);
        assert!(buffer.is_modified());
    }

    #[test]
    fn test_insert_str_japanese() {
        let mut buffer = DocumentBuffer::from_string("Hello").unwrap();
        buffer.cursor_mut().column = 5;
        buffer.insert_str("こんにちは");

        assert_eq!(buffer.line(0), Some("Helloこんにちは"));
        assert_eq!(buffer.cursor().column, 10); // 5 + "こんにちは".chars().count()
        assert!(buffer.is_modified());
    }

    #[test]
    fn test_insert_char_japanese() {
        let mut buffer = DocumentBuffer::from_string("Hello").unwrap();
        buffer.cursor_mut().column = 5;
        buffer.insert_char('あ');
        buffer.insert_char('い');

        assert_eq!(buffer.line(0), Some("Helloあい"));
        assert_eq!(buffer.cursor().column, 7); // 5 + 2
        assert!(buffer.is_modified());
    }

    #[test]
    fn test_delete_char_japanese() {
        let mut buffer = DocumentBuffer::from_string("こんにちは世界").unwrap();
        buffer.cursor_mut().column = 7; // After "世界"
        buffer.delete_char();

        assert_eq!(buffer.line(0), Some("こんにちは世"));
        assert_eq!(buffer.cursor().column, 6);
    }

    #[test]
    fn test_cursor_movement_japanese() {
        let mut buffer = DocumentBuffer::from_string("こんにちは").unwrap();

        // Move to end
        buffer.move_cursor(CursorMovement::EndOfLine);
        assert_eq!(buffer.cursor().column, 5);

        // Move left through Japanese characters
        buffer.move_cursor(CursorMovement::Left);
        assert_eq!(buffer.cursor().column, 4);
        buffer.move_cursor(CursorMovement::Left);
        assert_eq!(buffer.cursor().column, 3);
    }

    #[test]
    fn test_insert_str_mixed_content() {
        let mut buffer = DocumentBuffer::from_string("Hello世界").unwrap();
        buffer.cursor_mut().column = 5; // After "Hello"
        buffer.insert_str("ありがとう");

        assert_eq!(buffer.line(0), Some("Helloありがとう世界"));
        assert_eq!(buffer.cursor().column, 10); // 5 + 5
    }

    #[test]
    fn test_word_start_column() {
        let buffer = DocumentBuffer::from_string("hello world").unwrap();
        // In the middle of "hello"
        assert_eq!(buffer.word_start_column(0, 3), 0);
        // At the end of "hello"
        assert_eq!(buffer.word_start_column(0, 5), 0);
        // At the space
        assert_eq!(buffer.word_start_column(0, 6), 6);
        // In the middle of "world"
        assert_eq!(buffer.word_start_column(0, 8), 6);
        // At the end of "world"
        assert_eq!(buffer.word_start_column(0, 11), 6);
    }

    #[test]
    fn test_word_start_column_with_symbols() {
        let buffer = DocumentBuffer::from_string("self.method()").unwrap();
        // After "self"
        assert_eq!(buffer.word_start_column(0, 4), 0);
        // At the dot
        assert_eq!(buffer.word_start_column(0, 5), 5);
        // In "method"
        assert_eq!(buffer.word_start_column(0, 8), 5);
        // After "method"
        assert_eq!(buffer.word_start_column(0, 11), 5);
    }

    #[test]
    fn test_word_start_column_with_underscore() {
        let buffer = DocumentBuffer::from_string("my_variable = 10").unwrap();
        // In "my_variable"
        assert_eq!(buffer.word_start_column(0, 5), 0);
        assert_eq!(buffer.word_start_column(0, 11), 0);
    }

    #[test]
    fn test_word_start_column_at_start() {
        let buffer = DocumentBuffer::from_string("hello").unwrap();
        assert_eq!(buffer.word_start_column(0, 0), 0);
    }

    #[test]
    fn test_display_width_ascii() {
        let buffer = DocumentBuffer::from_string("hello world").unwrap();
        // ASCII characters have width 1
        assert_eq!(buffer.display_width_to_column(0, 0), 0);
        assert_eq!(buffer.display_width_to_column(0, 5), 5);
        assert_eq!(buffer.display_width_to_column(0, 11), 11);
    }

    #[test]
    fn test_display_width_japanese() {
        let buffer = DocumentBuffer::from_string("こんにちは").unwrap();
        // Japanese characters typically have width 2
        assert_eq!(buffer.display_width_to_column(0, 0), 0);
        assert_eq!(buffer.display_width_to_column(0, 1), 2); // After 'こ'
        assert_eq!(buffer.display_width_to_column(0, 2), 4); // After 'ん'
        assert_eq!(buffer.display_width_to_column(0, 5), 10); // After 'は'
    }

    #[test]
    fn test_display_width_mixed() {
        let buffer = DocumentBuffer::from_string("Hello世界").unwrap();
        // 'H' 'e' 'l' 'l' 'o' = 5 columns, '世' '界' = 4 columns
        assert_eq!(buffer.display_width_to_column(0, 5), 5); // After "Hello"
        assert_eq!(buffer.display_width_to_column(0, 6), 7); // After "Hello世"
        assert_eq!(buffer.display_width_to_column(0, 7), 9); // After "Hello世界"
    }
}
