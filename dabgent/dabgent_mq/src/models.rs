use serde::{Deserialize, Serialize};

pub trait Event: Serialize + for<'de> Deserialize<'de> + Send + Sync {
    const EVENT_VERSION: &'static str;
    fn event_type() -> &'static str;
}
