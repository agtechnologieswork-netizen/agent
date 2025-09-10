use crate::session::{ChatCommand, ChatEvent, ChatSession};
use dabgent_agent::handler::Handler;
use dabgent_mq::db::{EventStore, Metadata, Query};

pub struct MockAgent<S: EventStore> {
    store: S,
    stream_id: String,
    aggregate_id: String,
}

impl<S: EventStore> MockAgent<S> {
    pub fn new(store: S, stream_id: String, aggregate_id: String) -> Self {
        Self {
            store,
            stream_id,
            aggregate_id,
        }
    }

    pub async fn run(self) -> color_eyre::Result<()> {
        let query = Query {
            stream_id: self.stream_id.clone(),
            event_type: Some("user_message".to_string()),
            aggregate_id: Some(self.aggregate_id.clone()),
        };
        let mut event_stream = self.store.subscribe::<ChatEvent>(&query)?;
        while let Some(result) = event_stream.next().await {
            match result {
                Ok(ChatEvent::UserMessage { content, .. }) => {
                    let all_query = Query {
                        stream_id: self.stream_id.clone(),
                        event_type: None,
                        aggregate_id: Some(self.aggregate_id.clone()),
                    };
                    let events = self
                        .store
                        .load_events::<ChatEvent>(&all_query, None)
                        .await?;
                    let mut session = ChatSession::fold(&events);
                    let command = ChatCommand::AgentRespond(format!("I received: {}", content));
                    let new_events = session.process(command)?;
                    let metadata = Metadata::default();
                    for event in new_events {
                        self.store
                            .push_event(&self.stream_id, &self.aggregate_id, &event, &metadata)
                            .await?;
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::error!("Error receiving event: {}", e);
                }
            }
        }
        Ok(())
    }
}
