use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::document::{DocumentBuffer, LineAnalyzer, LineType, TableAlignment};
use crate::renderer::{CodeRenderer, ImageManager, MarkdownRenderer, Renderer};
use mq_lsp::DiagnosticsManager;

/// Table rendering context
struct TableContext {
    /// Column widths (calculated from all rows)
    column_widths: Vec<usize>,
    /// Column alignments from separator row
    alignments: Vec<TableAlignment>,
    /// Start line of current table
    start_line: usize,
    /// Line indices that belong to current table
    table_lines: Vec<usize>,
}

/// Editor widget for rendering the document
pub struct EditorWidget<'a> {
    buffer: &'a DocumentBuffer,
    scroll_offset: usize,
    markdown_renderer: MarkdownRenderer,
    code_renderer: Option<&'a CodeRenderer>,
    image_manager: Option<&'a ImageManager>,
    diagnostics: Option<&'a DiagnosticsManager>,
    show_line_numbers: bool,
    show_current_line_highlight: bool,
}

impl<'a> EditorWidget<'a> {
    pub fn new(buffer: &'a DocumentBuffer) -> Self {
        Self {
            buffer,
            scroll_offset: 0,
            markdown_renderer: MarkdownRenderer::new(),
            code_renderer: None,
            image_manager: None,
            diagnostics: None,
            show_line_numbers: true,
            show_current_line_highlight: true,
        }
    }

    pub fn with_line_numbers(mut self, show: bool) -> Self {
        self.show_line_numbers = show;
        self
    }

    pub fn with_current_line_highlight(mut self, show: bool) -> Self {
        self.show_current_line_highlight = show;
        self
    }

    pub fn with_scroll(mut self, offset: usize) -> Self {
        self.scroll_offset = offset;
        self
    }

    pub fn with_code_renderer(mut self, renderer: &'a CodeRenderer) -> Self {
        self.code_renderer = Some(renderer);
        self
    }

    pub fn with_image_manager(mut self, image_manager: &'a ImageManager) -> Self {
        self.image_manager = Some(image_manager);
        self
    }

    pub fn with_diagnostics(mut self, diagnostics: &'a DiagnosticsManager) -> Self {
        self.diagnostics = Some(diagnostics);
        self
    }

    /// Calculate visible line range based on viewport
    fn visible_range(&self, height: usize) -> (usize, usize) {
        let start = self.scroll_offset;
        let end = (start + height).min(self.buffer.line_count());
        (start, end)
    }

    /// Add diagnostic marker to a line if there are diagnostics
    fn add_diagnostic_marker<'b>(
        &self,
        mut spans: Vec<Span<'b>>,
        line_idx: usize,
    ) -> Vec<Span<'b>> {
        if let Some(diagnostics) = self.diagnostics
            && let Some(diagnostic) = diagnostics.most_severe_for_line(line_idx)
        {
            let marker = match diagnostic.severity {
                Some(lsp_types::DiagnosticSeverity::ERROR) => {
                    Span::styled(" ❌".to_string(), Style::default().fg(Color::Red))
                }
                Some(lsp_types::DiagnosticSeverity::WARNING) => {
                    Span::styled(" ⚠️ ".to_string(), Style::default().fg(Color::Yellow))
                }
                Some(lsp_types::DiagnosticSeverity::INFORMATION) => {
                    Span::styled(" ℹ️ ".to_string(), Style::default().fg(Color::Blue))
                }
                Some(lsp_types::DiagnosticSeverity::HINT) => {
                    Span::styled(" 💡".to_string(), Style::default().fg(Color::Cyan))
                }
                _ => Span::styled(" ⚠️ ".to_string(), Style::default().fg(Color::Yellow)),
            };
            spans.push(marker);
        }
        spans
    }

    /// Create a line number span
    fn make_line_number_span(
        &self,
        line_idx: usize,
        width: usize,
        is_current: bool,
    ) -> Span<'static> {
        let line_num = line_idx + 1; // 1-indexed display
        let formatted = format!("{:>width$} │ ", line_num, width = width);
        let style = if is_current {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        Span::styled(formatted, style)
    }

    /// Calculate the width needed for line numbers
    fn line_number_width(&self) -> usize {
        let total_lines = self.buffer.line_count();
        // Calculate digits needed for the largest line number
        if total_lines == 0 {
            1
        } else {
            ((total_lines as f64).log10().floor() as usize) + 1
        }
        .max(3) // Minimum width of 3
    }

    /// Pre-scan lines to identify tables and calculate column widths
    fn scan_tables(&self, start: usize, end: usize) -> Vec<TableContext> {
        let mut tables = Vec::new();
        let mut current_table: Option<TableContext> = None;

        for line_idx in start..end {
            let content = self.buffer.line(line_idx).unwrap_or("");

            if LineAnalyzer::is_table_row(content) {
                if current_table.is_none() {
                    // Start new potential table
                    let ctx = TableContext {
                        column_widths: Vec::new(),
                        alignments: Vec::new(),
                        start_line: line_idx,
                        table_lines: Vec::new(),
                    };
                    current_table = Some(ctx);
                }

                if let Some(ref mut ctx) = current_table {
                    ctx.table_lines.push(line_idx);

                    if LineAnalyzer::is_table_separator(content) {
                        ctx.alignments = LineAnalyzer::parse_table_alignment(content);
                    } else {
                        // Update column widths
                        let cells = LineAnalyzer::parse_table_cells(content);
                        while ctx.column_widths.len() < cells.len() {
                            ctx.column_widths.push(0);
                        }
                        for (i, cell) in cells.iter().enumerate() {
                            ctx.column_widths[i] = ctx.column_widths[i].max(cell.chars().count());
                        }
                    }
                }
            } else {
                // End of table
                if let Some(ctx) = current_table.take() {
                    // Validate: table must have header + separator (at least 2 lines)
                    if ctx.table_lines.len() >= 2 && !ctx.alignments.is_empty() {
                        tables.push(ctx);
                    }
                }
            }
        }

        // Don't forget table at end of range
        if let Some(ctx) = current_table
            && ctx.table_lines.len() >= 2
            && !ctx.alignments.is_empty()
        {
            tables.push(ctx);
        }

        tables
    }

    /// Get table context for a specific line
    fn get_table_context(line_idx: usize, tables: &[TableContext]) -> Option<&TableContext> {
        tables.iter().find(|t| t.table_lines.contains(&line_idx))
    }
}

impl Widget for EditorWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let (start, end) = self.visible_range(area.height as usize);
        let cursor_line = self.buffer.cursor().line;
        let line_num_width = self.line_number_width();

        // Determine which renderer to use based on file type
        let use_code_renderer = matches!(
            self.buffer.document_type(),
            crate::document::DocumentType::Code { .. }
        ) && self.code_renderer.is_some();

        let mut lines = Vec::new();

        if use_code_renderer {
            // Use CodeRenderer for code files
            let code_renderer = self.code_renderer.unwrap();

            for line_idx in start..end {
                let is_current = line_idx == cursor_line;

                let mut spans = Vec::new();

                // Add line number if enabled
                if self.show_line_numbers {
                    spans.push(self.make_line_number_span(line_idx, line_num_width, is_current));
                }

                spans.extend(code_renderer.render_line(self.buffer, line_idx, is_current));

                // Add diagnostic marker
                spans = self.add_diagnostic_marker(spans, line_idx);

                if is_current && self.show_current_line_highlight {
                    lines.push(Line::from(spans).style(Style::default().bg(Color::DarkGray)));
                } else {
                    lines.push(Line::from(spans));
                }
            }
        } else {
            // Use MarkdownRenderer for markdown files
            let mut in_code_block = false;

            // Pre-scan for tables
            let tables = self.scan_tables(start, end);

            for line_idx in start..end {
                let content = self.buffer.line(line_idx).unwrap_or("");
                let is_current = line_idx == cursor_line;
                let trimmed = content.trim();

                // Check if this line is a code fence
                let is_code_fence = trimmed.starts_with("```");

                // Create base spans with optional line number
                let mut base_spans = Vec::new();
                if self.show_line_numbers {
                    base_spans.push(self.make_line_number_span(
                        line_idx,
                        line_num_width,
                        is_current,
                    ));
                }

                if is_code_fence {
                    if !in_code_block {
                        // Opening fence
                        let code_block_lang = trimmed
                            .strip_prefix("```")
                            .map(|s| s.trim())
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string());

                        let content_spans = if is_current {
                            self.markdown_renderer.render_source(content)
                        } else {
                            self.markdown_renderer
                                .render_code_fence_start(code_block_lang.as_deref())
                        };
                        base_spans.extend(content_spans);

                        // Add diagnostic marker
                        base_spans = self.add_diagnostic_marker(base_spans, line_idx);

                        lines.push(if is_current && self.show_current_line_highlight {
                            Line::from(base_spans).style(Style::default().bg(Color::DarkGray))
                        } else {
                            Line::from(base_spans)
                        });

                        in_code_block = true;
                    } else {
                        // Closing fence
                        let content_spans = if is_current {
                            self.markdown_renderer.render_source(content)
                        } else {
                            self.markdown_renderer.render_code_fence_end()
                        };
                        base_spans.extend(content_spans);

                        // Add diagnostic marker
                        base_spans = self.add_diagnostic_marker(base_spans, line_idx);

                        lines.push(if is_current && self.show_current_line_highlight {
                            Line::from(base_spans).style(Style::default().bg(Color::DarkGray))
                        } else {
                            Line::from(base_spans)
                        });

                        in_code_block = false;
                    }
                } else if in_code_block && !is_current {
                    // Inside code block (not cursor line)
                    let content_spans = self.markdown_renderer.render_code_content(content);
                    base_spans.extend(content_spans);
                    // Add diagnostic marker
                    base_spans = self.add_diagnostic_marker(base_spans, line_idx);
                    lines.push(Line::from(base_spans));
                } else if !in_code_block && Self::get_table_context(line_idx, &tables).is_some() {
                    // Table line handling
                    let table_ctx = Self::get_table_context(line_idx, &tables).unwrap();

                    let content_spans = if is_current {
                        // Current line: show raw source for editing
                        self.markdown_renderer.render_source(content)
                    } else if LineAnalyzer::is_table_separator(content) {
                        // Separator row
                        self.markdown_renderer
                            .render_table_separator(&table_ctx.column_widths, &table_ctx.alignments)
                    } else {
                        // Header or data row
                        let cells = LineAnalyzer::parse_table_cells(content);
                        let is_header = line_idx == table_ctx.start_line;

                        if is_header {
                            self.markdown_renderer.render_table_header(
                                &cells,
                                &table_ctx.column_widths,
                                &table_ctx.alignments,
                            )
                        } else {
                            self.markdown_renderer.render_table_row(
                                &cells,
                                &table_ctx.column_widths,
                                &table_ctx.alignments,
                            )
                        }
                    };
                    base_spans.extend(content_spans);

                    // Add diagnostic marker
                    base_spans = self.add_diagnostic_marker(base_spans, line_idx);

                    if is_current && self.show_current_line_highlight {
                        lines.push(
                            Line::from(base_spans).style(Style::default().bg(Color::DarkGray)),
                        );
                    } else {
                        lines.push(Line::from(base_spans));
                    }
                } else {
                    // Regular line or cursor line
                    let line_type = if in_code_block {
                        LineType::InCode
                    } else {
                        LineAnalyzer::analyze_line(content)
                    };

                    let content_spans = if !is_current {
                        // For non-current lines, check if it's a heading to apply full-width background
                        if let LineType::Heading(level) = line_type {
                            // Calculate available width for content (terminal width - line number width)
                            let line_num_offset = if self.show_line_numbers {
                                line_num_width + 3 // width + " │ "
                            } else {
                                0
                            };
                            let content_width =
                                area.width.saturating_sub(line_num_offset as u16) as usize;
                            self.markdown_renderer.render_heading_line_with_width(
                                content,
                                level,
                                Some(content_width),
                            )
                        } else if let LineType::Image(ref alt_text, ref path) = line_type {
                            // Try to get image dimensions and terminal support info
                            if let Some(img_mgr) = self.image_manager.as_ref() {
                                let dimensions = img_mgr.get_dimensions(path).ok();
                                let supports_images = img_mgr.supports_images();
                                self.markdown_renderer.render_image_with_info(
                                    alt_text,
                                    path,
                                    dimensions,
                                    supports_images,
                                )
                            } else {
                                self.markdown_renderer.render_line(
                                    self.buffer,
                                    line_idx,
                                    is_current,
                                )
                            }
                        } else {
                            self.markdown_renderer
                                .render_line(self.buffer, line_idx, is_current)
                        }
                    } else {
                        self.markdown_renderer
                            .render_line(self.buffer, line_idx, is_current)
                    };
                    base_spans.extend(content_spans);

                    // Add diagnostic marker
                    base_spans = self.add_diagnostic_marker(base_spans, line_idx);

                    if is_current && self.show_current_line_highlight {
                        lines.push(
                            Line::from(base_spans).style(Style::default().bg(Color::DarkGray)),
                        );
                    } else {
                        lines.push(Line::from(base_spans));
                    }
                }
            }
        }

        let paragraph = Paragraph::new(lines)
            .block(Block::default().borders(Borders::NONE))
            .style(Style::default().fg(Color::White).bg(Color::Black));

        paragraph.render(area, buf);
    }
}
