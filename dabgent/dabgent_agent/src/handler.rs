pub trait Handler {
    type Command;
    type Event;
    type Error: std::error::Error + Send + Sync + 'static;

    fn process(&mut self, command: Self::Command) -> Result<Vec<Self::Event>, Self::Error>;
    fn fold(events: &[Self::Event]) -> Self;
}
