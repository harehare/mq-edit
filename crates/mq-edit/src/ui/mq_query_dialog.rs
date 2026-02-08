use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

/// mq query execution dialog widget
pub struct MqQueryDialog<'a> {
    query: &'a str,
    result: Option<&'a str>,
}

impl<'a> MqQueryDialog<'a> {
    pub fn new(query: &'a str, result: Option<&'a str>) -> Self {
        Self { query, result }
    }

    /// Calculate the dialog area (centered in the given area)
    fn dialog_area(area: Rect, has_result: bool) -> Rect {
        let dialog_width = 60.min(area.width.saturating_sub(4));
        let dialog_height = if has_result { 7 } else { 5 };
        let dialog_height = dialog_height.min(area.height.saturating_sub(2));

        let x = (area.width.saturating_sub(dialog_width)) / 2;
        let y = (area.height.saturating_sub(dialog_height)) / 2;

        Rect::new(x, y, dialog_width, dialog_height)
    }
}

impl Widget for MqQueryDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let dialog_area = Self::dialog_area(area, self.result.is_some());

        Clear.render(dialog_area, buf);

        let block = Block::default()
            .title(" mq ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta))
            .style(Style::default().bg(Color::Black));

        let inner_area = block.inner(dialog_area);
        block.render(dialog_area, buf);

        if self.result.is_some() {
            let chunks = Layout::vertical([
                Constraint::Length(1), // top padding
                Constraint::Length(1), // query input
                Constraint::Length(1), // spacing
                Constraint::Length(1), // result/error
                Constraint::Length(1), // hints
            ])
            .split(inner_area);

            self.render_input(chunks[1], buf);
            self.render_result(chunks[3], buf);
            self.render_hints(chunks[4], buf);
        } else {
            let chunks = Layout::vertical([
                Constraint::Length(1), // top padding
                Constraint::Length(1), // query input
                Constraint::Length(1), // bottom padding
            ])
            .split(inner_area);

            self.render_input(chunks[1], buf);
        }
    }
}

impl MqQueryDialog<'_> {
    fn render_input(&self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::horizontal([
            Constraint::Length(1), // left padding
            Constraint::Length(3), // icon
            Constraint::Min(1),    // input
            Constraint::Length(2), // gap
            Constraint::Length(9), // hints inline
            Constraint::Length(1), // right padding
        ])
        .split(area);

        let icon = Paragraph::new("> ").style(Style::default().fg(Color::Magenta));
        icon.render(chunks[1], buf);

        // Fill input area with background
        for x in chunks[2].x..chunks[2].x + chunks[2].width {
            buf[(x, chunks[2].y)].set_bg(Color::DarkGray).set_char(' ');
        }

        let display_query = format!("{}_", self.query);
        let input = Paragraph::new(display_query).style(
            Style::default()
                .fg(Color::White)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );
        input.render(chunks[2], buf);

        // Inline hints when no result shown
        if self.result.is_none() {
            let hints = Line::from(vec![
                Span::styled("Enter", Style::default().fg(Color::DarkGray)),
                Span::raw(" "),
                Span::styled("Esc", Style::default().fg(Color::DarkGray)),
            ]);
            Paragraph::new(hints).render(chunks[4], buf);
        }
    }

    fn render_result(&self, area: Rect, buf: &mut Buffer) {
        if let Some(result) = self.result {
            let chunks = Layout::horizontal([
                Constraint::Length(1), // left padding
                Constraint::Min(1),    // content
                Constraint::Length(1), // right padding
            ])
            .split(area);

            let is_error = result.starts_with("Error:");
            let style = if is_error {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::Green)
            };

            Paragraph::new(result).style(style).render(chunks[1], buf);
        }
    }

    fn render_hints(&self, area: Rect, buf: &mut Buffer) {
        let hints = Line::from(vec![
            Span::styled(
                "Enter",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Run  "),
            Span::styled(
                "Esc",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Close"),
        ]);
        Paragraph::new(hints)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray))
            .render(area, buf);
    }
}
