use dabgent_mq::Metadata;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerContext {
    pub worker_id: String,
    pub thread_id: String,
    pub sandbox_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentExtra {
    Worker(WorkerContext),
}

#[derive(Debug, Clone)]
pub struct AgentMetadata {
    pub correlation_id: Option<uuid::Uuid>,
    pub causation_id: Option<uuid::Uuid>,
    pub extra: Option<AgentExtra>,
}

impl AgentMetadata {
    pub fn new() -> Self {
        Self {
            correlation_id: None,
            causation_id: None,
            extra: None,
        }
    }

    pub fn with_correlation(mut self, id: uuid::Uuid) -> Self {
        self.correlation_id = Some(id);
        self
    }

    pub fn with_causation(mut self, id: uuid::Uuid) -> Self {
        self.causation_id = Some(id);
        self
    }

    pub fn with_worker_context(mut self, context: WorkerContext) -> Self {
        self.extra = Some(AgentExtra::Worker(context));
        self
    }
}

impl From<AgentMetadata> for Metadata {
    fn from(meta: AgentMetadata) -> Self {
        Metadata {
            correlation_id: meta.correlation_id,
            causation_id: meta.causation_id,
            extra: meta.extra.and_then(|e| serde_json::to_value(e).ok()),
        }
    }
}

impl TryFrom<Metadata> for AgentMetadata {
    type Error = eyre::Error;

    fn try_from(metadata: Metadata) -> Result<Self, Self::Error> {
        let extra = metadata
            .extra
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        Ok(AgentMetadata {
            correlation_id: metadata.correlation_id,
            causation_id: metadata.causation_id,
            extra,
        })
    }
}