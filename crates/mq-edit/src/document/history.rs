use super::Cursor;

/// Represents a single edit action that can be undone/redone
#[derive(Debug, Clone)]
pub enum EditAction {
    /// A character was inserted at a position
    InsertChar { line: usize, column: usize, c: char },
    /// A string was inserted at a position
    InsertStr {
        line: usize,
        column: usize,
        text: String,
    },
    /// A newline was inserted at a position (line split)
    InsertNewline { line: usize, column: usize },
    /// A character was deleted within a line (backspace)
    DeleteChar {
        line: usize,
        column: usize,
        deleted: char,
    },
    /// Lines were joined by backspace at column 0
    JoinLines {
        /// The line that was joined into the previous line
        line: usize,
        /// Column position in the previous line where join happened
        column: usize,
    },
    /// A range of characters was deleted on a line
    DeleteRange {
        line: usize,
        start_col: usize,
        deleted: String,
    },
    /// Text was replaced at a specific position
    ReplaceAt {
        line: usize,
        column: usize,
        old_text: String,
        new_text: String,
    },
}

/// Entry in the history stack, pairing an action with the cursor state before it
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub action: EditAction,
    pub cursor_before: Cursor,
}

/// Manages undo/redo history for edit operations
#[derive(Debug, Clone)]
pub struct EditHistory {
    undo_stack: Vec<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>,
    max_history: usize,
}

impl EditHistory {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_history: 1000,
        }
    }

    /// Record a new edit action, clearing the redo stack
    pub fn push(&mut self, action: EditAction, cursor_before: Cursor) {
        self.redo_stack.clear();
        self.undo_stack.push(HistoryEntry {
            action,
            cursor_before,
        });
        if self.undo_stack.len() > self.max_history {
            self.undo_stack.remove(0);
        }
    }

    /// Pop the last action from undo stack and move to redo stack
    pub fn undo(&mut self) -> Option<HistoryEntry> {
        let entry = self.undo_stack.pop()?;
        self.redo_stack.push(entry.clone());
        Some(entry)
    }

    /// Pop the last action from redo stack and move to undo stack
    pub fn redo(&mut self) -> Option<HistoryEntry> {
        let entry = self.redo_stack.pop()?;
        self.undo_stack.push(entry.clone());
        Some(entry)
    }
}

impl Default for EditHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_undo() {
        let mut history = EditHistory::new();
        let cursor = Cursor::new();

        history.push(
            EditAction::InsertChar {
                line: 0,
                column: 0,
                c: 'a',
            },
            cursor,
        );

        let entry = history.undo().unwrap();
        match entry.action {
            EditAction::InsertChar { c, .. } => assert_eq!(c, 'a'),
            _ => panic!("Expected InsertChar"),
        }
    }

    #[test]
    fn test_undo_empty() {
        let mut history = EditHistory::new();
        assert!(history.undo().is_none());
    }

    #[test]
    fn test_redo_after_undo() {
        let mut history = EditHistory::new();
        let cursor = Cursor::new();

        history.push(
            EditAction::InsertChar {
                line: 0,
                column: 0,
                c: 'x',
            },
            cursor,
        );

        history.undo();
        let entry = history.redo().unwrap();
        match entry.action {
            EditAction::InsertChar { c, .. } => assert_eq!(c, 'x'),
            _ => panic!("Expected InsertChar"),
        }
    }

    #[test]
    fn test_new_action_clears_redo() {
        let mut history = EditHistory::new();
        let cursor = Cursor::new();

        history.push(
            EditAction::InsertChar {
                line: 0,
                column: 0,
                c: 'a',
            },
            cursor,
        );
        history.undo();

        // Push new action should clear redo
        history.push(
            EditAction::InsertChar {
                line: 0,
                column: 0,
                c: 'b',
            },
            cursor,
        );

        assert!(history.redo().is_none());
    }

    #[test]
    fn test_max_history() {
        let mut history = EditHistory::new();
        history.max_history = 3;
        let cursor = Cursor::new();

        for i in 0..5 {
            history.push(
                EditAction::InsertChar {
                    line: 0,
                    column: i,
                    c: 'a',
                },
                cursor,
            );
        }

        assert_eq!(history.undo_stack.len(), 3);
    }
}
