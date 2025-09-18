use dabgent_agent::llm::CompletionResponse;
use dabgent_agent::thread::{Event, ToolResponse};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, List, ListItem, ListState, StatefulWidget, Widget},
};
use rig::completion::message::{
    AssistantContent, ToolCall, ToolResult, ToolResultContent, UserContent,
};

pub struct EventList<'a> {
    events: &'a [Event],
}

impl<'a> EventList<'a> {
    pub fn new(events: &'a [Event]) -> Self {
        Self { events }
    }
}

impl Widget for EventList<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = ListState::default(); // move to parent state

        let items: Vec<ListItem> = self
            .events
            .iter()
            .map(|event| ListItem::new(event_as_text(event)))
            .collect();

        let list = List::new(items)
            .block(Block::default().title("Event List"))
            .highlight_style(Style::default().fg(Color::Yellow))
            .highlight_symbol(">> ");

        StatefulWidget::render(list, area, buf, &mut state);
    }
}

pub fn event_as_text(event: &Event) -> Text<'_> {
    match event {
        Event::Prompted(prompt) => render_prompted(prompt),
        Event::LlmCompleted(completion) => render_llm_completed(completion),
        Event::ToolCompleted(result) => render_tool_completed(result),
        Event::ArtifactsCollected(artifacts) => render_artifacts_collected(artifacts),
    }
}

pub fn render_prompted(prompt: &str) -> Text<'_> {
    Text::from_iter(prompt.lines().map(|line| Line::from(line.to_owned())))
}

pub fn render_artifacts_collected(
    artifacts: &std::collections::HashMap<String, String>,
) -> Text<'_> {
    Text::from(format!("Collected {} artifacts", artifacts.len()))
}

pub fn render_llm_completed(completion: &CompletionResponse) -> Text<'_> {
    let mut lines = Vec::new();
    for item in completion.choice.iter() {
        match item {
            AssistantContent::Text(text) => {
                for line in text.text.lines() {
                    lines.push(Line::from(line.to_owned()));
                }
            }
            AssistantContent::Reasoning(reasoning) => {
                lines.push(Line::from("[reasoning]"));
                for line in reasoning.reasoning.iter() {
                    lines.push(Line::from(line.to_owned()));
                }
            }
            AssistantContent::ToolCall(tool_call) => {
                lines.append(&mut tool_call_lines(tool_call));
            }
        }
    }
    Text::from(lines)
}

pub fn render_tool_completed(response: &ToolResponse) -> Text<'_> {
    let mut lines = Vec::new();
    for item in response.content.iter() {
        match item {
            UserContent::Text(text) => {
                for line in text.text.lines() {
                    lines.push(Line::from(line.to_owned()));
                }
            }
            UserContent::ToolResult(tool_result) => {
                lines.append(&mut tool_result_lines(tool_result));
            }
            _ => continue,
        }
    }
    Text::from(lines)
}

pub fn tool_call_lines(value: &ToolCall) -> Vec<Line<'_>> {
    let args = serde_json::to_string_pretty(&value.function.arguments).unwrap();
    let mut lines = vec![Line::from(vec![
        Span::styled(value.function.name.clone(), Style::new().bold()),
        Span::raw(" "),
        Span::styled(format!("[{}]", value.id), Style::new().gray()),
    ])];
    for line in args.lines() {
        lines.push(Line::from(line.to_owned()));
    }
    lines
}

pub fn tool_result_lines(value: &ToolResult) -> Vec<Line<'_>> {
    let mut lines = vec![Line::from(vec![Span::styled(
        format!("[{}]", value.id),
        Style::new().gray(),
    )])];
    for item in value.content.iter() {
        match item {
            ToolResultContent::Text(text) => {
                match serde_json::from_str::<serde_json::Value>(&text.text) {
                    Ok(json_value) => {
                        for line in serde_json::to_string_pretty(&json_value).unwrap().lines() {
                            lines.push(Line::from(line.to_owned()));
                        }
                    }
                    Err(_) => lines.push(Line::from(text.text.clone())),
                }
            }
            ToolResultContent::Image(..) => {
                lines.push(Line::from("[image]"));
            }
        }
    }
    lines
}
