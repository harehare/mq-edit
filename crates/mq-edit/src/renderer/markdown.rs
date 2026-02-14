use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};

use super::Renderer;
use crate::document::{DocumentBuffer, LineType, TableAlignment};

/// Markdown renderer for rich text display
pub struct MarkdownRenderer {
    /// Theme colors and styles
    heading1_style: Style,
    heading2_style: Style,
    heading3_style: Style,
    heading4_style: Style,
    heading_other_style: Style,
    code_block_style: Style,
    quote_style: Style,
    quote_border_style: Style,
    /// Table styles
    table_border_style: Style,
    table_header_style: Style,
    table_cell_style: Style,
}

impl MarkdownRenderer {
    pub fn new() -> Self {
        Self {
            heading1_style: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            heading2_style: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            heading3_style: Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
            heading4_style: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            heading_other_style: Style::default().add_modifier(Modifier::BOLD),
            code_block_style: Style::default().fg(Color::Green),
            quote_style: Style::default().fg(Color::Gray),
            quote_border_style: Style::default().fg(Color::DarkGray),
            table_border_style: Style::default().fg(Color::DarkGray),
            table_header_style: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            table_cell_style: Style::default(),
        }
    }

    /// Render a line as either source (if is_current) or rich formatted (legacy method)
    pub fn render_line_with_type(
        &self,
        content: &str,
        line_type: &LineType,
        is_current: bool,
    ) -> Vec<Span<'_>> {
        if is_current {
            // Current line: show source
            vec![Span::styled(content.to_string(), Style::default())]
        } else {
            // Other lines: show rich formatted based on line type
            self.render_rich(content, line_type)
        }
    }

    /// Render line as rich formatted text based on LineType
    fn render_rich(&self, content: &str, line_type: &LineType) -> Vec<Span<'_>> {
        match line_type {
            LineType::Heading(level) => self.render_heading_line(content, *level),
            LineType::ListItem => self.render_list_item(content, false, false),
            LineType::OrderedListItem => self.render_list_item(content, true, false),
            LineType::TaskListItem(checked) => self.render_list_item(content, false, *checked),
            LineType::Blockquote => self.render_blockquote_line(content),
            LineType::CodeFence(lang) => self.render_code_fence(content, lang.as_deref()),
            LineType::InCode => self.render_code_content(content),
            LineType::HorizontalRule => vec![Span::styled(
                "─".repeat(80),
                Style::default().fg(Color::DarkGray),
            )],
            LineType::Image(alt_text, path) => self.render_image(alt_text, path),
            LineType::TableHeader(cells) => {
                // Fallback rendering without context (column widths)
                let widths: Vec<usize> = cells.iter().map(|c| c.chars().count()).collect();
                let alignments = vec![TableAlignment::Left; cells.len()];
                self.render_table_header(cells, &widths, &alignments)
            }
            LineType::TableSeparator(alignments) => {
                // Fallback rendering without context
                let widths = vec![10; alignments.len()];
                self.render_table_separator(&widths, alignments)
            }
            LineType::TableRow(cells) => {
                // Fallback rendering without context
                let widths: Vec<usize> = cells.iter().map(|c| c.chars().count()).collect();
                let alignments = vec![TableAlignment::Left; cells.len()];
                self.render_table_row(cells, &widths, &alignments)
            }
            LineType::FrontMatterDelimiter => self.render_front_matter_delimiter(),
            LineType::FrontMatterContent => self.render_front_matter_content(content),
            LineType::Text => self.render_text_line(content),
        }
    }

    /// Render heading line
    fn render_heading_line(&self, content: &str, level: usize) -> Vec<Span<'_>> {
        self.render_heading_line_with_width(content, level, None)
    }

    /// Render heading line with optional terminal width for full-width background
    pub fn render_heading_line_with_width(
        &self,
        content: &str,
        level: usize,
        terminal_width: Option<usize>,
    ) -> Vec<Span<'_>> {
        let style = match level {
            1 => self.heading1_style,
            2 => self.heading2_style,
            3 => self.heading3_style,
            4 => self.heading4_style,
            _ => self.heading_other_style,
        };

        // Extract text after heading markers
        let text = content.trim_start_matches('#').trim();

        // Add visual prefix for headings - make level more obvious
        let prefix = match level {
            1 => "# ",      // H1
            2 => "## ",     // H2
            3 => "### ",    // H3
            4 => "#### ",   // H4
            5 => "##### ",  // H5
            _ => "###### ", // H6+
        };

        // Add background color for headers - matching the text color tone
        let bg_color = match level {
            1 => Color::Rgb(0, 60, 80),  // Dark cyan matching Cyan text
            2 => Color::Rgb(0, 40, 100), // Dark blue matching Blue text
            3 => Color::Rgb(60, 0, 80),  // Dark magenta matching Magenta text
            4 => Color::Rgb(80, 60, 0),  // Dark yellow matching Yellow text
            _ => Color::Rgb(40, 40, 40), // Dark gray for others
        };
        let style_with_bg = style.bg(bg_color);

        let heading_text = format!("{}{}", prefix, text);

        // If terminal width is provided, pad to fill the line
        if let Some(width) = terminal_width {
            let text_len = heading_text.chars().count();
            if text_len < width {
                let padding = " ".repeat(width - text_len);
                vec![
                    Span::styled(heading_text, style_with_bg),
                    Span::styled(padding, Style::default().bg(bg_color)),
                ]
            } else {
                vec![Span::styled(heading_text, style_with_bg)]
            }
        } else {
            vec![Span::styled(heading_text, style_with_bg)]
        }
    }

    /// Render list item
    fn render_list_item(&self, content: &str, ordered: bool, task_checked: bool) -> Vec<Span<'_>> {
        let trimmed = content.trim_start();

        // Determine indentation
        let indent_count = content.len() - trimmed.len();
        let indent = " ".repeat(indent_count);

        // Determine bullet and extract text
        let (bullet, text) = if ordered {
            // Find the number and ". " or ") "
            if let Some(dot_pos) = trimmed.find(". ") {
                let num = &trimmed[..dot_pos + 1];
                let text = &trimmed[dot_pos + 2..];
                (num.to_string(), text)
            } else if let Some(paren_pos) = trimmed.find(") ") {
                let num = &trimmed[..paren_pos + 1];
                let text = &trimmed[paren_pos + 2..];
                (num.to_string(), text)
            } else {
                ("1. ".to_string(), trimmed)
            }
        } else if trimmed.starts_with("- [") {
            let bullet = if task_checked { "[✓] " } else { "[ ] " };
            let text = trimmed
                .strip_prefix("- [x] ")
                .or_else(|| trimmed.strip_prefix("- [X] "))
                .or_else(|| trimmed.strip_prefix("- [ ] "))
                .unwrap_or(trimmed);
            (bullet.to_string(), text)
        } else {
            let bullet = "• ";
            let text = trimmed
                .strip_prefix("- ")
                .or_else(|| trimmed.strip_prefix("* "))
                .or_else(|| trimmed.strip_prefix("+ "))
                .unwrap_or(trimmed);
            (bullet.to_string(), text)
        };

        vec![
            Span::raw(indent),
            Span::styled(bullet, Style::default().fg(Color::Yellow)),
            Span::raw(text.to_string()),
        ]
    }

    /// Render blockquote line
    fn render_blockquote_line(&self, content: &str) -> Vec<Span<'_>> {
        let text = content.trim_start().strip_prefix("> ").unwrap_or(content);
        vec![
            Span::styled("▎ ", self.quote_border_style),
            Span::styled(text.to_string(), self.quote_style),
        ]
    }

    /// Render code fence line (start or end of code block)
    fn render_code_fence(&self, _content: &str, lang: Option<&str>) -> Vec<Span<'_>> {
        if let Some(lang) = lang {
            vec![Span::styled(
                format!("╭─ {} ─╮", lang),
                Style::default().fg(Color::DarkGray),
            )]
        } else {
            vec![Span::styled(
                "╭─ code ─╮",
                Style::default().fg(Color::DarkGray),
            )]
        }
    }

    /// Render code fence start (opening ```lang)
    pub fn render_code_fence_start(&self, lang: Option<&str>) -> Vec<Span<'_>> {
        if let Some(lang) = lang {
            vec![Span::styled(
                format!(
                    "╭─ {} ─────────────────────────────────────────────────────",
                    lang
                ),
                Style::default().fg(Color::DarkGray),
            )]
        } else {
            vec![Span::styled(
                "╭─ code ────────────────────────────────────────────────────",
                Style::default().fg(Color::DarkGray),
            )]
        }
    }

    /// Render code fence end (closing ```)
    pub fn render_code_fence_end(&self) -> Vec<Span<'_>> {
        vec![Span::styled(
            "╰────────────────────────────────────────────────────────────",
            Style::default().fg(Color::DarkGray),
        )]
    }

    /// Render code content line (inside code block)
    pub fn render_code_content(&self, content: &str) -> Vec<Span<'_>> {
        vec![Span::styled(content.to_string(), self.code_block_style)]
    }

    /// Render source code as-is
    pub fn render_source(&self, content: &str) -> Vec<Span<'_>> {
        vec![Span::styled(content.to_string(), Style::default())]
    }

    /// Render text line with inline formatting
    fn render_text_line(&self, content: &str) -> Vec<Span<'_>> {
        // For now, simple rendering
        // TODO: Parse inline formatting (bold, italic, code, links)
        vec![Span::raw(content.to_string())]
    }

    /// Render image placeholder with detailed information
    pub fn render_image_with_info(
        &self,
        alt_text: &str,
        path: &str,
        dimensions: Option<(u32, u32)>,
    ) -> Vec<Span<'_>> {
        let mut spans = vec![
            Span::styled("🖼️  ", Style::default().fg(Color::Cyan)),
            Span::styled(
                format!("[{}]", alt_text),
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::ITALIC),
            ),
            Span::styled(" ", Style::default()),
        ];

        // Add dimensions if available
        if let Some((width, height)) = dimensions {
            spans.push(Span::styled(
                format!("{}x{} ", width, height),
                Style::default().fg(Color::Yellow),
            ));
        }

        spans.push(Span::styled(
            path.to_string(),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::UNDERLINED),
        ));

        spans
    }

    /// Render image placeholder (basic version for backwards compatibility)
    fn render_image(&self, alt_text: &str, path: &str) -> Vec<Span<'_>> {
        vec![
            Span::styled("🖼️  ", Style::default().fg(Color::Cyan)),
            Span::styled(
                format!("[{}]", alt_text),
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::ITALIC),
            ),
            Span::styled(" ", Style::default()),
            Span::styled(
                path.to_string(),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::UNDERLINED),
            ),
        ]
    }

    /// Render table header row
    pub fn render_table_header(
        &self,
        cells: &[String],
        column_widths: &[usize],
        alignments: &[TableAlignment],
    ) -> Vec<Span<'_>> {
        self.render_table_row_internal(cells, column_widths, alignments, true)
    }

    /// Render table separator row with box drawing characters
    pub fn render_table_separator(
        &self,
        column_widths: &[usize],
        alignments: &[TableAlignment],
    ) -> Vec<Span<'_>> {
        let mut result = String::new();
        result.push('├');

        for (i, &width) in column_widths.iter().enumerate() {
            let left_colon = matches!(
                alignments.get(i),
                Some(TableAlignment::Left) | Some(TableAlignment::Center)
            );
            let right_colon = matches!(
                alignments.get(i),
                Some(TableAlignment::Right) | Some(TableAlignment::Center)
            );

            if left_colon {
                result.push(':');
                result.push_str(&"─".repeat(width.saturating_sub(1) + 1));
            } else {
                result.push_str(&"─".repeat(width + 2));
            }

            if right_colon && !left_colon {
                // Replace last char with ':'
                result.pop();
                result.push(':');
            } else if right_colon && left_colon {
                result.pop();
                result.push(':');
            }

            if i < column_widths.len() - 1 {
                result.push('┼');
            }
        }
        result.push('┤');

        vec![Span::styled(result, self.table_border_style)]
    }

    /// Render table data row
    pub fn render_table_row(
        &self,
        cells: &[String],
        column_widths: &[usize],
        alignments: &[TableAlignment],
    ) -> Vec<Span<'_>> {
        self.render_table_row_internal(cells, column_widths, alignments, false)
    }

    /// Internal helper for rendering table rows
    fn render_table_row_internal(
        &self,
        cells: &[String],
        column_widths: &[usize],
        alignments: &[TableAlignment],
        is_header: bool,
    ) -> Vec<Span<'_>> {
        let mut spans = Vec::new();

        // Left border
        spans.push(Span::styled("│", self.table_border_style));

        for (i, cell) in cells.iter().enumerate() {
            let width = column_widths
                .get(i)
                .copied()
                .unwrap_or(cell.chars().count());
            let alignment = alignments.get(i).copied().unwrap_or(TableAlignment::Left);

            // Pad cell content based on alignment
            let padded = Self::pad_cell(cell, width, alignment);

            let style = if is_header {
                self.table_header_style
            } else {
                self.table_cell_style
            };

            spans.push(Span::styled(format!(" {} ", padded), style));
            spans.push(Span::styled("│", self.table_border_style));
        }

        spans
    }

    /// Pad cell content according to alignment
    fn pad_cell(content: &str, width: usize, alignment: TableAlignment) -> String {
        let content_width = content.chars().count();
        if content_width >= width {
            return content.to_string();
        }

        let padding = width - content_width;
        match alignment {
            TableAlignment::Left | TableAlignment::None => {
                format!("{}{}", content, " ".repeat(padding))
            }
            TableAlignment::Right => {
                format!("{}{}", " ".repeat(padding), content)
            }
            TableAlignment::Center => {
                let left_pad = padding / 2;
                let right_pad = padding - left_pad;
                format!(
                    "{}{}{}",
                    " ".repeat(left_pad),
                    content,
                    " ".repeat(right_pad)
                )
            }
        }
    }

    /// Render front matter delimiter (--- or +++)
    fn render_front_matter_delimiter(&self) -> Vec<Span<'_>> {
        vec![Span::styled(
            "─".repeat(60),
            Style::default().fg(Color::Rgb(100, 100, 150)),
        )]
    }

    /// Render front matter content (YAML/TOML)
    fn render_front_matter_content(&self, content: &str) -> Vec<Span<'_>> {
        // Simple YAML syntax highlighting
        let trimmed = content.trim_start();

        // Check if it's a YAML key-value pair
        if let Some(colon_pos) = trimmed.find(':') {
            let key = &trimmed[..colon_pos];
            let value = &trimmed[colon_pos..];

            vec![
                Span::styled(
                    key.to_string(),
                    Style::default()
                        .fg(Color::Rgb(150, 180, 200))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    value.to_string(),
                    Style::default().fg(Color::Rgb(200, 200, 220)),
                ),
            ]
        } else if trimmed.starts_with('-') {
            // YAML list item
            vec![Span::styled(
                content.to_string(),
                Style::default().fg(Color::Rgb(180, 180, 200)),
            )]
        } else if trimmed.starts_with('#') {
            // YAML comment
            vec![Span::styled(
                content.to_string(),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )]
        } else {
            // Default front matter style
            vec![Span::styled(
                content.to_string(),
                Style::default().fg(Color::Rgb(180, 180, 200)),
            )]
        }
    }

    /// Determine line type with context awareness for front matter
    fn determine_line_type(
        &self,
        buffer: &DocumentBuffer,
        line_idx: usize,
        content: &str,
    ) -> crate::document::LineType {
        use crate::document::LineAnalyzer;

        let line_type = LineAnalyzer::analyze_line(content);

        // Check if we're inside a front matter block
        if let crate::document::LineType::FrontMatterDelimiter = line_type {
            return line_type;
        }

        // Check if this line is inside a front matter block
        if self.is_inside_front_matter(buffer, line_idx) {
            return crate::document::LineType::FrontMatterContent;
        }

        line_type
    }

    /// Check if a line is inside a front matter block
    fn is_inside_front_matter(&self, buffer: &DocumentBuffer, line_idx: usize) -> bool {
        // Front matter must start at line 0
        if line_idx == 0 {
            return false;
        }

        // Check if line 0 is a front matter delimiter
        if let Some(first_line) = buffer.line(0) {
            let first_trimmed = first_line.trim();
            if first_trimmed != "---" && first_trimmed != "+++" {
                return false;
            }
        } else {
            return false;
        }

        // Look backwards from current line to find if we're between delimiters
        let mut delimiter_count = 0;
        for i in 0..=line_idx {
            if let Some(line) = buffer.line(i) {
                let trimmed = line.trim();
                if trimmed == "---" || trimmed == "+++" {
                    delimiter_count += 1;
                }
            }
        }

        // If we've seen exactly one delimiter, we're inside the front matter
        delimiter_count == 1
    }
}

impl Default for MarkdownRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer for MarkdownRenderer {
    fn render_line(
        &self,
        buffer: &DocumentBuffer,
        line_idx: usize,
        is_current_line: bool,
    ) -> Vec<Span<'_>> {
        // Get line content
        let content = buffer.line(line_idx).unwrap_or("");

        if is_current_line {
            // Current line: show source for editing
            vec![Span::styled(content.to_string(), Style::default())]
        } else {
            // Other lines: show rich formatted based on line type
            let line_type = self.determine_line_type(buffer, line_idx, content);
            self.render_rich(content, &line_type)
        }
    }

    fn supports_wysiwyg(&self) -> bool {
        true // Markdown renderer supports WYSIWYG mode
    }
}
