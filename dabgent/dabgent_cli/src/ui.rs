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
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(area);

        let content_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(3)])
            .split(main_chunks[0]);

        self.draw_messages(content_chunks[0], buf);
        self.draw_input(content_chunks[1], buf);
        self.draw_event_log(main_chunks[1], buf);
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

    fn draw_event_log(&self, area: Rect, buf: &mut Buffer) {
        let events: Vec<ListItem> = self
            .event_log
            .iter()
            .rev()
            .take(area.height as usize - 2)
            .map(|entry| {
                // Parse the formatted string to apply colors
                let parts: Vec<&str> = entry.formatted.splitn(3, ' ').collect();
                if parts.len() >= 3 {
                    // Format: "chat:1 👤💬 message"
                    let topic_seq = parts[0];
                    let icons = parts[1];
                    let message = parts[2];
                    
                    ListItem::new(Line::from(vec![
                        Span::styled(format!("{} {} ", topic_seq, icons), Style::default()),
                        Span::raw(message),
                    ]))
                } else {
                    ListItem::new(Line::from(entry.formatted.as_str()))
                }
            })
            .collect();

        let event_list = List::new(events)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Events")
                    .border_style(Style::default().fg(Color::Magenta)),
            )
            .style(Style::default().fg(Color::White));

        event_list.render(area, buf);
    }
}
