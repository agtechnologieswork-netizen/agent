use rig::{completion::AssistantContent, message::Message};

pub use rig::completion::AssistantContent as RigAssistantContent;

#[derive(Debug, Clone)]
pub enum FinishReason { Stop }

#[derive(Debug, Clone)]
pub struct CompletionResponse {
    pub choice: rig::OneOrMany<AssistantContent>,
    pub finish_reason: FinishReason,
    pub output_tokens: u64,
    pub input_tokens: u64,
    pub cache_read_input_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
}

pub struct Completion {
    pub model: String,
    pub message: Message,
    pub preamble: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u64>,
}

impl Completion {
    pub fn new(model: String, message: Message) -> Self {
        Self { model, message, preamble: None, temperature: None, max_tokens: None }
    }
    pub fn preamble(mut self, p: String) -> Self { self.preamble = Some(p); self }
    pub fn temperature(mut self, t: f32) -> Self { self.temperature = Some(t); self }
    pub fn max_tokens(mut self, m: u64) -> Self { self.max_tokens = Some(m); self }
}

pub trait LLMClientDyn: Send + Sync {
    fn completion(&self, completion: Completion) -> std::pin::Pin<Box<dyn std::future::Future<Output = eyre::Result<CompletionResponse>> + Send + '_>>;
}

pub trait LLMClient: LLMClientDyn + 'static {
    fn boxed(self) -> Box<dyn LLMClientDyn> where Self: Sized { Box::new(self) }
}

impl<T: LLMClientDyn + 'static> LLMClient for T {}
