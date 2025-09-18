use crate::events::{AppEvent, Event, EventHandler};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dabgent_agent::handler::Handler;
use dabgent_agent::thread::{self};
use dabgent_mq::db::{EventStore, Metadata, Query};

pub struct App<S: EventStore> {
    store: S,
    query: Query,
    pub thread: thread::Thread,
    pub history: Vec<thread::Event>,
    pub input_buffer: String,
    pub running: bool,
    pub events: EventHandler,
}

impl<S: EventStore> App<S> {
    pub fn new(store: S, stream_id: String) -> color_eyre::Result<Self> {
        let query = Query {
            stream_id: stream_id.clone(),
            event_type: None,
            aggregate_id: Some("thread".to_owned()),
        };

        let event_stream = store.subscribe::<thread::Event>(&query)?;
        let events = EventHandler::new(event_stream);
        let thread = thread::Thread::new();

        Ok(Self {
            store,
            query,
            thread,
            history: Vec::new(),
            input_buffer: String::new(),
            running: true,
            events,
        })
    }

    pub async fn run(mut self, mut terminal: ratatui::DefaultTerminal) -> color_eyre::Result<()> {
        self.fold_thread().await?;
        while self.running {
            terminal.draw(|frame| frame.render_widget(&self, frame.area()))?;
            match self.events.next().await? {
                Event::Tick => self.tick(),
                Event::Crossterm(event) => match event {
                    crossterm::event::Event::Key(key_event) => self.handle_key_events(key_event)?,
                    _ => {}
                },
                Event::Thread(event) => {
                    self.history.push(event);
                    self.fold_thread().await?;
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

    pub async fn fold_thread(&mut self) -> color_eyre::Result<()> {
        let events = self
            .store
            .load_events::<thread::Event>(&self.query, None)
            .await?;
        self.thread = thread::Thread::fold(&events);
        Ok(())
    }

    async fn send_message(&mut self, content: String) -> color_eyre::Result<()> {
        let command = thread::Command::Prompt(content);
        let events = self.thread.process(command)?;
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
}
