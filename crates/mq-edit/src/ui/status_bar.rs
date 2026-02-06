use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};
use unicode_width::UnicodeWidthStr;

use crate::document::DocumentBuffer;
use md_lsp::DiagnosticsManager;

/// Status bar widget
pub struct StatusBar<'a> {
    buffer: &'a DocumentBuffer,
    diagnostics: Option<&'a DiagnosticsManager>,
    warning_message: Option<&'a str>,
}

impl<'a> StatusBar<'a> {
    pub fn new(buffer: &'a DocumentBuffer) -> Self {
        Self {
            buffer,
            diagnostics: None,
            warning_message: None,
        }
    }

    pub fn with_diagnostics(mut self, diagnostics: &'a DiagnosticsManager) -> Self {
        self.diagnostics = Some(diagnostics);
        self
    }

    pub fn with_warning(mut self, message: &'a str) -> Self {
        self.warning_message = Some(message);
        self
    }
}

impl Widget for StatusBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // If there's a warning message, show warning-styled status bar
        if let Some(warning) = self.warning_message {
            let warning_line = Line::from(vec![Span::styled(
                format!(" ⚠ {} ", warning),
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]);
            let paragraph = Paragraph::new(warning_line).block(Block::default().bg(Color::Yellow));
            paragraph.render(area, buf);
            return;
        }

        let file_name = self
            .buffer
            .file_path()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("[No Name]");

        let modified = if self.buffer.is_modified() {
            " [+]"
        } else {
            ""
        };

        let cursor = self.buffer.cursor();
        let position = format!(" Ln {}, Col {} ", cursor.line + 1, cursor.column + 1);

        let line_count = format!(" {} lines ", self.buffer.line_count());

        // Get diagnostic counts if available
        let (error_count, warning_count) = if let Some(diagnostics) = self.diagnostics {
            (diagnostics.error_count(), diagnostics.warning_count())
        } else {
            (0, 0)
        };

        let separator = "│";

        let file_section = format!(" {}{} ", file_name, modified);

        let mut spans = vec![Span::styled(
            file_section.clone(),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )];

        // Build diagnostic spans
        let error_text = if error_count > 0 {
            format!(" ● {} ", error_count)
        } else {
            String::new()
        };
        let warning_text = if warning_count > 0 {
            format!(" ▲ {} ", warning_count)
        } else {
            String::new()
        };

        // Calculate remaining space using Unicode display width
        let separator_width = separator.width();
        let diagnostics_width = error_text.width() + warning_text.width();
        let has_diagnostics = error_count > 0 || warning_count > 0;
        let diagnostics_sep_width = if has_diagnostics { separator_width } else { 0 };

        let used_width = file_section.width()
            + position.width()
            + line_count.width()
            + diagnostics_width
            + diagnostics_sep_width
            + separator_width;
        let padding = (area.width as usize).saturating_sub(used_width);

        spans.push(Span::styled(
            " ".repeat(padding),
            Style::default().bg(Color::DarkGray),
        ));

        // Add diagnostics with color coding
        if error_count > 0 {
            spans.push(Span::styled(
                error_text,
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        if warning_count > 0 {
            spans.push(Span::styled(
                warning_text,
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        if has_diagnostics {
            spans.push(Span::styled(
                separator,
                Style::default().fg(Color::Gray).bg(Color::DarkGray),
            ));
        }

        spans.push(Span::styled(
            line_count,
            Style::default().fg(Color::White).bg(Color::DarkGray),
        ));

        spans.push(Span::styled(
            separator,
            Style::default().fg(Color::Gray).bg(Color::DarkGray),
        ));

        spans.push(Span::styled(
            position,
            Style::default().fg(Color::Black).bg(Color::Cyan),
        ));

        let status_line = Line::from(spans);

        let paragraph = Paragraph::new(status_line).block(Block::default().bg(Color::DarkGray));
        paragraph.render(area, buf);
    }
}
