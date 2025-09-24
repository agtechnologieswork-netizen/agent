use dabgent_mq::EventStore;
use eyre::Result;
use serde::{Deserialize, Serialize};
use super::{NoSandboxTool, NoSandboxAdapter};

/// Tool for creating a plan from task descriptions
pub struct CreatePlanTool<S: EventStore> {
    store: S,
    stream_id: String,
}

impl<S: EventStore + Clone> CreatePlanTool<S> {
    pub fn new(store: S, stream_id: String) -> Self {
        Self {
            store,
            stream_id,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePlanArgs {
    pub tasks: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePlanOutput {
    pub tasks: Vec<String>,
    pub message: String,
}

impl<S: EventStore + Clone + Send + Sync> NoSandboxTool for CreatePlanTool<S> {
    type Args = CreatePlanArgs;
    type Output = CreatePlanOutput;
    type Error = String;

    fn name(&self) -> String {
        "create_plan".to_string()
    }

    fn definition(&self) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.name(),
            description: "Create a plan by breaking down a task into concrete, actionable steps.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "tasks": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "description": "A concrete, actionable task description"
                        },
                        "description": "An ordered list of tasks to complete"
                    }
                },
                "required": ["tasks"]
            }),
        }
    }

    async fn call(
        &self,
        args: Self::Args,
    ) -> Result<Result<Self::Output, Self::Error>> {
        // Create PlanCreated event
        let event = crate::event::Event::PlanCreated {
            tasks: args.tasks.clone(),
        };

        // Push event to store
        match self.store
            .push_event(&self.stream_id, "planner", &event, &Default::default())
            .await {
            Ok(_) => {},
            Err(e) => return Ok(Err(e.to_string())),
        }

        let message = format!("Created plan with {} tasks", args.tasks.len());

        Ok(Ok(CreatePlanOutput {
            tasks: args.tasks,
            message
        }))
    }
}

/// Tool for updating an existing plan
pub struct UpdatePlanTool<S: EventStore> {
    store: S,
    stream_id: String,
}

impl<S: EventStore> UpdatePlanTool<S> {
    pub fn new(store: S, stream_id: String) -> Self {
        Self { store, stream_id }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdatePlanArgs {
    pub tasks: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdatePlanOutput {
    pub tasks: Vec<String>,
    pub message: String,
}

impl<S: EventStore + Send + Sync> NoSandboxTool for UpdatePlanTool<S> {
    type Args = UpdatePlanArgs;
    type Output = UpdatePlanOutput;
    type Error = String;

    fn name(&self) -> String {
        "update_plan".to_string()
    }

    fn definition(&self) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.name(),
            description: "Update the existing plan with a new set of tasks".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "tasks": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "description": "A concrete, actionable task description"
                        },
                        "description": "An updated ordered list of tasks to complete"
                    }
                },
                "required": ["tasks"]
            }),
        }
    }

    async fn call(
        &self,
        args: Self::Args,
    ) -> Result<Result<Self::Output, Self::Error>> {
        // Create PlanUpdated event
        let event = crate::event::Event::PlanUpdated {
            tasks: args.tasks.clone(),
        };

        // Push event to store
        match self.store
            .push_event(&self.stream_id, "planner", &event, &Default::default())
            .await {
            Ok(_) => {},
            Err(e) => return Ok(Err(e.to_string())),
        }

        let message = format!("Updated plan with {} tasks", args.tasks.len());

        Ok(Ok(UpdatePlanOutput {
            tasks: args.tasks,
            message
        }))
    }
}

/// Tool for getting the current plan status from events
pub struct GetPlanStatusTool<S: EventStore> {
    store: S,
    stream_id: String,
}

impl<S: EventStore> GetPlanStatusTool<S> {
    pub fn new(store: S, stream_id: String) -> Self {
        Self { store, stream_id }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetPlanStatusArgs {}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskStatus {
    pub description: String,
    pub thread_id: String,
    pub completed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetPlanStatusOutput {
    pub tasks: Vec<TaskStatus>,
    pub completed_count: usize,
    pub total_count: usize,
}

impl<S: EventStore + Send + Sync> NoSandboxTool for GetPlanStatusTool<S> {
    type Args = GetPlanStatusArgs;
    type Output = GetPlanStatusOutput;
    type Error = String;

    fn name(&self) -> String {
        "get_plan_status".to_string()
    }

    fn definition(&self) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.name(),
            description: "Get the current status of the plan".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    async fn call(
        &self,
        _args: Self::Args,
    ) -> Result<Result<Self::Output, Self::Error>> {
        // Load events to find the latest plan
        let query = dabgent_mq::Query::stream(&self.stream_id).aggregate("planner");
        let events = match self.store
            .load_events::<crate::event::Event>(&query, None)
            .await {
            Ok(events) => events,
            Err(e) => return Ok(Err(e.to_string())),
        };

        // Find the most recent plan
        let mut current_tasks: Option<Vec<String>> = None;
        for event in events.iter() {
            match event {
                crate::event::Event::PlanCreated { tasks } |
                crate::event::Event::PlanUpdated { tasks } => {
                    current_tasks = Some(tasks.clone());
                }
                _ => {}
            }
        }

        let tasks = match current_tasks {
            Some(tasks) => tasks,
            None => return Ok(Err("No plan exists yet. Use create_plan first.".to_string())),
        };

        // Convert to task status with thread IDs
        let task_statuses: Vec<TaskStatus> = tasks.iter()
            .enumerate()
            .map(|(i, desc)| TaskStatus {
                description: desc.clone(),
                thread_id: format!("task-{}", i),
                completed: false,  // Would need to track completion events
            })
            .collect();

        let total_count = task_statuses.len();
        let completed_count = task_statuses.iter().filter(|t| t.completed).count();

        Ok(Ok(GetPlanStatusOutput {
            tasks: task_statuses,
            completed_count,
            total_count,
        }))
    }
}

pub fn planning_toolset<S: EventStore + Clone + Send + Sync + 'static>(
    store: S,
    stream_id: String,
) -> Vec<Box<dyn super::ToolDyn>> {
    vec![
        Box::new(NoSandboxAdapter::new(CreatePlanTool::new(store.clone(), stream_id.clone()))),
        Box::new(NoSandboxAdapter::new(UpdatePlanTool::new(store.clone(), stream_id.clone()))),
        Box::new(NoSandboxAdapter::new(GetPlanStatusTool::new(store, stream_id))),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use dabgent_mq::db::sqlite::SqliteStore;
    use sqlx::SqlitePool;

    async fn test_store() -> SqliteStore {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");
        let store = SqliteStore::new(pool);
        store.migrate().await;
        store
    }

    #[tokio::test]
    async fn test_create_plan_tool() {
        let store = test_store().await;
        let tool = CreatePlanTool::new(store.clone(), "test-stream".to_string());

        // Test tool metadata
        assert_eq!(tool.name(), "create_plan");
        assert!(tool.definition().description.contains("Create a plan"));

        // Create a plan with structured tasks
        let args = CreatePlanArgs {
            tasks: vec!["Task 1".to_string(), "Task 2".to_string(), "Task 3".to_string()],
        };

        let result = tool.call(args).await.unwrap().unwrap();
        assert_eq!(result.tasks.len(), 3);
        assert_eq!(result.tasks[0], "Task 1");
        assert_eq!(result.tasks[1], "Task 2");
        assert_eq!(result.tasks[2], "Task 3");
        assert!(result.message.contains("3 tasks"));
    }

    #[tokio::test]
    async fn test_update_plan_tool() {
        let store = test_store().await;

        // First create a plan
        let create_tool = CreatePlanTool::new(store.clone(), "test-stream".to_string());
        let create_args = CreatePlanArgs {
            tasks: vec!["Initial task".to_string()],
        };
        create_tool.call(create_args).await.unwrap().unwrap();

        // Now test updating the plan
        let tool = UpdatePlanTool::new(store.clone(), "test-stream".to_string());
        assert_eq!(tool.name(), "update_plan");

        let args = UpdatePlanArgs {
            tasks: vec!["Updated task 1".to_string(), "Updated task 2".to_string()],
        };

        let result = tool.call(args).await.unwrap().unwrap();
        assert_eq!(result.tasks.len(), 2);
        assert_eq!(result.tasks[0], "Updated task 1");
        assert_eq!(result.tasks[1], "Updated task 2");
        assert!(result.message.contains("2 tasks"));
    }

    #[tokio::test]
    async fn test_get_plan_status_tool() {
        let store = test_store().await;

        // Create a plan first
        let create_tool = CreatePlanTool::new(store.clone(), "test-stream".to_string());
        let create_args = CreatePlanArgs {
            tasks: vec!["Task A".to_string(), "Task B".to_string(), "Task C".to_string()],
        };
        create_tool.call(create_args).await.unwrap().unwrap();

        // Get plan status
        let tool = GetPlanStatusTool::new(store.clone(), "test-stream".to_string());
        assert_eq!(tool.name(), "get_plan_status");

        let args = GetPlanStatusArgs {};
        let result = tool.call(args).await.unwrap().unwrap();

        assert_eq!(result.total_count, 3);
        assert_eq!(result.completed_count, 0); // No tasks completed yet
        assert_eq!(result.tasks.len(), 3);

        assert_eq!(result.tasks[0].description, "Task A");
        assert!(!result.tasks[0].completed);
        assert_eq!(result.tasks[0].thread_id, "task-0");

        assert_eq!(result.tasks[1].description, "Task B");
        assert!(!result.tasks[1].completed);
        assert_eq!(result.tasks[1].thread_id, "task-1");

        assert_eq!(result.tasks[2].description, "Task C");
        assert!(!result.tasks[2].completed);
        assert_eq!(result.tasks[2].thread_id, "task-2");
    }

    #[tokio::test]
    async fn test_get_plan_status_without_plan() {
        let store = test_store().await;
        let tool = GetPlanStatusTool::new(store, "test-stream".to_string());

        let args = GetPlanStatusArgs {};
        let result = tool.call(args).await.unwrap();

        assert!(result.is_err());
        if let Err(error) = result {
            assert!(error.contains("No plan exists"));
            assert!(error.contains("create_plan first"));
        }
    }
}