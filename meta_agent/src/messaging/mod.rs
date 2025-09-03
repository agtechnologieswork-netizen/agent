use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope<T> {
    pub topic: String,
    pub payload: T,
}

pub trait EventBus<T: Clone + Send + Sync + 'static> {
    fn publish(&self, message: Envelope<T>);
}

#[derive(Default)]
pub struct NoopBus;

impl<T: Clone + Send + Sync + 'static> EventBus<T> for NoopBus {
    fn publish(&self, _message: Envelope<T>) {}
}


