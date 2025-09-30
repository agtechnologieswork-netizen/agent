use dabgent_mq::Metadata;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentExtra {
    Worker { aggregate_id: String },
}

impl AgentExtra {
    pub fn new_worker(aggregate_id: String) -> Self {
        Self::Worker { aggregate_id }
    }
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

    pub fn with_extra(mut self, extra: AgentExtra) -> Self {
        self.extra = Some(extra);
        self
    }
}

impl From<AgentMetadata> for Metadata {
    fn from(meta: AgentMetadata) -> Self {
        Metadata {
            correlation_id: meta.correlation_id,
            causation_id: meta.causation_id,
            extra: meta.extra.map(|e| serde_json::to_value(e).unwrap()),
        }
    }
}

impl TryFrom<&Metadata> for AgentMetadata {
    type Error = eyre::Error;

    fn try_from(metadata: &Metadata) -> Result<Self, Self::Error> {
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
