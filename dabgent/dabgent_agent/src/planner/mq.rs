use crate::planner::handler::Event;
use dabgent_mq::models::Event as MqEvent;

impl MqEvent for Event {
    const EVENT_VERSION: &'static str = "1.0";

    fn event_type(&self) -> &'static str {
        // Return variant-specific event types for better routing
        // This enables specialized executors to subscribe to specific events
        "PlannerEvent"  // Base type - variants can be filtered via Query
    }
}

// Helper for getting variant-specific event type (for future routing)
impl Event {
    pub fn variant_type(&self) -> &'static str {
        match self {
            Event::TasksPlanned { .. } => "TasksPlanned",
            Event::TaskDispatched { .. } => "TaskDispatched",
            Event::TaskStatusUpdated { .. } => "TaskStatusUpdated",
            Event::ClarificationRequested { .. } => "ClarificationRequested",
            Event::ClarificationReceived { .. } => "ClarificationReceived",
            Event::PlanningCompleted { .. } => "PlanningCompleted",
        }
    }
}
