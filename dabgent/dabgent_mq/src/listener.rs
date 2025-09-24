use crate::{Aggregate, Envelope, EventStore};
use eyre::Result;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, broadcast};

pub trait Callback<A: Aggregate>: Send {
    fn process(&mut self, event: &Envelope<A>) -> impl Future<Output = Result<()>> + Send;
    fn boxed(self) -> Box<dyn CallbackDyn<A>>
    where
        A: 'static,
        Self: Sized + 'static,
    {
        Box::new(self)
    }
}

// pub trait CallbackDyn<A: Aggregate>: Send {
//     fn process(
//         &mut self,
//         event: Envelope<A>,
//     ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
// }

// impl<A: Aggregate + 'static, T: Callback<A>> CallbackDyn<A> for T {
//     fn process(
//         &mut self,
//         event: Envelope<A>,
//     ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
//         Box::pin(self.process(event))
//     }
// }

pub trait CallbackDyn<A: Aggregate>: Send {
    fn process<'a>(
        &'a mut self,
        event: &'a Envelope<A>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;
}

impl<A: Aggregate, T: Callback<A>> CallbackDyn<A> for T {
    fn process<'a>(
        &'a mut self,
        event: &'a Envelope<A>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(self.process(event))
    }
}

#[derive(Clone)]
pub struct Wake {
    aggregate_id: String,
    current_sequence: i64,
}

#[derive(Clone)]
pub struct PollingQueue<ES: EventStore> {
    store: ES,
    wake_tx: broadcast::Sender<Wake>,
}

impl<ES: EventStore> EventStore for PollingQueue<ES> {
    async fn commit<A: Aggregate>(
        &self,
        events: Vec<A::Event>,
        metadata: crate::Metadata,
        context: crate::AggregateContext<A>,
    ) -> Result<Vec<Envelope<A>>, crate::db::Error> {
        let wake = Wake {
            aggregate_id: context.aggregate_id.clone(),
            current_sequence: context.current_sequence,
        };
        let events = self.store.commit(events, metadata, context).await?;
        let _ = self.wake_tx.send(wake);
        Ok(events)
    }

    async fn load_aggregate<A: Aggregate>(
        &self,
        aggregate_id: &str,
    ) -> Result<crate::AggregateContext<A>, crate::db::Error> {
        self.store.load_aggregate(aggregate_id).await
    }

    async fn load_events<A: Aggregate>(
        &self,
        aggregate_id: &str,
    ) -> Result<Vec<Envelope<A>>, crate::db::Error> {
        self.store.load_events(aggregate_id).await
    }

    async fn load_latest_events<A: Aggregate>(
        &self,
        aggregate_id: &str,
        sequence_from: i64,
    ) -> Result<Vec<Envelope<A>>, crate::db::Error> {
        self.store
            .load_latest_events(aggregate_id, sequence_from)
            .await
    }

    async fn load_sequence_nums<A: Aggregate>(
        &self,
    ) -> Result<Vec<(String, i64)>, crate::db::Error> {
        self.store.load_sequence_nums::<A>().await
    }
}

type ArcCallback<A> = Arc<Mutex<dyn CallbackDyn<A>>>;

pub struct Listener<A: Aggregate, ES: EventStore> {
    store: ES,
    wake_rx: broadcast::Receiver<Wake>,
    callbacks: Vec<ArcCallback<A>>,
    offsets: HashMap<String, i64>,
    poll_interval: Duration,
}

impl<A: Aggregate, ES: EventStore> Listener<A, ES> {
    pub fn new(
        store: ES,
        wake_rx: broadcast::Receiver<Wake>,
        callbacks: Vec<ArcCallback<A>>,
    ) -> Self {
        Self {
            store,
            wake_rx,
            callbacks,
            offsets: HashMap::new(),
            poll_interval: Duration::from_secs(1),
        }
    }

    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    pub async fn run(&mut self) -> eyre::Result<()> {
        // let mut task_set = tokio::task::JoinSet::new();
        let mut interval = tokio::time::interval(self.poll_interval);
        loop {
            let mut update_set = Vec::new();
            tokio::select! {
                _ = interval.tick() => {
                    let candidates = self.store.load_sequence_nums::<A>().await?;
                    update_set = candidates.into_iter().filter_map(|(id, seq)| match self.offsets.get(&id) {
                        Some(&offset) if offset < seq => Some((id, offset)),
                        None => Some((id, 0)),
                        _ => None,
                    }).collect();
                }
                _ = self.wake_rx.recv() => {
                    // todo
                }
                // res = task_set.join_next() => match res {
                //     _ => {} // todo
                // }
            }
            // for (id, offset) in update_set.into_iter() {
            //     let store = self.store.clone();
            //     let callbacks = self.callbacks.clone();
            //     task_set.spawn(async move {
            //         let events = store.load_latest_events::<A>(&id, offset).await?;
            //         let sequence = events.iter().map(|e| e.sequence).max().unwrap();
            //         for event in events.iter() {
            //             let mut local = tokio::task::JoinSet::new();
            //             for callback in callbacks.iter().cloned() {
            //                 let event = event.clone();
            //                 local.spawn(async move {
            //                     let mut lock = callback.lock().await;
            //                     lock.process(&event).await;
            //                 });
            //             }
            //             local.join_all().await;
            //         }
            //         Ok::<_, eyre::ErrReport>((id, sequence))
            //     });
            // }
        }
        // while let Some(envelope) = self.receiver.recv().await {
        //     for callback in self.callbacks.iter_mut() {
        //         callback.process(&envelope)?;
        //     }
        // }
        Ok(())
    }

    // async fn execute_callbacks(
    //     &self,
    //     aggregate_id: &str,
    //     last_sequence: i64,
    // ) -> eyre::Result<(String, i64)> {
    //     let events = self
    //         .store
    //         .load_latest_events::<A>(aggregate_id, last_sequence)
    //         .await?;
    //     let sequence = match events.iter().map(|e| e.sequence).max() {
    //         Some(sequence) => sequence,
    //         None => return Ok(()),
    //     };
    //     for event in events.iter() {
    //         for callback in self.callbacks.iter_mut() {
    //             callback.process(event).await?;
    //         }
    //     }
    //     self.offsets.insert(aggregate_id.to_owned(), sequence);
    //     Ok((aggregate_id.to_owned(), sequence))
    // }
}

pub async fn run_callbacks<A: Aggregate>(
    event: Envelope<A>,
    callbacks: &[ArcCallback<A>],
) -> Result<()> {
    let mut set = tokio::task::JoinSet::new();
    for c in callbacks.iter().cloned() {
        let event = event.clone();
        set.spawn(async move { c.lock().await.process(&event).await });
    }
    while let Some(result) = set.join_next().await {
        result??;
    }
    Ok(())
}
