use crate::document::DocumentBuffer;

/// Editing operations for the document buffer
pub struct EditingOperations;

impl EditingOperations {
    /// Insert character at current cursor position
    pub fn insert_char(buffer: &mut DocumentBuffer, c: char) {
        buffer.insert_char(c);
    }

    /// Delete character before cursor (backspace)
    pub fn delete_char(buffer: &mut DocumentBuffer) {
        buffer.delete_char();
    }

    /// Insert newline at cursor
    pub fn insert_newline(buffer: &mut DocumentBuffer) {
        buffer.insert_newline();
    }
}
