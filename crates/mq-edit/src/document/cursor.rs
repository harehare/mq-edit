use mq_markdown::{Markdown, Node};

/// Cursor position in the editor (0-indexed)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub line: usize,
    pub column: usize,
    /// Desired column for vertical movement (sticky column)
    pub desired_column: usize,
}

impl Cursor {
    pub fn new() -> Self {
        Self {
            line: 0,
            column: 0,
            desired_column: 0,
        }
    }

    pub fn with_position(line: usize, column: usize) -> Self {
        Self {
            line,
            column,
            desired_column: column,
        }
    }

    /// Update desired column when moving horizontally
    pub fn update_desired_column(&mut self) {
        self.desired_column = self.column;
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new()
    }
}

/// Cursor movement directions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorMovement {
    Up,
    Down,
    Left,
    Right,
    StartOfLine,
    EndOfLine,
    PageUp,
    PageDown,
    StartOfDocument,
    EndOfDocument,
}

/// Maps visual line numbers to AST nodes
#[derive(Debug, Clone)]
pub struct LineMap {
    entries: Vec<LineEntry>,
}

#[derive(Debug, Clone)]
pub struct LineEntry {
    /// Index in the Markdown.nodes array
    pub node_index: usize,
    /// Line offset within a multi-line node
    pub node_line_offset: usize,
    /// Visual line number in the editor (0-indexed)
    pub visual_line: usize,
    /// Starting character offset from the beginning of the node
    pub char_offset: usize,
}

impl LineMap {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Build LineMap from Markdown AST
    pub fn from_markdown(markdown: &Markdown) -> Self {
        let mut map = Self::new();
        let mut current_line = 0;

        for (node_idx, node) in markdown.nodes.iter().enumerate() {
            Self::process_node(&mut map, node, node_idx, &mut current_line);
        }

        map
    }

    fn process_node(map: &mut LineMap, node: &Node, node_idx: usize, current_line: &mut usize) {
        if let Some(pos) = node.position() {
            let start_line = pos.start.line.saturating_sub(1); // Convert to 0-indexed
            let end_line = pos.end.line.saturating_sub(1);

            for line_offset in 0..=(end_line - start_line) {
                map.entries.push(LineEntry {
                    node_index: node_idx,
                    node_line_offset: line_offset,
                    visual_line: *current_line,
                    char_offset: 0,
                });
                *current_line += 1;
            }
        } else {
            // Node without position, allocate one line
            map.entries.push(LineEntry {
                node_index: node_idx,
                node_line_offset: 0,
                visual_line: *current_line,
                char_offset: 0,
            });
            *current_line += 1;
        }
    }

    /// Get the node index at a given visual line
    pub fn get_entry(&self, line: usize) -> Option<&LineEntry> {
        self.entries.get(line)
    }

    /// Get node at a given visual line
    pub fn get_node_at_line<'a>(&self, markdown: &'a Markdown, line: usize) -> Option<&'a Node> {
        let entry = self.get_entry(line)?;
        markdown.nodes.get(entry.node_index)
    }

    /// Total number of visual lines
    pub fn line_count(&self) -> usize {
        self.entries.len()
    }

    /// Invalidate entries from a given line onwards (for incremental updates)
    pub fn invalidate_from(&mut self, from_line: usize) {
        self.entries.truncate(from_line);
    }

    /// Rebuild from a specific line in the Markdown AST
    pub fn rebuild_from(&mut self, markdown: &Markdown, from_node_idx: usize) {
        let mut current_line = if let Some(entry) = self.entries.last() {
            entry.visual_line + 1
        } else {
            0
        };

        for (idx, node) in markdown.nodes.iter().enumerate().skip(from_node_idx) {
            Self::process_node(self, node, idx, &mut current_line);
        }
    }
}

impl Default for LineMap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_creation() {
        let cursor = Cursor::new();
        assert_eq!(cursor.line, 0);
        assert_eq!(cursor.column, 0);
        assert_eq!(cursor.desired_column, 0);
    }

    #[test]
    fn test_cursor_with_position() {
        let cursor = Cursor::with_position(5, 10);
        assert_eq!(cursor.line, 5);
        assert_eq!(cursor.column, 10);
        assert_eq!(cursor.desired_column, 10);
    }

    #[test]
    fn test_linemap_creation() {
        let linemap = LineMap::new();
        assert_eq!(linemap.line_count(), 0);
    }
}
