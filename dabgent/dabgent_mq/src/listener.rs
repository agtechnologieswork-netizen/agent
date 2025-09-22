use crate::{Aggregate, Envelope, EventStore};
use tokio::sync::mpsc;

pub trait Callback<A: Aggregate>: Send {
    fn process(&mut self, event: &Envelope<A>) -> eyre::Result<()>;
}

pub struct Listener<A: Aggregate> {
    pub receiver: mpsc::UnboundedReceiver<Envelope<A>>,
    pub callbacks: Vec<Box<dyn Callback<A>>>,
}

impl<A: Aggregate> Listener<A> {
    pub fn new(
        receiver: mpsc::UnboundedReceiver<Envelope<A>>,
        callbacks: Vec<Box<dyn Callback<A>>>,
    ) -> Self {
        Self {
            receiver,
            callbacks,
        }
    }

    pub async fn run(&mut self) -> eyre::Result<()> {
        while let Some(envelope) = self.receiver.recv().await {
            for callback in self.callbacks.iter_mut() {
                callback.process(&envelope)?;
            }
        }
        Ok(())
    }
}
