use crate::{App, ChatEvent};
use dabgent_mq::db::EventStore;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Widget},
};

impl<S: EventStore> Widget for &App<S> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(3)])
            .split(area.clone());

        self.draw_messages(chunks[0], buf);
        self.draw_input(chunks[1], buf);
    }
}

impl<S: EventStore> App<S> {
    fn draw_messages(&self, area: Rect, buf: &mut Buffer) {
        let messages: Vec<ListItem> = self
            .session
            .messages()
            .iter()
            .map(|event| {
                let (prefix, content, style) = match event {
                    ChatEvent::UserMessage { content, .. } => {
                        ("User: ", content.as_str(), Style::default().fg(Color::Cyan))
                    }
                    ChatEvent::AgentMessage { content, .. } => (
                        "Agent: ",
                        content.as_str(),
                        Style::default().fg(Color::Green),
                    ),
                };
                ListItem::new(Line::from(vec![
                    Span::styled(prefix, style),
                    Span::raw(content),
                ]))
            })
            .collect();

        let messages_list =
            List::new(messages).block(Block::default().borders(Borders::ALL).title("Messages"));

        messages_list.render(area, buf);
    }

    fn draw_input(&self, area: Rect, buf: &mut Buffer) {
        let input = Paragraph::new(self.input_buffer.as_str())
            .style(Style::default())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Input (Enter to send)"),
            );

        input.render(area, buf);
    }
}
