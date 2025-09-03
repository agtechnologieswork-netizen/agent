use crate::planner::handler::Event;
use dabgent_mq::models::Event as MqEvent;

impl MqEvent for Event {
    const EVENT_VERSION: &'static str = "1.0";

    fn event_type() -> &'static str {
        // Use enum name as the broad event type; DabGent MQ's trait signature
        // requires an associated type name, not a per-variant discriminator.
        // Variant-level filtering can be handled via metadata if needed later.
        "PlannerEvent"
    }
}


