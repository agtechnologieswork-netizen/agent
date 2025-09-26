use crate::events::{AppEvent, Event as UiEvent, EventHandler};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dabgent_agent::Aggregate;
use dabgent_agent::event::Event as AgentEvent;
use dabgent_agent::processor::thread::{self};
use dabgent_mq::db::{Event as StoreEvent, EventStore, Metadata, Query};
use dabgent_mq::Event as EventTrait;
use ratatui::widgets::ListState;
use rig::OneOrMany;
use rig::message::{Text, UserContent};

pub struct App<S: EventStore> {
    store: S,
    query: Query,
    pub thread: thread::Thread,
    pub history: Vec<StoreEvent<AgentEvent>>,
    pub input_buffer: String,
    pub running: bool,
    pub events: EventHandler,
    pub pending_prompt: Option<String>,
    pub pending_prompt_target: Option<String>,
    pub list_state: ListState,
    pub auto_scroll: bool,
}

impl<S: EventStore> App<S> {
    pub fn new(store: S, stream_id: String) -> color_eyre::Result<Self> {
        let query = Query::stream(stream_id.clone()).aggregate("thread");

        let event_stream = store.subscribe::<AgentEvent>(&Query::stream(stream_id.clone()))?;
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
            pending_prompt: None,
            pending_prompt_target: None,
            list_state: ListState::default(),
            auto_scroll: true,
        })
    }

    pub async fn run(mut self, mut terminal: ratatui::DefaultTerminal) -> color_eyre::Result<()> {
        self.setup_thread().await?;
        self.fold_thread().await?;
        while self.running {
            terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;
            match self.events.next().await? {
                UiEvent::Tick => self.tick(),
                UiEvent::Crossterm(event) => match event {
                    crossterm::event::Event::Key(key_event) => self.handle_key_events(key_event)?,
                    _ => {}
                },
                UiEvent::Thread(event) => {
                    let aggregate_id = event.aggregate_id.clone();

                    // Emit debug event for event processing
                    let debug_event = AgentEvent::Debug {
                        message: format!("Processing event: {}", event.data.event_type()),
                        context: Some(serde_json::json!({
                            "aggregate_id": aggregate_id.clone(),
                            "event_type": event.data.event_type(),
                            "created_at": event.created_at.to_rfc3339(),
                        })),
                        target: "debugger".to_string(),
                    };
                    self.history.push(StoreEvent {
                        stream_id: self.query.stream_id.clone(),
                        event_type: "debug".to_string(),
                        aggregate_id: "debugger".to_string(),
                        sequence: self.history.len() as i64,
                        event_version: "1.0".to_string(),
                        data: debug_event,
                        metadata: serde_json::json!({}),
                        created_at: chrono::Utc::now(),
                    });

                    if let dabgent_agent::event::Event::UserInputRequested { prompt, .. } =
                        &event.data
                    {
                        self.pending_prompt = Some(prompt.clone());
                        self.pending_prompt_target = Some(aggregate_id.clone());
                    }

                    self.history.push(event);

                    // Auto-scroll to bottom if enabled
                    if self.auto_scroll && !self.history.is_empty() {
                        self.list_state.select(Some(self.history.len() - 1));
                    }

                    if self
                        .query
                        .aggregate_id
                        .as_ref()
                        .map(|agg| aggregate_id == *agg)
                        .unwrap_or(false)
                    {
                        self.fold_thread().await?;
                    }
                }
                UiEvent::App(app_event) => match app_event {
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
            KeyCode::Enter => self.events.send(UiEvent::App(AppEvent::Confirm)),
            KeyCode::Char('c' | 'C') if key.modifiers == KeyModifiers::CONTROL => {
                self.events.send(UiEvent::App(AppEvent::Quit))
            }
            KeyCode::Char(c) => self.events.send(UiEvent::App(AppEvent::Input(c))),
            KeyCode::Backspace => self.events.send(UiEvent::App(AppEvent::Erase)),
            KeyCode::Up => {
                self.auto_scroll = false;
                if let Some(selected) = self.list_state.selected() {
                    if selected > 0 {
                        self.list_state.select(Some(selected - 1));
                    }
                } else if !self.history.is_empty() {
                    self.list_state.select(Some(self.history.len() - 1));
                }
            }
            KeyCode::Down => {
                if let Some(selected) = self.list_state.selected() {
                    if selected < self.history.len() - 1 {
                        self.list_state.select(Some(selected + 1));
                        // Re-enable auto-scroll if we reach the bottom
                        if selected + 1 == self.history.len() - 1 {
                            self.auto_scroll = true;
                        }
                    }
                } else if !self.history.is_empty() {
                    self.list_state.select(Some(0));
                }
            }
            KeyCode::PageUp => {
                self.auto_scroll = false;
                if !self.history.is_empty() {
                    let current = self.list_state.selected().unwrap_or(self.history.len() - 1);
                    let new_pos = current.saturating_sub(10);
                    self.list_state.select(Some(new_pos));
                }
            }
            KeyCode::PageDown => {
                if !self.history.is_empty() {
                    let current = self.list_state.selected().unwrap_or(0);
                    let new_pos = (current + 10).min(self.history.len() - 1);
                    self.list_state.select(Some(new_pos));
                    // Re-enable auto-scroll if we reach the bottom
                    if new_pos == self.history.len() - 1 {
                        self.auto_scroll = true;
                    }
                }
            }
            KeyCode::Home => {
                self.auto_scroll = false;
                if !self.history.is_empty() {
                    self.list_state.select(Some(0));
                }
            }
            KeyCode::End => {
                self.auto_scroll = true;
                if !self.history.is_empty() {
                    self.list_state.select(Some(self.history.len() - 1));
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn fold_thread(&mut self) -> color_eyre::Result<()> {
        let events = self
            .store
            .load_events::<AgentEvent>(&self.query, None)
            .await?;

        // Emit debug event for thread folding
        let debug_event = AgentEvent::Debug {
            message: format!("Folding thread with {} events", events.len()),
            context: Some(serde_json::json!({
                "event_count": events.len(),
                "aggregate_id": self.query.aggregate_id.clone(),
            })),
            target: "debugger".to_string(),
        };
        self.history.push(StoreEvent {
            stream_id: self.query.stream_id.clone(),
            event_type: "debug".to_string(),
            aggregate_id: "debugger".to_string(),
            sequence: self.history.len() as i64,
            event_version: "1.0".to_string(),
            data: debug_event,
            metadata: serde_json::json!({}),
            created_at: chrono::Utc::now(),
        });

        self.thread = thread::Thread::fold(&events);

        // Auto-scroll to bottom after folding thread
        if self.auto_scroll && !self.history.is_empty() {
            self.list_state.select(Some(self.history.len() - 1));
        }

        Ok(())
    }

    async fn send_message_to(
        &mut self,
        aggregate_id: &str,
        content: String,
    ) -> color_eyre::Result<()> {
        let text = UserContent::Text(Text { text: content });
        let message = OneOrMany::one(text);
        let metadata = Metadata::default();

        if self
            .query
            .aggregate_id
            .as_ref()
            .map(|current| current == aggregate_id)
            .unwrap_or(false)
        {
            let events = self.thread.process(thread::Command::User(message))?;
            for event in events {
                self.store
                    .push_event(&self.query.stream_id, aggregate_id, &event, &metadata)
                    .await?;
            }
        } else {
            let query = Query::stream(&self.query.stream_id).aggregate(aggregate_id);
            let events = self.store.load_events::<AgentEvent>(&query, None).await?;
            let mut thread = thread::Thread::fold(&events);
            let events = thread.process(thread::Command::User(message))?;
            for event in events {
                self.store
                    .push_event(&self.query.stream_id, aggregate_id, &event, &metadata)
                    .await?;
            }
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
            let aggregate_id = self
                .pending_prompt_target
                .clone()
                .or_else(|| self.query.aggregate_id.clone())
                .unwrap_or_else(|| "thread".to_string());

            self.send_message_to(&aggregate_id, self.input_buffer.clone())
                .await?;
            self.input_buffer.clear();

            if self.pending_prompt_target.is_some() {
                self.pending_prompt = None;
                self.pending_prompt_target = None;
            }
        }
        Ok(())
    }

    pub fn input(&mut self, input: char) {
        self.input_buffer.push(input);
    }

    async fn setup_thread(&mut self) -> color_eyre::Result<()> {
        // Check if thread is already configured
        if self.thread.model.is_some() {
            return Ok(());
        }

        // Send setup command
        let setup_command = thread::Command::Setup {
            model: "claude-sonnet-4-20250514".to_string(),
            temperature: 0.7,
            max_tokens: 4096,
            preamble: Some("You are a helpful AI assistant.".to_string()),
            tools: None,
            recipient: None,
        };

        let events = self.thread.process(setup_command)?;
        let metadata = Metadata::default();

        for event in events {
            self.store
                .push_event(
                    &self.query.stream_id,
                    &self.query.aggregate_id.clone().unwrap_or_default(),
                    &event,
                    &metadata,
                )
                .await?;
        }

        Ok(())
    }
}
