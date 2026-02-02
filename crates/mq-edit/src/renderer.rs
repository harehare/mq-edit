pub mod code;
pub mod image_manager;
pub mod markdown;
pub mod plain_text;

use crate::document::DocumentBuffer;
pub use code::{CodeRenderer, SemanticToken, decode_semantic_tokens};
pub use image_manager::ImageManager;
pub use markdown::MarkdownRenderer;
pub use plain_text::PlainTextRenderer;
use ratatui::text::Span;

/// Trait for rendering document lines
///
/// Implementors of this trait can provide custom rendering logic for different
/// file types (Markdown, Code, PlainText, etc.).
pub trait Renderer {
    /// Render a single line
    ///
    /// # Parameters
    /// - `buffer`: The document buffer containing the content
    /// - `line_idx`: The zero-based line index to render
    /// - `is_current_line`: Whether this is the cursor line (affects rendering)
    ///
    /// # Returns
    /// Vector of styled text spans representing the rendered line
    fn render_line(
        &self,
        buffer: &DocumentBuffer,
        line_idx: usize,
        is_current_line: bool,
    ) -> Vec<Span<'_>>;

    /// Check if this renderer supports WYSIWYG mode
    ///
    /// WYSIWYG (What You See Is What You Get) mode shows:
    /// - Current line (cursor line): raw source for editing
    /// - Other lines: rich formatted output
    ///
    /// Returns `true` if the renderer provides different rendering for
    /// cursor vs non-cursor lines, `false` if all lines are rendered the same.
    fn supports_wysiwyg(&self) -> bool {
        false
    }
}
