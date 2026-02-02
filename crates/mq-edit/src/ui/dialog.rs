use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

/// Quit confirmation dialog widget
pub struct QuitDialog;

impl QuitDialog {
    pub fn new() -> Self {
        Self
    }

    /// Calculate the dialog area (centered in the given area)
    fn dialog_area(area: Rect) -> Rect {
        let dialog_width = 50.min(area.width.saturating_sub(4));
        let dialog_height = 7.min(area.height.saturating_sub(2));

        let x = (area.width.saturating_sub(dialog_width)) / 2;
        let y = (area.height.saturating_sub(dialog_height)) / 2;

        Rect::new(x, y, dialog_width, dialog_height)
    }
}

impl Widget for QuitDialog {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let dialog_area = Self::dialog_area(area);

        // Clear the dialog area first
        Clear.render(dialog_area, buf);

        // Create the dialog block with warning colors
        let block = Block::default()
            .title(" Unsaved Changes ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .style(Style::default().bg(Color::Black));

        let inner_area = block.inner(dialog_area);
        block.render(dialog_area, buf);

        // Create dialog content
        let chunks = Layout::vertical([
            Constraint::Length(1), // spacing
            Constraint::Length(1), // message
            Constraint::Length(1), // spacing
            Constraint::Length(1), // buttons
        ])
        .split(inner_area);

        // Warning message
        let message = Paragraph::new("You have unsaved changes. Quit anyway?")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::White));
        message.render(chunks[1], buf);

        // Button hints
        let buttons = Line::from(vec![
            Span::styled(
                " [Y] ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Quit  "),
            Span::styled(
                " [N] ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Cancel "),
        ]);
        let buttons_para = Paragraph::new(buttons).alignment(Alignment::Center);
        buttons_para.render(chunks[3], buf);
    }
}

impl Default for QuitDialog {
    fn default() -> Self {
        Self::new()
    }
}

/// Save As dialog widget for entering a file name
pub struct SaveAsDialog<'a> {
    filename: &'a str,
}

impl<'a> SaveAsDialog<'a> {
    pub fn new(filename: &'a str) -> Self {
        Self { filename }
    }

    /// Calculate the dialog area (centered in the given area)
    fn dialog_area(area: Rect) -> Rect {
        let dialog_width = 60.min(area.width.saturating_sub(4));
        let dialog_height = 7.min(area.height.saturating_sub(2));

        let x = (area.width.saturating_sub(dialog_width)) / 2;
        let y = (area.height.saturating_sub(dialog_height)) / 2;

        Rect::new(x, y, dialog_width, dialog_height)
    }
}

impl<'a> Widget for SaveAsDialog<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let dialog_area = Self::dialog_area(area);

        // Clear the dialog area first
        Clear.render(dialog_area, buf);

        // Create the dialog block
        let block = Block::default()
            .title(" Save As ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().bg(Color::Black));

        let inner_area = block.inner(dialog_area);
        block.render(dialog_area, buf);

        // Create dialog content
        let chunks = Layout::vertical([
            Constraint::Length(1), // spacing
            Constraint::Length(1), // prompt
            Constraint::Length(1), // input field
            Constraint::Length(1), // hint
        ])
        .split(inner_area);

        // Prompt
        let prompt = Paragraph::new("Enter filename:").style(Style::default().fg(Color::White));
        prompt.render(chunks[1], buf);

        // Input field with current filename
        let input = Paragraph::new(self.filename).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
        input.render(chunks[2], buf);

        // Hint
        let hint = Paragraph::new("Press Enter to save, Esc to cancel")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        hint.render(chunks[3], buf);
    }
}

/// Go to line dialog widget for entering a line number
pub struct GotoLineDialog<'a> {
    line_number: &'a str,
    current_line: usize,
    total_lines: usize,
}

impl<'a> GotoLineDialog<'a> {
    pub fn new(line_number: &'a str, current_line: usize, total_lines: usize) -> Self {
        Self {
            line_number,
            current_line,
            total_lines,
        }
    }

    /// Calculate the dialog area (centered in the given area)
    fn dialog_area(area: Rect) -> Rect {
        let dialog_width = 50.min(area.width.saturating_sub(4));
        let dialog_height = 8.min(area.height.saturating_sub(2));

        let x = (area.width.saturating_sub(dialog_width)) / 2;
        let y = (area.height.saturating_sub(dialog_height)) / 2;

        Rect::new(x, y, dialog_width, dialog_height)
    }
}

impl<'a> Widget for GotoLineDialog<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let dialog_area = Self::dialog_area(area);

        // Clear the dialog area first
        Clear.render(dialog_area, buf);

        // Create the dialog block
        let block = Block::default()
            .title(" Go to Line ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().bg(Color::Black));

        let inner_area = block.inner(dialog_area);
        block.render(dialog_area, buf);

        // Create dialog content
        let chunks = Layout::vertical([
            Constraint::Length(1), // spacing
            Constraint::Length(1), // info
            Constraint::Length(1), // prompt
            Constraint::Length(1), // input field
            Constraint::Length(1), // hint
        ])
        .split(inner_area);

        // Info about current position
        let info = Paragraph::new(format!(
            "Current: {} / {}",
            self.current_line + 1,
            self.total_lines
        ))
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
        info.render(chunks[1], buf);

        // Prompt
        let prompt = Paragraph::new("Enter line number:").style(Style::default().fg(Color::White));
        prompt.render(chunks[2], buf);

        // Input field with current input
        let input = Paragraph::new(self.line_number).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
        input.render(chunks[3], buf);

        // Hint
        let hint = Paragraph::new("Press Enter to jump, Esc to cancel")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        hint.render(chunks[4], buf);
    }
}
