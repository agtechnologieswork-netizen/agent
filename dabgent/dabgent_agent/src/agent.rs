use crate::llm::LLMClient;

pub struct Worker<T: LLMClient> {
    llm: T,
    tools: Vec<rig::completion::ToolDefinition>,
}
