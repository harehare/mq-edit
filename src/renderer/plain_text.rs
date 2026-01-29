use ratatui::{style::Style, text::Span};

use crate::document::DocumentBuffer;
use super::Renderer;

/// Plain text renderer (no formatting)
///
/// This renderer simply displays text as-is without any syntax highlighting
/// or formatting. Useful for text files, unknown file types, or when the
/// user wants to view raw content.
pub struct PlainTextRenderer;

impl PlainTextRenderer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PlainTextRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer for PlainTextRenderer {
    fn render_line(
        &self,
        buffer: &DocumentBuffer,
        line_idx: usize,
        _is_current_line: bool,
    ) -> Vec<Span<'_>> {
        let content = buffer.line(line_idx).unwrap_or("");
        vec![Span::styled(content.to_string(), Style::default())]
    }

    fn supports_wysiwyg(&self) -> bool {
        false // All lines rendered the same way
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_text_renderer() {
        let renderer = PlainTextRenderer::new();
        assert!(!renderer.supports_wysiwyg());
    }
}
