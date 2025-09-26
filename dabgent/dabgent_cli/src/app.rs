use crate::events::{AppEvent, Event as UiEvent, EventHandler};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dabgent_agent::event::Event as AgentEvent;
use dabgent_mq::db::{Event as StoreEvent, EventStore, Metadata, Query};
use ratatui::widgets::ListState;
use rig::OneOrMany;
use rig::message::{Text, UserContent};

pub struct App<S: EventStore> {
    store: S,
    query: Query,
    pub history: Vec<StoreEvent<AgentEvent>>,
    pub input_buffer: String,
    pub running: bool,
    pub events: EventHandler,
    pub list_state: ListState,
    pub auto_scroll: bool,
    pub pending_prompt: Option<String>,
    pub pending_prompt_target: Option<String>,
}

impl<S: EventStore> App<S> {
    pub fn new(store: S, stream_id: String) -> color_eyre::Result<Self> {
        let query = Query::stream(stream_id.clone()).aggregate("thread");

        let event_stream = store.subscribe::<AgentEvent>(&Query::stream(stream_id.clone()))?;
        let events = EventHandler::new(event_stream);

        Ok(Self {
            store,
            query,
            history: Vec::new(),
            input_buffer: String::new(),
            running: true,
            events,
            list_state: ListState::default(),
            auto_scroll: true,
            pending_prompt: None,
            pending_prompt_target: None,
        })
    }

    pub async fn run(mut self, mut terminal: ratatui::DefaultTerminal) -> color_eyre::Result<()> {
        // Don't set up thread here - let the pipeline handle it
        // Don't fold thread at startup - wait for pipeline to initialize
        while self.running {
            terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;
            match self.events.next().await? {
                UiEvent::Tick => self.tick(),
                UiEvent::Crossterm(event) => match event {
                    crossterm::event::Event::Key(key_event) => self.handle_key_events(key_event)?,
                    _ => {}
                },
                UiEvent::Thread(event) => {
                    if let dabgent_agent::event::Event::UserInputRequested { prompt, .. } =
                        &event.data
                    {
                        self.pending_prompt = Some(prompt.clone());
                        self.pending_prompt_target = Some(event.aggregate_id.clone());
                    }

                    self.history.push(event);

                    // Auto-scroll to bottom if enabled
                    if self.auto_scroll && !self.history.is_empty() {
                        self.list_state.select(Some(self.history.len() - 1));
                    }

                    // No need to fold thread anymore - pipeline handles everything
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

    async fn send_message_to(
        &mut self,
        aggregate_id: &str,
        content: String,
    ) -> color_eyre::Result<()> {
        let text = UserContent::Text(Text { text: content });
        let message = OneOrMany::one(text);
        let user_event = AgentEvent::UserMessage(message);
        let metadata = Metadata::default();

        // Just push the user message event and let the pipeline handle it
        self.store
            .push_event(&self.query.stream_id, aggregate_id, &user_event, &metadata)
            .await?;

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
}