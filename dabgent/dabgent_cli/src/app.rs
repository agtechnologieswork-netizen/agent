use crate::events::{AppEvent, Event, EventHandler};
use crate::session::{ChatCommand, ChatEvent, ChatSession};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dabgent_agent::handler::Handler;
use dabgent_mq::db::{EventStore, Metadata, Query};
use std::collections::VecDeque;

pub struct App<S: EventStore> {
    store: S,
    query: Query,
    pub session: ChatSession,
    pub input_buffer: String,
    pub running: bool,
    pub events: EventHandler,
    pub event_log: VecDeque<EventLogEntry>,
    chat_seq: i64,
}

#[derive(Clone, Debug)]
pub struct EventLogEntry {
    pub formatted: String,
}

impl<S: EventStore> App<S> {
    pub fn new(store: S, stream_id: String, aggregate_id: String) -> color_eyre::Result<Self> {
        let query = Query {
            stream_id: stream_id.clone(),
            event_type: None,
            aggregate_id: Some(aggregate_id.clone()),
        };

        let event_stream = store.subscribe::<ChatEvent>(&query)?;
        let events = EventHandler::new(event_stream);
        let session = ChatSession::new();
        
        Ok(Self {
            store,
            query,
            session,
            input_buffer: String::new(),
            running: true,
            events,
            event_log: VecDeque::with_capacity(100),
            chat_seq: 0,
        })
    }

    pub async fn run(mut self, mut terminal: ratatui::DefaultTerminal) -> color_eyre::Result<()> {
        self.fold_session().await?;
        while self.running {
            terminal.draw(|frame| frame.render_widget(&self, frame.area()))?;
            match self.events.next().await? {
                Event::Tick => self.tick(),
                Event::Crossterm(event) => match event {
                    crossterm::event::Event::Key(key_event) => self.handle_key_events(key_event)?,
                    _ => {}
                },
                Event::Chat(ref chat_event) => {
                    // Log the actual event from the topic
                    self.chat_seq += 1;
                    let icons = match chat_event {
                        ChatEvent::UserMessage { .. } => "💬 👤",
                        ChatEvent::AgentMessage { .. } => "↩ 🤖",
                    };
                    let summary = match chat_event {
                        ChatEvent::UserMessage { content, .. } => content.clone(),
                        ChatEvent::AgentMessage { content, .. } => content.clone(),
                    };
                    self.log_event("chat", self.chat_seq, icons, summary);
                    self.fold_session().await?;
                }
                Event::App(app_event) => match app_event {
                    AppEvent::Confirm => self.confirm().await?,
                    AppEvent::Erase => self.erase(),
                    AppEvent::Input(input) => self.input(input),
                    AppEvent::Quit => self.quit(),
                },
            }
        }
        Ok(())
    }

    pub fn handle_key_events(&mut self, key: KeyEvent) -> color_eyre::Result<()> {
        match key.code {
            KeyCode::Enter => self.events.send(Event::App(AppEvent::Confirm)),
            KeyCode::Char('c' | 'C') if key.modifiers == KeyModifiers::CONTROL => {
                self.events.send(Event::App(AppEvent::Quit))
            }
            KeyCode::Char(c) => self.events.send(Event::App(AppEvent::Input(c))),
            KeyCode::Backspace => self.events.send(Event::App(AppEvent::Erase)),
            _ => {}
        }
        Ok(())
    }

    pub async fn fold_session(&mut self) -> color_eyre::Result<()> {
        let events = self
            .store
            .load_events::<ChatEvent>(&self.query, None)
            .await?;
        self.session = ChatSession::fold(&events);
        Ok(())
    }

    async fn send_message(&mut self, content: String) -> color_eyre::Result<()> {
        let command = ChatCommand::SendMessage(content);
        let events = self.session.process(command)?;
        let metadata = Metadata::default();
        for event in events {
            self.store
                .push_event(
                    &self.query.stream_id,
                    self.query.aggregate_id.as_ref().unwrap(),
                    &event,
                    &metadata,
                )
                .await?;
        }
        Ok(())
    }

    pub fn tick(&self) {
        // animations
    }

    pub fn erase(&mut self) {
        self.input_buffer.pop();
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub async fn confirm(&mut self) -> color_eyre::Result<()> {
        if !self.input_buffer.is_empty() {
            self.send_message(self.input_buffer.clone()).await?;
            self.input_buffer.clear();
        }
        Ok(())
    }

    pub fn input(&mut self, input: char) {
        self.input_buffer.push(input);
    }
    
    
    fn log_event(&mut self, stream: &str, seq: i64, icons: &str, summary: String) {
        let entry = EventLogEntry {
            formatted: format!("{}:{:<2} {} {}", stream, seq, icons, summary),
        };
        self.event_log.push_back(entry);
        if self.event_log.len() > 50 {
            self.event_log.pop_front();
        }
    }
}
