use crate::event::Event;
use crate::llm::FinishReason;
use dabgent_mq::db::Metadata;
use dabgent_mq::{EventStore, Query};
use eyre::Result;
use rig::OneOrMany;
use rig::completion::ToolDefinition;
use rig::message::{Text, UserContent};

#[derive(Debug, Clone)]
pub struct ThreadSettings {
    pub model: String,
    pub temperature: f64,
    pub max_tokens: u64,
    pub preamble: Option<String>,
    pub tools: Option<Vec<ToolDefinition>>,
}

impl ThreadSettings {
    pub fn new(model: impl Into<String>, temperature: f64, max_tokens: u64) -> Self {
        Self {
            model: model.into(),
            temperature,
            max_tokens,
            preamble: None,
            tools: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Task {
    pub description: String,
    thread: String,
    pub completed: bool,
}

impl Task {
    fn new(description: String, thread: String) -> Self {
        Self {
            description,
            thread,
            completed: false,
        }
    }

    pub fn thread(&self) -> &str {
        &self.thread
    }
}

pub struct Planner<S> {
    store: S,
    stream_id: String,
    settings: ThreadSettings,
    tasks: Vec<Task>,
}

impl<S: EventStore> Planner<S> {
    pub fn new(store: S, stream_id: impl Into<String>, settings: ThreadSettings) -> Self {
        Self {
            store,
            stream_id: stream_id.into(),
            settings,
            tasks: Vec::new(),
        }
    }

    pub fn plan(&mut self, input: &str) {
        self.tasks.clear();
        for line in input.lines() {
            let description = line
                .trim()
                .trim_start_matches(|c: char| c.is_ascii_punctuation() || c.is_ascii_digit())
                .trim();
            if description.is_empty() {
                continue;
            }
            let thread = format!("task-{}", self.tasks.len());
            self.tasks.push(Task::new(description.to_string(), thread));
        }
        if self.tasks.is_empty() {
            let trimmed = input.trim();
            if !trimmed.is_empty() {
                self.tasks
                    .push(Task::new(trimmed.to_string(), "task-0".to_string()));
            }
        }
    }

    pub fn tasks(&self) -> &[Task] {
        &self.tasks
    }

    pub fn set_tasks(&mut self, descriptions: Vec<String>) {
        self.tasks.clear();
        for (index, description) in descriptions.into_iter().enumerate() {
            let thread = format!("task-{}", index);
            self.tasks.push(Task::new(description, thread));
        }
    }

    pub async fn execute(&mut self) -> Result<()> {
        for index in 0..self.tasks.len() {
            let description = self.tasks[index].description.clone();
            let thread = self.tasks[index].thread.clone();
            self.run_thread(&thread, &description).await?;
            self.tasks[index].completed = true;
        }
        Ok(())
    }

    async fn run_thread(&self, thread: &str, description: &str) -> Result<()> {
        let config = Event::LLMConfig {
            model: self.settings.model.clone(),
            temperature: self.settings.temperature,
            max_tokens: self.settings.max_tokens,
            preamble: self.settings.preamble.clone(),
            tools: self.settings.tools.clone(),
            recipient: Some(thread.to_string()),
        };
        let meta = Metadata::default();
        self.store
            .push_event(&self.stream_id, thread, &config, &meta)
            .await?;

        let content = UserContent::Text(Text {
            text: description.to_string(),
        });
        let user = Event::UserMessage(OneOrMany::one(content));
        let meta = Metadata::default();
        self.store
            .push_event(&self.stream_id, thread, &user, &meta)
            .await?;

        let query = Query::stream(&self.stream_id).aggregate(thread);
        let mut stream = self.store.subscribe::<Event>(&query)?;
        while let Some(event) = stream.next_full().await {
            let event = event?;
            if let Event::AgentMessage { response, .. } = event.data {
                if response.finish_reason != FinishReason::ToolUse {
                    break;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::CompletionResponse;
    use dabgent_mq::Query;
    use dabgent_mq::db::sqlite::SqliteStore;
    use rig::OneOrMany;
    use rig::message::{AssistantContent, Text as AssistantText};
    use sqlx::SqlitePool;

    async fn store() -> SqliteStore {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("failed to create sqlite pool");
        let store = SqliteStore::new(pool);
        store.migrate().await;
        store
    }

    #[tokio::test]
    async fn creates_tasks_from_lines() {
        let settings = ThreadSettings::new("model", 0.0, 128);
        let store = store().await;
        let mut planner = Planner::new(store, "stream", settings);
        planner.plan("- implement feature\n- write tests\n- deploy");

        let descriptions: Vec<_> = planner
            .tasks()
            .iter()
            .map(|task| task.description.as_str())
            .collect();
        assert_eq!(
            descriptions,
            vec!["implement feature", "write tests", "deploy"]
        );
        assert!(planner.tasks().iter().all(|task| !task.completed));
    }

    #[tokio::test]
    async fn falls_back_to_whole_prompt_when_no_lines() {
        let settings = ThreadSettings::new("model", 0.0, 128);
        let store = store().await;
        let mut planner = Planner::new(store, "stream", settings);
        planner.plan("Build an API endpoint");

        assert_eq!(planner.tasks().len(), 1);
        assert_eq!(planner.tasks()[0].description, "Build an API endpoint");
        assert_eq!(planner.tasks()[0].thread(), "task-0");
    }

    #[tokio::test]
    async fn executes_tasks_and_marks_completion() {
        let settings = ThreadSettings::new("model", 0.0, 128);
        let store = store().await;
        let responder_store = store.clone();
        let mut planner = Planner::new(store, "stream", settings);
        planner.plan("First\nSecond");

        let handle = tokio::spawn(async move {
            let mut subscription = responder_store
                .subscribe::<Event>(&Query::stream("stream"))
                .expect("subscribe");
            let mut responded = 0;
            while let Some(event) = subscription.next_full().await {
                let event = event.expect("event");
                if let Event::UserMessage(_) = event.data {
                    responded += 1;
                    let response = CompletionResponse {
                        choice: OneOrMany::one(AssistantContent::Text(AssistantText {
                            text: format!("ack-{responded}"),
                        })),
                        finish_reason: FinishReason::Stop,
                        output_tokens: 0,
                    };
                    let reply = Event::AgentMessage {
                        response,
                        recipient: Some(event.aggregate_id.clone()),
                    };
                    let meta = Metadata::default();
                    responder_store
                        .push_event(&event.stream_id, &event.aggregate_id, &reply, &meta)
                        .await
                        .expect("push reply");
                    if responded == 2 {
                        break;
                    }
                }
            }
        });

        planner.execute().await.expect("planner execution");
        handle.await.expect("worker finished");

        assert!(planner.tasks().iter().all(|task| task.completed));
    }
}
