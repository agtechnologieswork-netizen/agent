use color_eyre::eyre::OptionExt;
use crossterm::event::Event as CrosstermEvent;
use dabgent_agent::thread::{self};
use dabgent_mq::db::EventStream;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;

const TICK_FPS: f64 = 30.0;

#[derive(Debug, Clone)]
pub enum AppEvent {
    Confirm,
    Erase,
    Input(char),
    Quit,
}

#[derive(Debug, Clone)]
pub enum Event {
    Tick,
    Crossterm(CrosstermEvent),
    Thread(thread::Event),
    App(AppEvent),
}

pub struct EventHandler {
    sender: mpsc::UnboundedSender<Event>,
    receiver: mpsc::UnboundedReceiver<Event>,
}

impl EventHandler {
    pub fn new(events_stream: EventStream<thread::Event>) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let actor = EventTask::new(sender.clone());
        tokio::spawn(async { actor.run().await });
        let actor = StoreTask::new(sender.clone(), events_stream);
        tokio::spawn(async { actor.run().await });
        Self { sender, receiver }
    }

    pub async fn next(&mut self) -> color_eyre::Result<Event> {
        self.receiver
            .recv()
            .await
            .ok_or_eyre("Failed to receive event")
    }

    pub fn send(&self, event: Event) {
        let _ = self.sender.send(event);
    }
}

pub struct StoreTask {
    sender: mpsc::UnboundedSender<Event>,
    receiver: EventStream<thread::Event>,
}

impl StoreTask {
    pub fn new(sender: mpsc::UnboundedSender<Event>, receiver: EventStream<thread::Event>) -> Self {
        Self { sender, receiver }
    }

    pub async fn run(mut self) -> color_eyre::Result<()> {
        while let Some(event) = self.receiver.next().await {
            match event {
                Ok(event) => {
                    let _ = self.sender.send(Event::Thread(event));
                }
                Err(error) => {
                    tracing::error!("Error receiving app event: {}", error);
                }
            }
        }
        Ok(())
    }
}

pub struct EventTask {
    sender: mpsc::UnboundedSender<Event>,
}

impl EventTask {
    pub fn new(sender: mpsc::UnboundedSender<Event>) -> Self {
        Self { sender }
    }

    pub async fn run(self) -> color_eyre::Result<()> {
        let tick_rate = Duration::from_secs_f64(1.0 / TICK_FPS);
        let mut reader = crossterm::event::EventStream::new();
        let mut tick = tokio::time::interval(tick_rate);
        loop {
            let tick_delay = tick.tick();
            tokio::select! {
                _ = self.sender.closed() => {
                    break;
                }
                _ = tick_delay => {
                    self.send(Event::Tick);
                }
                Some(Ok(evt)) = reader.next() => {
                    self.send(Event::Crossterm(evt));
                }
            };
        }
        Ok(())
    }

    fn send(&self, event: Event) {
        let _ = self.sender.send(event);
    }
}
