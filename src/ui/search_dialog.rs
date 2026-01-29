use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

/// Search mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    Find,
    Replace,
}

/// Search dialog widget
pub struct SearchDialog<'a> {
    search_query: &'a str,
    replace_query: &'a str,
    mode: SearchMode,
    match_count: usize,
    current_match: Option<usize>,
    active_field: SearchField,
}

/// Which field is currently active for input
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchField {
    Search,
    Replace,
}

impl<'a> SearchDialog<'a> {
    pub fn new(search_query: &'a str, match_count: usize, current_match: Option<usize>) -> Self {
        Self {
            search_query,
            replace_query: "",
            mode: SearchMode::Find,
            match_count,
            current_match,
            active_field: SearchField::Search,
        }
    }

    pub fn with_replace(mut self, replace_query: &'a str) -> Self {
        self.replace_query = replace_query;
        self.mode = SearchMode::Replace;
        self
    }

    pub fn with_active_field(mut self, field: SearchField) -> Self {
        self.active_field = field;
        self
    }

    /// Calculate the dialog area (at top center of the given area)
    fn dialog_area(area: Rect, mode: SearchMode) -> Rect {
        let dialog_width = 60.min(area.width.saturating_sub(4));
        let dialog_height = if mode == SearchMode::Replace { 10 } else { 8 };
        let dialog_height = dialog_height.min(area.height.saturating_sub(2));

        let x = (area.width.saturating_sub(dialog_width)) / 2;
        let y = 2; // Near the top with some margin

        Rect::new(x, y, dialog_width, dialog_height)
    }

    fn render_text_field(
        &self,
        label: &str,
        value: &str,
        is_active: bool,
        area: Rect,
        buf: &mut Buffer,
    ) {
        let label_style = Style::default().fg(Color::Cyan);
        let input_bg = if is_active {
            Color::DarkGray
        } else {
            Color::Black
        };
        let input_style = Style::default().fg(Color::White).bg(input_bg);

        // Split area for label and input with padding
        let chunks = Layout::horizontal([
            Constraint::Length(1),  // left padding
            Constraint::Length(10), // label
            Constraint::Length(1),  // gap
            Constraint::Min(1),     // input
            Constraint::Length(1),  // right padding
        ])
        .split(area);

        // Render label
        let label_text = Paragraph::new(format!("{}:", label)).style(label_style);
        label_text.render(chunks[1], buf);

        // Render input field with cursor indicator
        let display_value = if is_active {
            format!("{}_", value)
        } else {
            value.to_string()
        };

        // Fill the input area with background color
        for x in chunks[3].x..chunks[3].x + chunks[3].width {
            buf[(x, chunks[3].y)]
                .set_bg(input_bg)
                .set_char(' ');
        }

        let input_text = Paragraph::new(display_value).style(input_style);
        input_text.render(chunks[3], buf);
    }
}

impl Widget for SearchDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let dialog_area = Self::dialog_area(area, self.mode);

        // Clear the dialog area first
        Clear.render(dialog_area, buf);

        // Create the dialog block
        let title = match self.mode {
            SearchMode::Find => " Find ",
            SearchMode::Replace => " Find & Replace ",
        };
        let block = Block::default()
            .title(title)
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().bg(Color::Black));

        let inner_area = block.inner(dialog_area);
        block.render(dialog_area, buf);

        // Layout based on mode with padding between elements
        let chunks = if self.mode == SearchMode::Replace {
            Layout::vertical([
                Constraint::Length(1), // top padding
                Constraint::Length(1), // search field
                Constraint::Length(1), // gap
                Constraint::Length(1), // replace field
                Constraint::Length(1), // gap
                Constraint::Length(1), // match info
                Constraint::Length(1), // hints
                Constraint::Min(0),    // remaining space
            ])
            .split(inner_area)
        } else {
            Layout::vertical([
                Constraint::Length(1), // top padding
                Constraint::Length(1), // search field
                Constraint::Length(1), // gap
                Constraint::Length(1), // match info
                Constraint::Length(1), // gap
                Constraint::Length(1), // hints
                Constraint::Min(0),    // remaining space
            ])
            .split(inner_area)
        };

        // Render search field (index 1, after top padding)
        self.render_text_field(
            "Find",
            self.search_query,
            self.active_field == SearchField::Search,
            chunks[1],
            buf,
        );

        // Render replace field if in replace mode
        let (match_chunk, hints_chunk) = if self.mode == SearchMode::Replace {
            self.render_text_field(
                "Replace",
                self.replace_query,
                self.active_field == SearchField::Replace,
                chunks[3],
                buf,
            );
            (chunks[5], chunks[6])
        } else {
            (chunks[3], chunks[5])
        };

        // Render match count on its own line
        let match_info = if self.match_count > 0 {
            if let Some(current) = self.current_match {
                format!("{}/{}", current + 1, self.match_count)
            } else {
                format!("{} matches", self.match_count)
            }
        } else if !self.search_query.is_empty() {
            "No matches".to_string()
        } else {
            String::new()
        };

        if !match_info.is_empty() {
            let match_para = Paragraph::new(match_info)
                .style(Style::default().fg(Color::Yellow))
                .alignment(Alignment::Center);
            match_para.render(match_chunk, buf);
        }

        // Render hints
        let hints = if self.mode == SearchMode::Replace {
            Line::from(vec![
                Span::styled(
                    "Enter",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Next  "),
                Span::styled(
                    "Tab",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Switch  "),
                Span::styled(
                    "^R",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Replace  "),
                Span::styled(
                    "^A",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" All  "),
                Span::styled(
                    "Esc",
                    Style::default()
                        .fg(Color::Red)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Close"),
            ])
        } else {
            Line::from(vec![
                Span::styled(
                    "Enter",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Next  "),
                Span::styled(
                    "Shift+Enter",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Prev  "),
                Span::styled(
                    "Esc",
                    Style::default()
                        .fg(Color::Red)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Close"),
            ])
        };

        let hints_para = Paragraph::new(hints)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        hints_para.render(hints_chunk, buf);
    }
}
