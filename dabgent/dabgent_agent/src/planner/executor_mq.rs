//! Event persistence for executor events

use crate::planner::executor::ExecutorEventOutput;
use dabgent_mq::models::Event as MqEvent;

impl MqEvent for ExecutorEventOutput {
    const EVENT_VERSION: &'static str = "1.0";
    
    fn event_type(&self) -> &'static str {
        match self {
            ExecutorEventOutput::TaskStarted { .. } => "TaskStarted",
            ExecutorEventOutput::TaskCompleted { .. } => "TaskCompleted",
            ExecutorEventOutput::TaskFailed { .. } => "TaskFailed",
            ExecutorEventOutput::NeedsClarification { .. } => "NeedsClarification",
            ExecutorEventOutput::ClarificationProvided { .. } => "ClarificationProvided",
        }
    }
}
