use crate::document::{CursorMovement, DocumentBuffer};

/// Navigation operations for the document buffer
pub struct NavigationOperations;

impl NavigationOperations {
    /// Move cursor in the specified direction
    pub fn move_cursor(buffer: &mut DocumentBuffer, movement: CursorMovement) {
        buffer.move_cursor(movement);
    }
}
