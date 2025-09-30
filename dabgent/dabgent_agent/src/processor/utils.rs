use dabgent_mq::{Aggregate, Callback, Envelope};
use eyre::Result;

pub struct LoggerCallback<T: Aggregate> {
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Aggregate> LoggerCallback<T> {
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T: Aggregate + std::fmt::Debug> Callback<T> for LoggerCallback<T> {
    async fn process(&mut self, envelope: &Envelope<T>) -> Result<()> {
        tracing::info!(aggregate = T::TYPE, envelope = ?envelope, "event");
        Ok(())
    }
}
