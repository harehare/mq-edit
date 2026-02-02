use lsp_types::CompletionItem;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Widget},
};

/// Completion popup widget for displaying code completion suggestions
pub struct CompletionPopup<'a> {
    items: &'a [CompletionItem],
    selected: usize,
}

impl<'a> CompletionPopup<'a> {
    pub fn new(items: &'a [CompletionItem], selected: usize) -> Self {
        Self { items, selected }
    }

    /// Calculate the popup position and size based on cursor position
    pub fn calculate_rect(cursor_x: u16, cursor_y: u16, area: Rect) -> Rect {
        const POPUP_WIDTH: u16 = 40;
        const POPUP_HEIGHT: u16 = 10;

        let x = cursor_x.min(area.width.saturating_sub(POPUP_WIDTH));
        let y = if cursor_y + POPUP_HEIGHT < area.height {
            cursor_y + 1 // Below cursor
        } else {
            cursor_y.saturating_sub(POPUP_HEIGHT) // Above cursor
        };

        Rect {
            x,
            y,
            width: POPUP_WIDTH.min(area.width - x),
            height: POPUP_HEIGHT.min(area.height - y),
        }
    }
}

impl Widget for CompletionPopup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Calculate visible height (subtract 2 for borders)
        let visible_height = area.height.saturating_sub(2) as usize;

        if visible_height == 0 || self.items.is_empty() {
            return;
        }

        // Calculate scroll offset to keep selected item visible
        let scroll_offset = if self.selected >= visible_height {
            self.selected - visible_height + 1
        } else {
            0
        };

        // Get the visible slice of items
        let mut visible_items: Vec<ListItem> = self
            .items
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(visible_height)
            .map(|(idx, item)| {
                let label = &item.label;
                let kind_text = item.kind.map(|k| format!("[{:?}]", k)).unwrap_or_default();

                let content = if idx == self.selected {
                    Line::from(vec![
                        Span::styled(
                            format!("> {:37} ", label),
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            kind_text,
                            Style::default().fg(Color::DarkGray).bg(Color::Cyan),
                        ),
                    ])
                } else {
                    Line::from(vec![
                        Span::styled(
                            format!("  {:37} ", label),
                            Style::default().fg(Color::White).bg(Color::Black),
                        ),
                        Span::styled(
                            kind_text,
                            Style::default().fg(Color::DarkGray).bg(Color::Black),
                        ),
                    ])
                };

                ListItem::new(content)
            })
            .collect();

        while visible_items.len() < visible_height {
            visible_items.push(ListItem::new(Line::from(vec![Span::styled(
                " ".repeat(40),
                Style::default().bg(Color::Black),
            )])));
        }

        // Create scroll indicator in title
        let total = self.items.len();
        let title = if total > visible_height {
            let end = (scroll_offset + visible_height).min(total);
            format!(" Completions ({}-{}/{}) ", scroll_offset + 1, end, total)
        } else {
            " Completions ".to_string()
        };

        let list = List::new(visible_items)
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .style(Style::default().bg(Color::Black).fg(Color::White))
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .style(Style::default().bg(Color::Black).fg(Color::White));

        list.render(area, buf);
    }
}
